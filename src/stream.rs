use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio_io::{AsyncRead, AsyncWrite};

#[cfg(feature = "rustls")]
use tokio_rustls::client::TlsStream;
#[cfg(feature = "tls")]
use tokio_tls::TlsStream;

/// A Proxy Stream wrapper
pub enum ProxyStream<R> {
    Regular(R),
    #[cfg(any(feature = "tls", feature = "rustls"))]
    Secured(TlsStream<R>),
}

macro_rules! match_fn {
    ($self:expr, $fn:ident $(,$buf:expr)*) => {
        match *$self {
            ProxyStream::Regular(ref mut s) => Pin::new(s).$fn($($buf),*),
            #[cfg(any(feature = "tls", feature = "rustls"))]
            ProxyStream::Secured(ref mut s) => Pin::new(s).$fn($($buf),*),
        }
    }
}

impl<R: AsyncRead + AsyncWrite + Unpin> AsyncRead for ProxyStream<R> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        match_fn!(self, poll_read, cx, buf)
    }
}

impl<R: AsyncRead + AsyncWrite + Unpin> AsyncWrite for ProxyStream<R> {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        match_fn!(self, poll_write, cx, buf)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        match_fn!(self, poll_flush, cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        match_fn!(self, poll_shutdown, cx)
    }
}
