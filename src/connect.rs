use std::{future::Future, net::SocketAddr};

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use trust_dns_resolver::{
    error::ResolveError,
    name_server::{GenericConnector, TokioRuntimeProvider},
    AsyncResolver, TokioAsyncResolver,
};

use crate::{
    constant::{CONNECT, DOMAIN_NAME, IPV4, IPV6, OK, RSV, UNSPECIFIED_SOCKET_ADDR, VER},
    error::Error::{self, *},
    extract::{try_extract_rsv, try_extract_version},
    forward::Forward,
    marker::{Stream, UnpinAsyncRead},
    read_vec_u8, IOResult, Result, Socks5, Upstream,
};

lazy_static::lazy_static! {
    static ref DNS_RESOLVER: AsyncResolver<GenericConnector<TokioRuntimeProvider>> = TokioAsyncResolver::tokio_from_system_conf().unwrap();
}

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
    let atype = client.read_u8().await?;
    match atype {
        IPV4 => {
            let ip = client.read_u32().await?;
            let port = client.read_u16().await?;
            Ok(SocketAddr::from((ip.to_be_bytes(), port)))
        }
        IPV6 => {
            let ip = client.read_u128().await?;
            let port = client.read_u16().await?;
            Ok(SocketAddr::from((ip.to_be_bytes(), port)))
        }
        DOMAIN_NAME => {
            let len = client.read_u8().await? as usize;
            let domain = read_vec_u8(&mut client, len).await?;
            let domain = std::str::from_utf8(&domain).map_err(Error::InvalidDomainName)?;
            let port = client.read_u16().await?;
            let record = DNS_RESOLVER
                .ipv4_lookup(domain)
                .await?
                .into_iter()
                .next()
                .ok_or_else(|| ResolveError::from("No record found"))?;
            Ok(SocketAddr::from((record.0, port)))
        }
        _ => todo!(),
    }
}

#[cfg(test)]
mod tests {

    use std::{io::Cursor, net::SocketAddr};

    use tokio::{
        io::{duplex, AsyncWriteExt},
        net::TcpStream,
    };
    use trust_dns_resolver::TokioAsyncResolver;

    use crate::{
        connect::Connect,
        constant::{CONNECT, DOMAIN_NAME, IPV4, IPV6, OK, RSV, UNSPECIFIED_SOCKET_ADDR, VER},
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

    #[tokio::test]
    async fn extract_ipv6_addr() {
        let buf = Cursor::new([
            VER, CONNECT, RSV, IPV6, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0,
            0x0, 0x0, 0x0, 0x1, 0, 10,
        ]);
        let addr = super::try_extract_addr(buf).await.unwrap();

        assert_eq!(addr, "[::1]:10".parse().unwrap());
    }

    #[tokio::test]
    async fn extract_domain_name() {
        let buf = Cursor::new({
            let host = "www.baidu.com";
            let mut buf = vec![VER, CONNECT, RSV, DOMAIN_NAME];
            buf.push(host.len() as u8);
            buf.extend(host.as_bytes());
            buf.extend([0, 80]);
            buf
        });
        let addr = super::try_extract_addr(buf).await.unwrap();

        let ip = TokioAsyncResolver::tokio_from_system_conf().unwrap();
        let lookup = ip.ipv4_lookup("www.baidu.com").await.unwrap();
        let available = lookup
            .iter()
            .map(|it| SocketAddr::from((it.0, 80)))
            .collect::<Vec<_>>();
        assert!(available.contains(&addr));
    }
}
