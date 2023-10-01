#[macro_use]
mod extract;
mod connect;
mod constant;
mod error;
mod forward;
mod marker;
mod negotiation;
#[cfg(test)]
mod test;

use connect::Connect;
use error::Error;
use forward::Forward;
use marker::Stream;
use negotiation::Negotiation;
use tokio::net::{TcpListener, TcpStream};

type Result<T> = std::result::Result<T, Error>;

pub async fn start(port: u16) -> Result<()> {
    let server = TcpListener::bind(format!("127.0.0.1:{port}")).await?;
    loop {
        tokio::spawn(run(server.accept().await?.0));
    }
}

async fn run<S: Stream>(client: S) -> Result<()> {
    let negotiation = Negotiation(client);
    let client = negotiation.run().await?;

    let connect = Connect(client);
    let (client, addr) = connect.run().await?;

    let upstream = TcpStream::connect(addr).await?;
    Forward(client, upstream).run().await?;
    Ok(())
}
