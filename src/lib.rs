#[macro_use]
mod extract;
mod connect;
mod constant;
mod credential;
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
use credential::Credential;
use error::Error;
use forward::Forward;
use marker::{Stream, UnpinAsyncRead};
use negotiation::Negotiation;
use tokio::{net::{TcpListener, TcpStream, ToSocketAddrs}, io::AsyncReadExt};

type Result<T> = std::result::Result<T, Error>;
type IOResult<T> = std::io::Result<T>;
type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

pub async fn start(port: u16) -> Result<()> {
    let server = TcpListener::bind(format!("127.0.0.1:{port}")).await?;
    loop {
        tokio::spawn(Sock5::starts(server.accept().await?.0));
    }
}

#[derive(Debug)]
enum Sock5<U = TcpStream> {
    Negotiation(Negotiation),
    Authentication(Credential),
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
            Sock5::Negotiation(stage) => Continue(stage.run(client).await),
            Sock5::Authentication(credential) => Continue(credential.run(client).await),
            Sock5::Connect(stage) => Continue(stage.run(client).await),
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

async fn read_vec_u8<R: UnpinAsyncRead>(mut client: R, n: usize) -> IOResult<Vec<u8>> {
    let mut buf = unsafe {
        let mut buf = Vec::with_capacity(n);
        #[allow(clippy::uninit_vec)]
        buf.set_len(n);
        buf
    };
    client.read_exact(&mut buf).await?;
    Ok(buf)
}

