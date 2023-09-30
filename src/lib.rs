use std::net::SocketAddr;

use tokio::{
    io::{copy_bidirectional, AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};

type IOResult<T> = std::io::Result<T>;

pub const VER: u8 = 0x5;
pub const NO_AUTH: u8 = 0x0;
pub const OK: u8 = 0x0;
pub const CONNECT: u8 = 0x1;
pub const RSV: u8 = 0x0;
pub const IPV4: u8 = 0x1;
pub const UNSPECIFIED_SOCKET_ADDR: [u8; 6] = [0x0, 0x0, 0x0, 0x0, 0x0, 0x0];

pub async fn start(port: u16) -> IOResult<()> {
    let server = TcpListener::bind(format!("127.0.0.1:{port}")).await?;
    loop {
        tokio::spawn(run(server.accept().await?));
    }
}

async fn run((mut downstream, _): (TcpStream, SocketAddr)) -> IOResult<()> {
    // Negotiation
    let _version = downstream.read_u8().await?;
    let _nmethods = downstream.read_u8().await? as usize;
    let _methods = downstream
        .read_exact(vec![0u8; _nmethods].as_mut_slice())
        .await?;
    downstream.write_all(&[VER, OK]).await?;

    // Connect
    let _version = downstream.read_u8().await?;
    let _cmd = downstream.read_u8().await?;
    let _rsv = downstream.read_u8().await?;
    let _atype = downstream.read_u8().await?;
    let ip = downstream.read_u32().await?;
    let port = downstream.read_u16().await?;
    let addr = SocketAddr::from((ip.to_be_bytes(), port));
    downstream.write_all(&[VER, OK, RSV, IPV4]).await?;
    downstream.write_all(&UNSPECIFIED_SOCKET_ADDR).await?;

    // Send
    let mut upstream = TcpStream::connect(addr).await?;
    _ = copy_bidirectional(&mut upstream, &mut downstream).await?;
    Ok(())
}
