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

use std::{
    io,
    ops::ControlFlow::{self, *},
};

use connect::Connect;
use error::Error;
use forward::Forward;
use futures_util::{future::BoxFuture, Future};
use marker::Stream;
use negotiation::Negotiation;
use tokio::net::{TcpListener, TcpStream, ToSocketAddrs};

type Result<T> = std::result::Result<T, Error>;

pub async fn start(port: u16) -> Result<()> {
    let server = TcpListener::bind(format!("127.0.0.1:{port}")).await?;
    loop {
        tokio::spawn(Sock5::starts(server.accept().await?.0));
    }
}

enum Sock5<T, U = TcpStream> {
    Negotiation(Negotiation<T>),
    Connect(Connect<T>),
    Forward(Forward<T, U>),
}

impl<T: Stream> Sock5<T> {
    pub async fn starts(stream: T) -> Result<()> {
        Self::process(stream).await
    }
}

impl<'a, T: Stream, U> Sock5<T, U>
where
    U: for<'b> Upstream<'b> + Stream,
    <U as Upstream<'a>>::Output: Future<Output = io::Result<U>>,
{
    pub async fn process(stream: T) -> Result<()> {
        let mut stage = Continue(Ok(Self::Negotiation(Negotiation(stream))));
        while let Continue(current) = stage {
            stage = current?.run().await;
        }
        Ok(())
    }

    async fn run(self) -> ControlFlow<(), Result<Self>> {
        macro_rules! try_await {
            ($future: expr) => {
                match $future.await {
                    Ok(value) => value,
                    Err(err) => return Continue(Err(Error::from(err))),
                }
            };
        }

        match self {
            Sock5::Negotiation(stage) => {
                Continue(Ok(Self::Connect(Connect(try_await!(stage.run())))))
            }
            Sock5::Connect(stage) => {
                let (client, addr) = try_await!(stage.run());
                let upstream = try_await!(U::connect(addr));
                Continue(Ok(Self::Forward(Forward(client, upstream))))
            }
            Sock5::Forward(stage) => Break(try_await!(stage.run())),
        }
    }
}

trait Upstream<'a> {
    type Output;

    fn connect<S: ToSocketAddrs + Send + 'a>(addr: S) -> Self::Output;
}

impl<'a> Upstream<'a> for TcpStream {
    type Output = BoxFuture<'a, io::Result<Self>>;

    fn connect<S: ToSocketAddrs + Send + 'a>(addr: S) -> Self::Output {
        Box::pin(TcpStream::connect(addr))
    }
}
