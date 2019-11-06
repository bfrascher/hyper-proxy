use super::io_err;
use http::HeaderMap;
use hyper::client::connect::Connected;
use std::fmt::{self, Display, Formatter};
use std::io;
use tokio_io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

pub(crate) struct TunnelConnect {
    buf: Vec<u8>,
}

impl TunnelConnect {
    /// Change stream
    pub async fn with_stream<S: AsyncRead + AsyncWrite + Unpin>(
        mut self,
        mut stream: S,
        connected: Connected,
    ) -> io::Result<(S, Connected)> {
        stream.write_all(&self.buf).await?;
        self.buf.truncate(0);
        stream.read_to_end(&mut self.buf).await?;
        if (self.buf.starts_with(b"HTTP/1.1 200") || self.buf.starts_with(b"HTTP/1.0 200"))
            && self.buf.ends_with(b"\r\n\r\n")
        {
            Ok((stream, connected.proxy(true)))
        } else {
            Err(io_err("unsuccessful tunnel"))
        }
    }
}

struct HeadersDisplay<'a>(&'a HeaderMap);

impl<'a> Display for HeadersDisplay<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), fmt::Error> {
        for (key, value) in self.0 {
            let value_str = value.to_str().map_err(|_| fmt::Error)?;
            write!(f, "{}: {}\r\n", key.as_str(), value_str)?;
        }

        Ok(())
    }
}

/// Creates a new tunnel through proxy
pub(crate) fn new(host: &str, port: u16, headers: &HeaderMap) -> TunnelConnect {
    let buf = format!(
        "CONNECT {0}:{1} HTTP/1.1\r\n\
         Host: {0}:{1}\r\n\
         {2}\
         \r\n",
        host,
        port,
        HeadersDisplay(headers)
    )
    .into_bytes();

    TunnelConnect { buf }
}

#[cfg(test)]
mod tests {
    use super::{Connected, HeaderMap};
    use std::io::{self, Read, Write};
    use std::net::{SocketAddr, TcpListener};
    use std::thread;
    use tokio::net::tcp::TcpStream;
    use tokio::runtime::current_thread::Runtime;

    async fn work(addr: &SocketAddr) -> io::Result<()> {
        let tcp = TcpStream::connect(addr).await?;
        let host = addr.ip().to_string();
        super::new(&host, addr.port(), &HeaderMap::new())
            .with_stream(tcp, Connected::new())
            .await?;
        Ok(())
    }

    #[cfg_attr(rustfmt, rustfmt_skip)]
    macro_rules! mock_tunnel {
        () => {{
            mock_tunnel!(
                b"\
                HTTP/1.1 200 OK\r\n\
                \r\n\
                "
            )
        }};
        ($write:expr) => {{
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let addr = listener.local_addr().unwrap();
            let connect_expected = format!(
                "\
                 CONNECT {0}:{1} HTTP/1.1\r\n\
                 Host: {0}:{1}\r\n\
                 \r\n\
                 ",
                addr.ip(),
                addr.port()
            ).into_bytes();

            thread::spawn(move || {
                let (mut sock, _) = listener.accept().unwrap();
                let mut buf = [0u8; 4096];
                let n = sock.read(&mut buf).unwrap();
                assert_eq!(&buf[..n], &connect_expected[..]);

                sock.write_all($write).unwrap();
            });
            addr
        }};
    }

    #[test]
    fn test_tunnel() {
        let addr = mock_tunnel!();

        let mut core = Runtime::new().unwrap();
        assert!(core.block_on(work(&addr)).is_ok());
    }

    #[test]
    fn test_tunnel_eof() {
        let addr = mock_tunnel!(b"HTTP/1.1 200 OK");

        let mut core = Runtime::new().unwrap();
        assert!(core.block_on(work(&addr)).is_err());
    }

    #[test]
    fn test_tunnel_bad_response() {
        let addr = mock_tunnel!(b"foo bar baz hallo");

        let mut core = Runtime::new().unwrap();
        assert!(core.block_on(work(&addr)).is_err())
    }
}
