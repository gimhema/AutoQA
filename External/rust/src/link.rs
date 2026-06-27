use std::io;
use std::net::ToSocketAddrs;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

use serde_json::Value;

use crate::conn::{Connection, Listener};
use crate::message::{Action, Message, Observation};

struct Shared {
    latest: Mutex<Option<Action>>,
    alive: AtomicBool,
}

pub struct OuroborosLink {
    writer: Connection,
    next_seq: AtomicU64,
    shared: Arc<Shared>,
    reader: Option<JoinHandle<()>>,
}

impl OuroborosLink {
    /// 지정한 주소에서 Ouroboros 에이전트의 접속을 기다린다 (blocking).
    pub fn accept<A: ToSocketAddrs>(addr: A) -> io::Result<Self> {
        let listener = Listener::bind(addr)?;
        let conn = listener.accept()?;
        Self::from_connection(conn)
    }

    /// 이미 수립된 연결로부터 Link를 구성한다.
    pub fn from_connection(conn: Connection) -> io::Result<Self> {
        let reader_conn = conn.try_clone()?;

        let shared = Arc::new(Shared {
            latest: Mutex::new(None),
            alive: AtomicBool::new(true),
        });

        let reader = {
            let shared = Arc::clone(&shared);
            thread::spawn(move || reader_loop(reader_conn, shared))
        };

        Ok(Self {
            writer: conn,
            next_seq: AtomicU64::new(0),
            shared,
            reader: Some(reader),
        })
    }

    /// 게임 상태를 관측으로 전송한다. seq와 timestamp는 자동 부여.
    pub fn send_observation(&mut self, state: Value) -> io::Result<()> {
        let seq = self.next_seq.fetch_add(1, Ordering::Relaxed);
        let msg = Message::from(Observation::new(seq, state));
        let bytes = msg
            .to_bytes()
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        self.writer.send(&bytes)
    }

    /// 가장 최근에 수신된 액션을 가져가며 슬롯을 비운다 (non-blocking).
    pub fn poll_action(&self) -> Option<Action> {
        self.shared.latest.lock().unwrap().take()
    }

    /// 슬롯을 비우지 않고 최신 액션을 복제해 본다.
    pub fn peek_action(&self) -> Option<Action> {
        self.shared.latest.lock().unwrap().clone()
    }

    /// Ouroboros 에이전트와의 연결이 살아 있는지.
    pub fn is_connected(&self) -> bool {
        self.shared.alive.load(Ordering::Acquire)
    }
}

impl Drop for OuroborosLink {
    fn drop(&mut self) {
        let _ = self.writer.stream().shutdown(std::net::Shutdown::Both);
        if let Some(handle) = self.reader.take() {
            let _ = handle.join();
        }
    }
}

fn reader_loop(mut conn: Connection, shared: Arc<Shared>) {
    loop {
        match conn.recv() {
            Ok(Some(bytes)) => match Message::from_bytes(&bytes) {
                Ok(Message::Action(action)) => {
                    let mut slot = shared.latest.lock().unwrap();
                    let newer = slot.as_ref().is_none_or(|cur| action.seq > cur.seq);
                    if newer {
                        *slot = Some(action);
                    }
                }
                Ok(Message::Observation(_)) => {}
                Err(_) => {}
            },
            Ok(None) => break,
            Err(_) => break,
        }
    }
    shared.alive.store(false, Ordering::Release);
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::net::TcpStream;
    use std::time::Duration;

    fn connect_pair() -> (OuroborosLink, Connection) {
        let listener = Listener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();

        let agent_side = thread::spawn(move || {
            let stream = TcpStream::connect(addr).unwrap();
            Connection::from_stream(stream).unwrap()
        });

        let game_conn = listener.accept().unwrap();
        let link = OuroborosLink::from_connection(game_conn).unwrap();
        let agent = agent_side.join().unwrap();
        (link, agent)
    }

    #[test]
    fn send_observation_received_by_agent() {
        let (mut link, mut agent) = connect_pair();

        link.send_observation(json!({"hp": 100, "pos": [1.0, 2.0]}))
            .unwrap();

        let bytes = agent.recv().unwrap().unwrap();
        match Message::from_bytes(&bytes).unwrap() {
            Message::Observation(obs) => {
                assert_eq!(obs.seq, 0);
                assert_eq!(obs.state["hp"], 100);
            }
            _ => panic!("expected observation"),
        }
    }

    #[test]
    fn seq_auto_increments() {
        let (mut link, mut agent) = connect_pair();

        for i in 0..3u64 {
            link.send_observation(json!({"tick": i})).unwrap();
        }

        for expected_seq in 0..3u64 {
            let bytes = agent.recv().unwrap().unwrap();
            match Message::from_bytes(&bytes).unwrap() {
                Message::Observation(obs) => assert_eq!(obs.seq, expected_seq),
                _ => panic!("expected observation"),
            }
        }
    }

    #[test]
    fn poll_action_receives_agent_commands() {
        let (link, mut agent) = connect_pair();

        let action = Action {
            seq: 0,
            timestamp_ms: crate::message::now_millis(),
            command: json!({"key": "attack"}),
        };
        let bytes = Message::from(action).to_bytes().unwrap();
        agent.send(&bytes).unwrap();

        let mut received = None;
        for _ in 0..50 {
            received = link.poll_action();
            if received.is_some() {
                break;
            }
            thread::sleep(Duration::from_millis(5));
        }
        let action = received.expect("should have received action");
        assert_eq!(action.command, json!({"key": "attack"}));
    }

    #[test]
    fn poll_action_consumes_slot() {
        let (link, mut agent) = connect_pair();

        let action = Action {
            seq: 0,
            timestamp_ms: crate::message::now_millis(),
            command: json!({"key": "jump"}),
        };
        agent
            .send(&Message::from(action).to_bytes().unwrap())
            .unwrap();

        let mut first = None;
        for _ in 0..50 {
            first = link.poll_action();
            if first.is_some() {
                break;
            }
            thread::sleep(Duration::from_millis(5));
        }
        assert!(first.is_some());
        assert!(link.poll_action().is_none());
    }

    #[test]
    fn keeps_latest_action_only() {
        let (link, mut agent) = connect_pair();

        for seq in 0..5u64 {
            let action = Action {
                seq,
                timestamp_ms: crate::message::now_millis(),
                command: json!({"seq": seq}),
            };
            agent
                .send(&Message::from(action).to_bytes().unwrap())
                .unwrap();
        }

        let mut latest = None;
        for _ in 0..50 {
            if let Some(a) = link.peek_action() {
                if a.seq == 4 {
                    latest = Some(a);
                    break;
                }
            }
            thread::sleep(Duration::from_millis(5));
        }
        let latest = latest.expect("should have received actions");
        assert_eq!(latest.seq, 4);
    }

    #[test]
    fn detects_agent_disconnect() {
        let (link, agent) = connect_pair();
        assert!(link.is_connected());

        drop(agent);
        thread::sleep(Duration::from_millis(50));
        assert!(!link.is_connected());
    }
}
