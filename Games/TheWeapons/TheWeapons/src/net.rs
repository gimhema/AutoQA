//! 2인 플레이용 TCP 네트워크 계층 (MiniChess의 net.rs와 동일한 프레이밍).
//!
//! host가 bind/accept하고 guest가 connect한다. 두 피어는 length-prefix로 프레이밍된
//! JSON 메시지를 주고받는다 (4바이트 빅엔디언 길이 접두사).
//!
//! 프로토콜:
//! - host → guest: [`Msg::Config`] (세팅). 접속 직후 1회.
//! - 양방향: [`Msg::Play`] (이번 턴 소켓 배치). 이 게임은 동시 공개라 선후 구분이 없다 —
//!   양쪽 모두 자기 배치를 먼저 보내고 나서 상대 것을 받는다.
//! - 양방향: [`Msg::Quit`] (종료 통보).

use std::io::{self, Read, Write};
use std::net::{TcpListener, TcpStream, ToSocketAddrs};

use serde_json::{json, Value};

use crate::cards::Card;
use crate::game::Config;

/// 페이로드 최대 크기 방어.
const MAX_PAYLOAD_LEN: usize = 1024 * 1024;

/// 와이어 메시지.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Msg {
    /// host가 정한 게임 세팅.
    Config(Config),
    /// 이번 턴 소켓 배치. `None` = 빈 소켓.
    Play(Vec<Option<Card>>),
    /// 종료 통보.
    Quit,
}

fn card_to_str(card: Card) -> &'static str {
    match card {
        Card::Sword => "sword",
        Card::Shield => "shield",
        Card::Spear => "spear",
    }
}

fn card_from_str(s: &str) -> io::Result<Card> {
    match s {
        "sword" => Ok(Card::Sword),
        "shield" => Ok(Card::Shield),
        "spear" => Ok(Card::Spear),
        other => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("unknown card: {other}"),
        )),
    }
}

impl Msg {
    fn to_value(&self) -> Value {
        match self {
            Msg::Config(c) => json!({
                "type": "config",
                "socket_count": c.socket_count,
                "initial_hp": c.initial_hp,
                "sword_count": c.sword_count,
                "shield_count": c.shield_count,
                "spear_count": c.spear_count,
            }),
            Msg::Play(play) => json!({
                "type": "play",
                "slots": play.iter().map(|s| s.map(card_to_str)).collect::<Vec<_>>(),
            }),
            Msg::Quit => json!({ "type": "quit" }),
        }
    }

    fn from_value(v: &Value) -> io::Result<Msg> {
        let bad = |m: &str| io::Error::new(io::ErrorKind::InvalidData, m.to_string());
        match v.get("type").and_then(|t| t.as_str()) {
            Some("config") => {
                let get_u32 = |key: &str| -> io::Result<u32> {
                    v.get(key)
                        .and_then(|x| x.as_u64())
                        .map(|x| x as u32)
                        .ok_or_else(|| bad(&format!("missing/invalid field: {key}")))
                };
                let get_i32 = |key: &str| -> io::Result<i32> {
                    v.get(key)
                        .and_then(|x| x.as_i64())
                        .map(|x| x as i32)
                        .ok_or_else(|| bad(&format!("missing/invalid field: {key}")))
                };
                Ok(Msg::Config(Config {
                    socket_count: get_u32("socket_count")? as usize,
                    initial_hp: get_i32("initial_hp")?,
                    sword_count: get_u32("sword_count")?,
                    shield_count: get_u32("shield_count")?,
                    spear_count: get_u32("spear_count")?,
                }))
            }
            Some("play") => {
                let slots = v
                    .get("slots")
                    .and_then(|s| s.as_array())
                    .ok_or_else(|| bad("missing field: slots"))?;
                let mut play = Vec::with_capacity(slots.len());
                for slot in slots {
                    play.push(match slot {
                        Value::Null => None,
                        Value::String(s) => Some(card_from_str(s)?),
                        _ => return Err(bad("invalid slot value")),
                    });
                }
                Ok(Msg::Play(play))
            }
            Some("quit") => Ok(Msg::Quit),
            _ => Err(bad("unknown message type")),
        }
    }
}

