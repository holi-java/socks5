use std::{
    future::Future,
    io::{self, ErrorKind},
    mem::MaybeUninit,
    net::{SocketAddr, ToSocketAddrs},
    pin::Pin,
    task::{Context, Poll},
};

use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWriteExt, ReadBuf},
    net::TcpStream,
};

const VER: u8 = 0x5;
const NO_AUTH: u8 = 0x0;
const OK: u8 = 0x0;
const CONNECT: u8 = 0x1;
const RSV: u8 = 0x0;
const IPV4: u8 = 0x1;
const UNSPECIFIED_SOCKET_ADDR: [u8; 6] = [0x0, 0x0, 0x0, 0x0, 0x0, 0x0];

#[tokio::test]
async fn no_auth() {
    let port = 1081;
    tokio::spawn(socks5::start(port));
    _ = tokio::spawn(async {}).await;
    let mut client = TcpStream::connect(format!("127.0.0.1:{port}"))
        .await
        .unwrap();

    // Negotiation
    client.write_all(&[VER, 1, NO_AUTH]).await.unwrap();
    assert_eq!(client.read_exact_bytes().await.unwrap(), [VER, OK]);

    // Connect
    client.write_all(&[VER, CONNECT, RSV, IPV4]).await.unwrap();
    client
        .write_all(resolve("www.baidu.com:80").unwrap().as_slice())
        .await
        .unwrap();
    let response = client.read_exact_bytes::<10>().await.unwrap();
    assert_eq!(response[..4], [VER, OK, RSV, IPV4]);
    assert_eq!(response[4..], UNSPECIFIED_SOCKET_ADDR);

    // Send
    let data = b"\
        GET / HTTP/1.1\r\n\
        Host: www.baidu.com\r\n\
        User-Agent: curl/7.68.0\r\n\
        Accept: */*\r\n\r\n\r\n\
    ";
    client.write_all(data).await.unwrap();
    let mut response = String::new();
    client.read_to_string(&mut response).await.unwrap();
    assert!(response.contains("百度一下"), "{}", response);
}

fn resolve<T: ToSocketAddrs>(addr: T) -> io::Result<[u8; 6]> {
    addr.to_socket_addrs()?
        .find(SocketAddr::is_ipv4)
        .ok_or_else(|| io::Error::new(io::ErrorKind::Other, "Empty"))
        .map(|addr| {
            if let SocketAddr::V4(addr) = addr {
                let mut out = [0; 6];
                const BOUNDARY: usize = 4;
                out[..BOUNDARY].copy_from_slice(&addr.ip().octets());
                out[BOUNDARY..].copy_from_slice(&addr.port().to_be_bytes());
                return out;
            }
            unreachable!()
        })
}

trait AsyncExactRead {
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

struct ReadExactBytes<'a, const N: usize, T: ?Sized> {
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
