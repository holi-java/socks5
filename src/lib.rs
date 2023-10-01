#[macro_use]
mod extract;
mod constant;
mod error;
mod marker;
mod negotiation;
#[cfg(test)]
mod test;

use constant::*;
use error::Error;
use marker::Stream;
use negotiation::Negotiation;
use std::net::SocketAddr;
use tokio::{
    io::{copy_bidirectional, AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};

type Result<T> = std::result::Result<T, Error>;

pub async fn start(port: u16) -> Result<()> {
    let server = TcpListener::bind(format!("127.0.0.1:{port}")).await?;
    loop {
        tokio::spawn(run(server.accept().await?.0));
    }
}

async fn run<S: Stream>(client: S) -> Result<()> {
    let negotiation = Negotiation(client);
    let mut client = negotiation.run().await?;

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
