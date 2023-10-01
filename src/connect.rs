use std::net::SocketAddr;

use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::{
    constant::{CONNECT, IPV4, OK, RSV, UNSPECIFIED_SOCKET_ADDR, VER},
    error::Error::*,
    extract::{try_extract_rsv, try_extract_version},
    marker::{Stream, UnpinAsyncRead},
    Result,
};

#[derive(Debug)]
pub struct Connect;

impl Connect {
    pub async fn run<S: Stream>(self, mut client: S) -> Result<SocketAddr> {
        let addr = try_extract_addr(&mut client).await?;
        client.write_all(&[VER, OK, RSV, IPV4]).await?;
        client.write_all(&UNSPECIFIED_SOCKET_ADDR).await?;
        Ok(addr)
    }
}

extract!(connect_cmd != CONNECT => BadCommand(connect_cmd));

async fn try_extract_addr<T: UnpinAsyncRead>(client: &mut T) -> Result<SocketAddr> {
    _ = try_extract_version(&mut *client).await?;
    _ = try_extract_connect_cmd(&mut *client).await?;
    _ = try_extract_rsv(&mut *client).await?;
    let _atype = client.read_u8().await?;
    let ip = client.read_u32().await?;
    let port = client.read_u16().await?;
    Ok(SocketAddr::from((ip.to_be_bytes(), port)))
}

#[cfg(test)]
mod tests {
    use std::net::SocketAddr;

    use tokio::io::{duplex, AsyncWriteExt};

    use crate::{
        connect::Connect,
        constant::{CONNECT, IPV4, OK, RSV, UNSPECIFIED_SOCKET_ADDR, VER},
        error::Error::*,
        test::AsyncExactRead,
    };

    #[tokio::test]
    async fn connect() {
        let (mut client, mut server) = duplex(usize::MAX);
        let connect = Connect;
        client
            .write_all(&[VER, CONNECT, RSV, IPV4, 1, 2, 3, 4, 5, 6])
            .await
            .unwrap();

        let addr: SocketAddr = connect.run(&mut server).await.unwrap();
        assert_eq!(addr, "1.2.3.4:1286".parse().unwrap());

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

        let err = connect.run(&mut server).await.unwrap_err();
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

        let err = connect.run(&mut server).await.unwrap_err();
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

        let err = connect.run(&mut server).await.unwrap_err();
        assert!(matches!(err, BadRSV(0x1)));
    }
}
