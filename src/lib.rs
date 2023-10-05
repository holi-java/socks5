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
    ops::ControlFlow::{self, *},
    pin::Pin,
};

use connect::Connect;
use core::future::Future;
use error::Error;
use forward::Forward;
use marker::Stream;
use negotiation::Negotiation;
use tokio::net::{TcpListener, TcpStream, ToSocketAddrs};

type Result<T> = std::result::Result<T, Error>;
type IOResult<T> = std::io::Result<T>;
type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

pub async fn start(port: u16) -> Result<()> {
    let server = TcpListener::bind(format!("127.0.0.1:{port}")).await?;
    loop {
        tokio::spawn(Sock5::starts(server.accept().await?.0));
    }
}

enum Sock5<U = TcpStream> {
    Negotiation(Negotiation),
    Connect(Connect),
    Forward(Forward<U>),
}

impl Sock5 {
    pub async fn starts<S: Stream>(client: S) -> IOResult<()> {
        Self::process(client).await
    }
}

impl<'a, U> Sock5<U>
where
    U: for<'b> Upstream<'b> + Stream,
    <U as Upstream<'a>>::Output: Future<Output = IOResult<U>>,
{
    pub async fn process<S: Stream>(mut client: S) -> IOResult<()> {
        match Self::try_process(&mut client).await {
            Err(err) => err.write(&mut client).await,
            Ok(_) => Ok(()),
        }
    }

    pub async fn try_process<S: Stream>(mut client: S) -> Result<()> {
        let mut stage = Continue(Ok(Self::Negotiation(Negotiation)));
        while let Continue(current) = stage {
            stage = current?.run(&mut client).await;
        }
        Ok(())
    }

    async fn run<S: Stream>(self, client: S) -> ControlFlow<(), Result<Self>> {
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
                try_await!(stage.run(client));
                Continue(Ok(Self::Connect(Connect)))
            }
            Sock5::Connect(stage) => {
                let addr = try_await!(stage.run(client));
                let upstream = try_await!(U::connect(addr));
                Continue(Ok(Self::Forward(Forward(upstream))))
            }
            Sock5::Forward(stage) => Break(try_await!(stage.run(client))),
        }
    }
}

trait Upstream<'a> {
    type Output;

    fn connect<S: ToSocketAddrs + Send + 'a>(addr: S) -> Self::Output;
}

impl<'a> Upstream<'a> for TcpStream {
    type Output = BoxFuture<'a, IOResult<Self>>;

    fn connect<S: ToSocketAddrs + Send + 'a>(addr: S) -> Self::Output {
        Box::pin(TcpStream::connect(addr))
    }
}