/// 상대 피어와의 연결. length-prefix JSON 메시지 송수신.
pub struct Peer {
    stream: TcpStream,
}

impl Peer {
    /// host: 주소에 bind하고 guest 접속을 기다린다.
    pub fn host<A: ToSocketAddrs>(addr: A) -> io::Result<Self> {
        let listener = TcpListener::bind(addr)?;
        let (stream, _) = listener.accept()?;
        stream.set_nodelay(true)?;
        Ok(Self { stream })
    }

    /// guest: host에 접속한다.
    pub fn join<A: ToSocketAddrs>(addr: A) -> io::Result<Self> {
        let stream = TcpStream::connect(addr)?;
        stream.set_nodelay(true)?;
        Ok(Self { stream })
    }

    /// 메시지 하나를 전송한다.
    pub fn send(&mut self, msg: &Msg) -> io::Result<()> {
        let payload = serde_json::to_vec(&msg.to_value())
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        if payload.len() > MAX_PAYLOAD_LEN {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "payload too large"));
        }
        let len = payload.len() as u32;
        self.stream.write_all(&len.to_be_bytes())?;
        self.stream.write_all(&payload)?;
        self.stream.flush()
    }

    /// 메시지 하나를 수신한다. 상대가 연결을 닫으면 `None`.
    pub fn recv(&mut self) -> io::Result<Option<Msg>> {
        let mut len_buf = [0u8; 4];
        match self.stream.read_exact(&mut len_buf) {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
            Err(e) => return Err(e),
        }
        let len = u32::from_be_bytes(len_buf) as usize;
        if len > MAX_PAYLOAD_LEN {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "declared length too large"));
        }
        let mut payload = vec![0u8; len];
        self.stream.read_exact(&mut payload)?;
        let value: Value = serde_json::from_slice(&payload)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        Msg::from_value(&value).map(Some)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    fn test_config() -> Config {
        Config {
            socket_count: 3,
            initial_hp: 10,
            sword_count: 5,
            shield_count: 5,
            spear_count: 5,
        }
    }

    #[test]
    fn msg_roundtrip_values() {
        for msg in [
            Msg::Config(test_config()),
            Msg::Play(vec![Some(Card::Sword), None, Some(Card::Spear)]),
            Msg::Quit,
        ] {
            let v = msg.to_value();
            assert_eq!(Msg::from_value(&v).unwrap(), msg);
        }
    }

    #[test]
    fn host_and_join_exchange_messages() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        drop(listener); // 포트만 얻고 host가 다시 bind하도록.

        let host = thread::spawn(move || {
            let mut peer = Peer::host(addr).unwrap();
            peer.send(&Msg::Config(test_config())).unwrap();
            peer.recv().unwrap().unwrap()
        });

        let mut guest = loop {
            match Peer::join(addr) {
                Ok(p) => break p,
                Err(_) => thread::sleep(std::time::Duration::from_millis(10)),
            }
        };

        let config_msg = guest.recv().unwrap().unwrap();
        assert_eq!(config_msg, Msg::Config(test_config()));

        guest
            .send(&Msg::Play(vec![Some(Card::Shield), None, None]))
            .unwrap();
        let received = host.join().unwrap();
        assert_eq!(received, Msg::Play(vec![Some(Card::Shield), None, None]));
    }

    #[test]
    fn recv_returns_none_on_disconnect() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();

        let server = thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let mut peer = Peer { stream };
            peer.recv().unwrap()
        });

        let client = TcpStream::connect(addr).unwrap();
        drop(client);
        assert_eq!(server.join().unwrap(), None);
    }
}
