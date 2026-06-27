use std::io::{self, Read, Write};
use std::net::{TcpListener, TcpStream, ToSocketAddrs};

const MAX_PAYLOAD_LEN: usize = 16 * 1024 * 1024;

pub struct Connection {
    stream: TcpStream,
}

impl Connection {
    pub fn from_stream(stream: TcpStream) -> io::Result<Self> {
        stream.set_nodelay(true)?;
        Ok(Self { stream })
    }

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

    pub fn recv(&mut self) -> io::Result<Option<Vec<u8>>> {
        let mut len_buf = [0u8; 4];
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

    pub fn try_clone(&self) -> io::Result<Self> {
        let cloned = self.stream.try_clone()?;
        Ok(Self { stream: cloned })
    }

    pub fn stream(&self) -> &TcpStream {
        &self.stream
    }
}

pub struct Listener {
    inner: TcpListener,
}

impl Listener {
    pub fn bind<A: ToSocketAddrs>(addr: A) -> io::Result<Self> {
        Ok(Self {
            inner: TcpListener::bind(addr)?,
        })
    }

    pub fn accept(&self) -> io::Result<Connection> {
        let (stream, _peer) = self.inner.accept()?;
        Connection::from_stream(stream)
    }

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
            while let Some(msg) = conn.recv().unwrap() {
                conn.send(&msg).unwrap();
            }
        });

        let stream = TcpStream::connect(addr).unwrap();
        let mut client = Connection::from_stream(stream).unwrap();
        let payloads: [&[u8]; 3] = [b"hello", b"", b"test payload"];
        for p in payloads {
            client.send(p).unwrap();
            let echoed = client.recv().unwrap().unwrap();
            assert_eq!(echoed, p);
        }
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

        let stream = TcpStream::connect(addr).unwrap();
        let mut client = Connection::from_stream(stream).unwrap();
        let huge = vec![0u8; MAX_PAYLOAD_LEN + 1];
        let err = client.send(&huge).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
    }
}
