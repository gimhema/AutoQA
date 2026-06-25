//! 게임과의 고수준 인터페이스.
//!
//! [`crate::conn`](전송)과 [`crate::conn_message`](스키마) 위에 얹혀, 빠른 루프가
//! 쓰기 좋은 API를 제공한다:
//! - **액션 전송**: `seq`를 자동 증가시키며 [`Action`]을 보낸다.
//! - **최신 관측 조회**: 백그라운드 리더 스레드가 들어오는 [`Observation`]을 계속
//!   수신해 "최신 슬롯"만 갱신한다. 빠른 루프는 블로킹 없이 이 슬롯을 읽으며,
//!   밀린(stale) 관측은 자연히 버려진다 (HoL blocking 완화).
//!
//! 읽기/쓰기를 분리하기 위해 TCP 스트림을 `try_clone`한다. 리더 스레드는 읽기
//! 전용 [`Connection`]을, 메인은 쓰기 전용 [`Connection`]을 소유한다.

use std::io;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

use serde_json::Value;

use crate::conn::Connection;
use crate::conn_message::{Action, Message, Observation};

/// 리더 스레드와 메인이 공유하는 상태.
struct Shared {
    /// 지금까지 본 가장 신선한 관측값 (seq 최대). 아직 없으면 `None`.
    latest: Mutex<Option<Observation>>,
    /// 리더 스레드가 살아 있는지. 연결 종료/오류 시 false.
    alive: AtomicBool,
}

/// 게임과의 연결을 관리하는 고수준 핸들.
pub struct GameInterface {
    writer: Connection,
    /// 다음 액션에 부여할 시퀀스 번호.
    next_seq: AtomicU64,
    shared: Arc<Shared>,
    reader: Option<JoinHandle<()>>,
}

impl GameInterface {
    /// 게임 서버에 접속하고 백그라운드 관측 수신을 시작한다.
    pub fn connect<A: std::net::ToSocketAddrs>(addr: A) -> io::Result<Self> {
        let writer = Connection::connect(addr)?;
        Self::from_connection(writer)
    }

    /// 이미 수립된 연결로부터 인터페이스를 구성한다.
    ///
    /// 읽기 전용 복제 스트림으로 리더 스레드를 띄운다.
    pub fn from_connection(writer: Connection) -> io::Result<Self> {
        let read_stream = writer.stream().try_clone()?;
        let reader_conn = Connection::from_stream(read_stream)?;

        let shared = Arc::new(Shared {
            latest: Mutex::new(None),
            alive: AtomicBool::new(true),
        });

        let reader = {
            let shared = Arc::clone(&shared);
            thread::spawn(move || reader_loop(reader_conn, shared))
        };

        Ok(Self {
            writer,
            next_seq: AtomicU64::new(0),
            shared,
            reader: Some(reader),
        })
    }

    /// 액션을 전송한다. `seq`는 내부에서 자동 부여된다.
    pub fn send_action(&mut self, command: Value) -> io::Result<()> {
        let seq = self.next_seq.fetch_add(1, Ordering::Relaxed);
        let msg = Message::from(Action::new(seq, command));
        let bytes = msg
            .to_bytes()
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        self.writer.send(&bytes)
    }

    /// 가장 신선한 관측값을 가져가며 슬롯을 비운다.
    ///
    /// 빠른 루프가 매 틱 호출하는 비차단(non-blocking) 진입점. 새 관측이 없으면
    /// `None`. 같은 관측을 두 번 행동에 쓰지 않도록 take 의미로 동작한다.
    pub fn take_latest_observation(&self) -> Option<Observation> {
        self.shared.latest.lock().unwrap().take()
    }

    /// 슬롯을 비우지 않고 최신 관측값을 복제해 본다 (폴링/디버그용).
    pub fn peek_latest_observation(&self) -> Option<Observation> {
        self.shared.latest.lock().unwrap().clone()
    }

    /// 리더 스레드가 아직 살아 있는지 (연결 유지 여부).
    pub fn is_alive(&self) -> bool {
        self.shared.alive.load(Ordering::Acquire)
    }
}

impl Drop for GameInterface {
    fn drop(&mut self) {
        // 리더 스레드가 블로킹 recv 중이면 종료를 깨우기 위해 소켓을 닫는다.
        let _ = self.writer.stream().shutdown(std::net::Shutdown::Both);
        if let Some(handle) = self.reader.take() {
            let _ = handle.join();
        }
    }
}

