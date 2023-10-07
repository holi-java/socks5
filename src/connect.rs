use std::{future::Future, net::SocketAddr};

use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::{
    constant::{CONNECT, IPV4, OK, RSV, UNSPECIFIED_SOCKET_ADDR, VER},
    error::Error::*,
    extract::{try_extract_rsv, try_extract_version},
    forward::Forward,
    marker::{Stream, UnpinAsyncRead},
    IOResult, Result, Socks5, Upstream,
};

#[derive(Debug)]
pub struct Connect;

impl Connect {
    pub async fn run<'a, S: Stream, U>(self, mut client: S) -> Result<Socks5<U>>
    where
        U: Upstream<'a>,
        U::Output: Future<Output = IOResult<U>>,
    {
        let addr = try_extract_addr(&mut client).await?;
        client.write_all(&[VER, OK, RSV, IPV4]).await?;
        client.write_all(&UNSPECIFIED_SOCKET_ADDR).await?;
        let upstream = U::connect(addr).await?;
        Ok(Socks5::Forward(Forward(upstream)))
    }
}

extract!(connect_cmd != CONNECT => BadCommand(connect_cmd));

async fn try_extract_addr<T: UnpinAsyncRead>(mut client: T) -> Result<SocketAddr> {
    _ = try_extract_version(&mut client).await?;
    _ = try_extract_connect_cmd(&mut client).await?;
    _ = try_extract_rsv(&mut client).await?;
    let _atype = client.read_u8().await?;
    let ip = client.read_u32().await?;
    let port = client.read_u16().await?;
    Ok(SocketAddr::from((ip.to_be_bytes(), port)))
}

#[cfg(test)]
mod tests {

    use tokio::{
        io::{duplex, AsyncWriteExt},
        net::TcpStream,
    };

    use crate::{
        connect::Connect,
        constant::{CONNECT, IPV4, OK, RSV, UNSPECIFIED_SOCKET_ADDR, VER},
        error::Error::*,
        test::AsyncExactRead,
        Socks5,
    };

    #[tokio::test]
    async fn connect() {
        let (mut client, mut server) = duplex(usize::MAX);
        let connect = Connect;
        client
            .write_all(&[VER, CONNECT, RSV, IPV4, 14, 119, 104, 254, 0, 80])
            .await
            .unwrap();

        let forward = connect.run::<_, TcpStream>(&mut server).await.unwrap();
        assert!(matches!(forward,
                Socks5::Forward(forward) if forward.0.peer_addr().unwrap() == "14.119.104.254:80".parse().unwrap()));

        let response = client.read_exact_bytes::<10>().await.unwrap();
        assert_eq!(response[..4], [VER, OK, RSV, IPV4]);
        assert_eq!(response[4..], UNSPECIFIED_SOCKET_ADDR);
    }

    #[tokio::test]
    async fn fails_with_bad_version() {
        let (mut client, mut server) = duplex(usize::MAX);
        let connect = Connect;
        client
            .write_all(&[0x6, CONNECT, RSV, IPV4, 1, 2, 3, 4, 5, 6])
            .await
            .unwrap();

        let err = connect.run::<_, TcpStream>(&mut server).await.unwrap_err();
        assert!(matches!(err, BadVersion(0x6)));
    }

    #[tokio::test]
    async fn fails_with_bad_cmd() {
        let (mut client, mut server) = duplex(usize::MAX);
        let connect = Connect;
        client
            .write_all(&[VER, 0x6, RSV, IPV4, 1, 2, 3, 4, 5, 6])
            .await
            .unwrap();

        let err = connect.run::<_, TcpStream>(&mut server).await.unwrap_err();
        assert!(matches!(err, BadCommand(0x6)));
    }

    #[tokio::test]
    async fn fails_with_bad_rsv() {
        let (mut client, mut server) = duplex(usize::MAX);
        let connect = Connect;
        client
            .write_all(&[VER, CONNECT, 0x1, IPV4, 1, 2, 3, 4, 5, 6])
            .await
            .unwrap();

        let err = connect.run::<_, TcpStream>(&mut server).await.unwrap_err();
        assert!(matches!(err, BadRSV(0x1)));
    }
}
