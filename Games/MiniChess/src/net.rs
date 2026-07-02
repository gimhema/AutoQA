//! 2인 플레이용 TCP 네트워크 계층.
//!
//! host가 bind/accept하고 guest가 connect한다. 두 피어는 length-prefix로 프레이밍된
//! JSON 메시지를 주고받는다 (`conn.rs`와 동일한 4바이트 빅엔디언 길이 접두사).
//!
//! 프로토콜:
//! - host → guest: [`Msg::Config`] (필드 크기·Pawn 개수). 접속 직후 1회.
//! - 양방향: [`Msg::Move`] (이동). 자기 턴에 검증된 이동을 상대에게 알린다.
//! - 양방향: [`Msg::Quit`] (종료 통보).

use std::io::{self, Read, Write};
use std::net::{TcpListener, TcpStream, ToSocketAddrs};

use serde_json::{json, Value};

use crate::game::{Config, Pos};

/// 페이로드 최대 크기 방어.
const MAX_PAYLOAD_LEN: usize = 1024 * 1024;

/// 와이어 메시지.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Msg {
    /// host가 정한 게임 설정.
    Config(Config),
    /// 이동: `from` → `to`.
    Move { from: Pos, to: Pos },
    /// 종료 통보.
    Quit,
}

impl Msg {
    fn to_value(&self) -> Value {
        match self {
            Msg::Config(c) => json!({
                "type": "config",
                "width": c.width, "height": c.height, "pawns": c.pawns
            }),
            Msg::Move { from, to } => json!({
                "type": "move",
                "fx": from.x, "fy": from.y, "tx": to.x, "ty": to.y
            }),
            Msg::Quit => json!({ "type": "quit" }),
        }
    }

    fn from_value(v: &Value) -> io::Result<Msg> {
        let bad = |m: &str| io::Error::new(io::ErrorKind::InvalidData, m.to_string());
        let get_i32 = |key: &str| -> io::Result<i32> {
            v.get(key)
                .and_then(|x| x.as_i64())
                .map(|x| x as i32)
                .ok_or_else(|| bad(&format!("missing/invalid field: {key}")))
        };
        match v.get("type").and_then(|t| t.as_str()) {
            Some("config") => Ok(Msg::Config(Config {
                width: get_i32("width")?,
                height: get_i32("height")?,
                pawns: get_i32("pawns")?,
            })),
            Some("move") => Ok(Msg::Move {
                from: Pos::new(get_i32("fx")?, get_i32("fy")?),
                to: Pos::new(get_i32("tx")?, get_i32("ty")?),
            }),
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

    #[test]
    fn msg_roundtrip_values() {
        for msg in [
            Msg::Config(Config { width: 8, height: 8, pawns: 5 }),
            Msg::Move { from: Pos::new(1, 2), to: Pos::new(1, 3) },
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

        // 재바인딩 레이스를 피하려고 host를 먼저 스레드에서 띄운다.
        let host = thread::spawn(move || {
            let mut peer = Peer::host(addr).unwrap();
            peer.send(&Msg::Config(Config { width: 5, height: 5, pawns: 2 })).unwrap();
            // guest의 move를 받는다.
            peer.recv().unwrap().unwrap()
        });

        // host가 bind할 시간을 준다.
        let mut guest = loop {
            match Peer::join(addr) {
                Ok(p) => break p,
                Err(_) => thread::sleep(std::time::Duration::from_millis(10)),
            }
        };

        let config_msg = guest.recv().unwrap().unwrap();
        assert_eq!(config_msg, Msg::Config(Config { width: 5, height: 5, pawns: 2 }));

        guest.send(&Msg::Move { from: Pos::new(2, 4), to: Pos::new(2, 3) }).unwrap();
        let received = host.join().unwrap();
        assert_eq!(received, Msg::Move { from: Pos::new(2, 4), to: Pos::new(2, 3) });
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
