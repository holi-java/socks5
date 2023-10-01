use std::io::{self, ErrorKind};
use std::mem::MaybeUninit;

use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::io::{AsyncRead, ReadBuf};

pub trait AsyncExactRead {
    fn read_exact_bytes<const N: usize>(&mut self) -> ReadExactBytes<N, Self>
    where
        Self: Unpin,
    {
        ReadExactBytes {
            inner: self,
            buf: [MaybeUninit::uninit(); N],
            offset: 0,
        }
    }
}

impl<T: AsyncRead> AsyncExactRead for T {}

pub struct ReadExactBytes<'a, const N: usize, T: ?Sized> {
    inner: &'a mut T,
    buf: [MaybeUninit<u8>; N],
    offset: usize,
}

impl<const N: usize, T> Future for ReadExactBytes<'_, N, T>
where
    T: AsyncRead + Unpin,
{
    type Output = io::Result<[u8; N]>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        let this = unsafe { self.get_unchecked_mut() };
        let mut buf = ReadBuf::uninit(&mut this.buf[this.offset..]);
        while buf.remaining() > 0 {
            match Pin::new(&mut *this.inner).poll_read(cx, &mut buf) {
                Poll::Ready(Ok(_)) => (),
                Poll::Ready(Err(e)) if e.kind() != ErrorKind::WouldBlock => {
                    return Poll::Ready(Err(e))
                }
                _ => {
                    let filled = N - this.offset - buf.remaining();
                    this.offset += filled;
                    return Poll::Pending;
                }
            }
        }

        // read the stacked `Copy` value is safety
        let bytes = unsafe { this.buf.as_ptr().cast::<[u8; N]>().read() };
        Poll::Ready(Ok(bytes))
    }
}
