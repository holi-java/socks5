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
use tokio::{
    io::AsyncReadExt,
    net::{TcpListener, TcpStream, ToSocketAddrs},
};

type Result<T> = std::result::Result<T, Error>;
type IOResult<T> = std::io::Result<T>;
type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

pub async fn start(port: u16) -> IOResult<()> {
    let server = TcpListener::bind(format!("127.0.0.1:{port}")).await?;
    loop {
        tokio::spawn(Socks5::<TcpStream>::new().start(server.accept().await?.0));
    }
}

pub struct Socks5<U> {
    stage: Stage<U>,
}

impl<U> Socks5<U> {
    pub fn new() -> Self {
        Socks5 {
            stage: Stage::Negotiation(Negotiation),
        }
    }
}

impl<'a, U> Socks5<U>
where
    U: for<'b> Upstream<'b> + Stream,
    <U as Upstream<'a>>::Output: Future<Output = IOResult<U>>,
{
    pub async fn start<S: Stream>(mut self, client: S) -> IOResult<()> {
        self.process(client).await
    }

    pub async fn process<S: Stream>(&mut self, mut client: S) -> IOResult<()> {
        match self.try_process(&mut client).await {
            Err(err) => err.write(&mut client).await,
            Ok(_) => Ok(()),
        }
    }

    pub async fn try_process<S: Stream>(&mut self, mut client: S) -> Result<()> {
        while let Continue(result) = self.run(&mut client).await {
            result?;
        }
        Ok(())
    }

    async fn run<S: Stream>(&mut self, client: S) -> ControlFlow<(), Result<()>> {
        macro_rules! try_await {
            ($future: expr) => {
                match $future.await {
                    Ok(value) => value,
                    Err(err) => return Continue(Err(Error::from(err))),
                }
            };
        }

        self.stage = match &mut self.stage {
            Stage::Negotiation(stage) => try_await!(stage.run(client)),
            Stage::Authentication(stage) => try_await!(stage.run(client)),
            Stage::Connect(stage) => try_await!(stage.run(client)),
            Stage::Forward(stage) => return Break(try_await!(stage.run(client))),
        };
        Continue(Ok(()))
    }
}

#[derive(Debug)]
enum Stage<U = TcpStream> {
    Negotiation(Negotiation),
    Authentication(Credential),
    Connect(Connect),
    Forward(Forward<U>),
}

pub trait Upstream<'a> {
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
