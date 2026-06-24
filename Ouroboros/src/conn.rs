//! 게임과의 TCP 통신 계층.
//!
//! TCP는 메시지 경계가 없는 스트림이므로, 4바이트 빅엔디언 길이 접두사로
//! 메시지를 프레이밍한다 (length-prefix framing). 작은 고빈도 패킷이 Nagle
//! 알고리즘에 의해 묶이지 않도록 `TCP_NODELAY`를 켠다.
//!
//! 페이로드는 raw 바이트로 다룬다. 직렬화 포맷(JSON 등)은 상위 계층이 정한다.

use std::io::{self, Read, Write};
use std::net::{TcpListener, TcpStream, ToSocketAddrs};

/// 단일 메시지 페이로드의 최대 크기 (바이트). 손상된 길이 접두사로 인한
/// 비정상적 메모리 할당을 막기 위한 안전장치.
const MAX_PAYLOAD_LEN: usize = 16 * 1024 * 1024;

/// 게임과의 프레임 단위 TCP 연결.
///
/// `send`/`recv`는 length-prefix로 구분된 하나의 완전한 메시지를 단위로 동작한다.
pub struct Connection {
    stream: TcpStream,
}

impl Connection {
    /// 게임 서버에 클라이언트로 접속한다.
    pub fn connect<A: ToSocketAddrs>(addr: A) -> io::Result<Self> {
        let stream = TcpStream::connect(addr)?;
        Self::from_stream(stream)
    }

    /// 이미 수립된 스트림을 프레임 연결로 감싼다 (서버 측 accept 등).
    pub fn from_stream(stream: TcpStream) -> io::Result<Self> {
        // 고빈도 작은 패킷의 지연을 막기 위해 Nagle 비활성화.
        stream.set_nodelay(true)?;
        Ok(Self { stream })
    }

    /// 하나의 메시지를 전송한다. `[u32 길이(BE)][페이로드]` 형식.
    pub fn send(&mut self, payload: &[u8]) -> io::Result<()> {
        if payload.len() > MAX_PAYLOAD_LEN {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("payload too large: {} bytes", payload.len()),
            ));
        }
        let len = payload.len() as u32;
        self.stream.write_all(&len.to_be_bytes())?;
        self.stream.write_all(payload)?;
        self.stream.flush()
    }

    /// 하나의 완전한 메시지를 수신한다. 상대가 정상 종료하면 `None`.
    pub fn recv(&mut self) -> io::Result<Option<Vec<u8>>> {
        let mut len_buf = [0u8; 4];
        // 첫 바이트를 읽기 전 EOF면 정상 종료로 간주.
        match self.stream.read_exact(&mut len_buf) {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
            Err(e) => return Err(e),
        }
        let len = u32::from_be_bytes(len_buf) as usize;
        if len > MAX_PAYLOAD_LEN {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("declared payload length too large: {len} bytes"),
            ));
        }
        let mut payload = vec![0u8; len];
        self.stream.read_exact(&mut payload)?;
        Ok(Some(payload))
    }

    /// 내부 스트림의 참조. 타임아웃 설정 등 세부 제어용.
    pub fn stream(&self) -> &TcpStream {
        &self.stream
    }
}

/// 게임 서버 측 리스너. 들어오는 연결을 `Connection`으로 감싸 반환한다.
pub struct Listener {
    inner: TcpListener,
}

impl Listener {
    /// 주어진 주소에 바인딩한다.
    pub fn bind<A: ToSocketAddrs>(addr: A) -> io::Result<Self> {
        Ok(Self {
            inner: TcpListener::bind(addr)?,
        })
    }

    /// 단일 연결을 수락할 때까지 블로킹한다.
    pub fn accept(&self) -> io::Result<Connection> {
        let (stream, _peer) = self.inner.accept()?;
        Connection::from_stream(stream)
    }

    /// 바인딩된 실제 로컬 주소 (포트 0으로 바인딩한 경우 확인용).
    pub fn local_addr(&self) -> io::Result<std::net::SocketAddr> {
        self.inner.local_addr()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn roundtrip_frames() {
        let listener = Listener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();

        let server = thread::spawn(move || {
            let mut conn = listener.accept().unwrap();
            // 받은 메시지를 그대로 에코.
            while let Some(msg) = conn.recv().unwrap() {
                conn.send(&msg).unwrap();
            }
        });

        let mut client = Connection::connect(addr).unwrap();
        let payloads: [&[u8]; 3] = [b"hello", b"", b"observation:{hp:100}"];
        for p in payloads {
            client.send(p).unwrap();
            let echoed = client.recv().unwrap().unwrap();
            assert_eq!(echoed, p);
        }
        // 클라이언트 드롭 → 서버 recv가 None 받고 종료.
        drop(client);
        server.join().unwrap();
    }

    #[test]
    fn rejects_oversized_payload() {
        let listener = Listener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let _server = thread::spawn(move || {
            let _conn = listener.accept().unwrap();
            thread::sleep(std::time::Duration::from_millis(50));
        });

        let mut client = Connection::connect(addr).unwrap();
        let huge = vec![0u8; MAX_PAYLOAD_LEN + 1];
        let err = client.send(&huge).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
    }
}
