mod constant;

use constant::*;
use std::net::SocketAddr;
use tokio::{
    io::{copy_bidirectional, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};

type IOResult<T> = std::io::Result<T>;

/// Just used as typealias only for bounded generic parameters.
trait Stream: AsyncRead + AsyncWrite + Unpin {}

impl<T: ?Sized> Stream for T where T: AsyncRead + AsyncWrite + Unpin {}

pub async fn start(port: u16) -> IOResult<()> {
    let server = TcpListener::bind(format!("127.0.0.1:{port}")).await?;
    loop {
        tokio::spawn(run(server.accept().await?.0));
    }
}

async fn run<S: Stream>(mut client: S) -> IOResult<()> {
    // Negotiation
    let _version = client.read_u8().await?;
    let _nmethods = client.read_u8().await? as usize;
    let _methods = client
        .read_exact(vec![0u8; _nmethods].as_mut_slice())
        .await?;
    client.write_all(&[VER, OK]).await?;

    // Connect
    let _version = client.read_u8().await?;
    let _cmd = client.read_u8().await?;
    let _rsv = client.read_u8().await?;
    let _atype = client.read_u8().await?;
    let ip = client.read_u32().await?;
    let port = client.read_u16().await?;
    let addr = SocketAddr::from((ip.to_be_bytes(), port));
    client.write_all(&[VER, OK, RSV, IPV4]).await?;
    client.write_all(&UNSPECIFIED_SOCKET_ADDR).await?;

    // Send
    let mut upstream = TcpStream::connect(addr).await?;
    _ = copy_bidirectional(&mut upstream, &mut client).await?;
    Ok(())
}