/// 리더 스레드 본체: 관측을 계속 수신해 더 신선한 것으로 슬롯을 갱신한다.
fn reader_loop(mut conn: Connection, shared: Arc<Shared>) {
    loop {
        match conn.recv() {
            Ok(Some(bytes)) => match Message::from_bytes(&bytes) {
                Ok(Message::Observation(obs)) => {
                    let mut slot = shared.latest.lock().unwrap();
                    // 순서 뒤바뀐(stale) 관측은 무시하고 더 신선한 것만 보관.
                    let newer = slot.as_ref().is_none_or(|cur| obs.seq > cur.seq);
                    if newer {
                        *slot = Some(obs);
                    }
                }
                // 게임이 액션을 되돌려보낼 일은 없지만, 와도 빠른 루프엔 무의미.
                Ok(Message::Action(_)) => {}
                // 깨진 페이로드 하나가 연결 전체를 죽이지 않도록 건너뛴다.
                Err(_) => {}
            },
            // 정상 종료.
            Ok(None) => break,
            // 소켓 오류(상대 종료, shutdown 등).
            Err(_) => break,
        }
    }
    shared.alive.store(false, Ordering::Release);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::conn::Listener;
    use crate::conn_message::now_millis;
    use serde_json::json;
    use std::time::Duration;

    /// 더 큰 seq 관측이 도착하면 latest 슬롯이 그것으로 갱신된다.
    #[test]
    fn keeps_freshest_observation() {
        let listener = Listener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();

        let game = thread::spawn(move || {
            let mut conn = listener.accept().unwrap();
            for seq in 0..5u64 {
                let obs = Observation::new(seq, json!({ "hp": 100 - seq }));
                conn.send(&Message::from(obs).to_bytes().unwrap()).unwrap();
            }
            // 에이전트가 다 받을 때까지 잠시 유지.
            thread::sleep(Duration::from_millis(100));
        });

        let agent = GameInterface::connect(addr).unwrap();

        // 리더 스레드가 5개를 처리할 시간을 준다.
        let mut latest = None;
        for _ in 0..50 {
            if let Some(o) = agent.peek_latest_observation() {
                if o.seq == 4 {
                    latest = Some(o);
                    break;
                }
            }
            thread::sleep(Duration::from_millis(5));
        }
        let latest = latest.expect("should have received observations");
        assert_eq!(latest.seq, 4);
        assert_eq!(latest.state["hp"], 96);

        drop(agent);
        game.join().unwrap();
    }

    /// take는 슬롯을 비우고, 같은 관측을 두 번 돌려주지 않는다.
    #[test]
    fn take_consumes_slot() {
        let listener = Listener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();

        let game = thread::spawn(move || {
            let mut conn = listener.accept().unwrap();
            conn.send(&Message::from(Observation::new(0, json!({ "x": 1 })))
                .to_bytes()
                .unwrap())
                .unwrap();
            thread::sleep(Duration::from_millis(100));
        });

        let agent = GameInterface::connect(addr).unwrap();
        let mut first = None;
        for _ in 0..50 {
            first = agent.take_latest_observation();
            if first.is_some() {
                break;
            }
            thread::sleep(Duration::from_millis(5));
        }
        assert!(first.is_some());
        // 두 번째 take는 비어 있어야 한다.
        assert!(agent.take_latest_observation().is_none());

        drop(agent);
        game.join().unwrap();
    }

    /// 에이전트가 보낸 액션을 게임 측이 정상 수신한다.
    #[test]
    fn sends_actions_with_incrementing_seq() {
        let listener = Listener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();

        let game = thread::spawn(move || {
            let mut conn = listener.accept().unwrap();
            let mut seqs = Vec::new();
            for _ in 0..3 {
                let bytes = conn.recv().unwrap().unwrap();
                if let Message::Action(a) = Message::from_bytes(&bytes).unwrap() {
                    seqs.push(a.seq);
                    assert!(a.timestamp_ms <= now_millis());
                }
            }
            seqs
        });

        let mut agent = GameInterface::connect(addr).unwrap();
        agent.send_action(json!({ "move": "forward" })).unwrap();
        agent.send_action(json!({ "fire": true })).unwrap();
        agent.send_action(json!({ "move": "back" })).unwrap();

        let seqs = game.join().unwrap();
        assert_eq!(seqs, vec![0, 1, 2]);
    }
}
