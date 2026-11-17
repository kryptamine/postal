use std::{
    io,
    pin::Pin,
    task::{Context, Poll},
};

use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

use super::wire_trace::WireTrace;

/// A transport that copies what crosses it into the wire trace.
#[derive(Debug)]
pub struct Traced<T> {
    inner: T,
    trace: WireTrace,
}

impl<T> Traced<T> {
    pub fn new(inner: T) -> Self {
        Self {
            inner,
            trace: WireTrace::default(),
        }
    }
}

impl<T: AsyncRead + Unpin> AsyncRead for Traced<T> {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        let before = buf.filled().len();
        let poll = Pin::new(&mut this.inner).poll_read(cx, buf);
        if matches!(poll, Poll::Ready(Ok(()))) {
            this.trace.received(&buf.filled()[before..]);
        }

        poll
    }
}

impl<T: AsyncWrite + Unpin> AsyncWrite for Traced<T> {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        data: &[u8],
    ) -> Poll<io::Result<usize>> {
        let this = self.get_mut();
        let poll = Pin::new(&mut this.inner).poll_write(cx, data);
        if let Poll::Ready(Ok(written)) = poll {
            this.trace.sent(&data[..written]);
        }

        poll
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.get_mut().inner).poll_flush(cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.get_mut().inner).poll_shutdown(cx)
    }
}
