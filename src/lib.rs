#[macro_use]
mod extract;
mod connect;
mod constant;
mod error;
mod marker;
mod negotiation;
#[cfg(test)]
mod test;

use connect::Connect;
use error::Error;
use marker::Stream;
use negotiation::Negotiation;
use tokio::{
    io::copy_bidirectional,
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
    let client = negotiation.run().await?;

    let connect = Connect(client);
    let (mut client, addr) = connect.run().await?;

    // Send
    let mut upstream = TcpStream::connect(addr).await?;
    _ = copy_bidirectional(&mut upstream, &mut client).await?;
    Ok(())
}
