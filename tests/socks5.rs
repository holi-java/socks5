use std::{
    io::{self},
    net::{SocketAddr, ToSocketAddrs},
};

use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};

#[path = "../src/test.rs"]
mod test;
use test::*;

include!("../src/constant.rs");

#[tokio::test]
async fn no_auth() {
    let port = 1081;
    tokio::spawn(socks5::start(port));
    _ = tokio::spawn(async {}).await;
    let mut client = TcpStream::connect(format!("127.0.0.1:{port}"))
        .await
        .unwrap();

    // Negotiation
    client.write_all(&[VER, 1, NO_AUTH]).await.unwrap();
    assert_eq!(client.read_exact_bytes().await.unwrap(), [VER, NO_AUTH]);

    // Connect
    client.write_all(&[VER, CONNECT, RSV, IPV4]).await.unwrap();
    client
        .write_all(resolve("www.baidu.com:80").unwrap().as_slice())
        .await
        .unwrap();
    let response = client.read_exact_bytes::<10>().await.unwrap();
    assert_eq!(response[..4], [VER, OK, RSV, IPV4]);
    assert_eq!(response[4..], UNSPECIFIED_SOCKET_ADDR);

    // Send
    let data = b"\
        GET / HTTP/1.1\r\n\
        Host: www.baidu.com\r\n\
        User-Agent: curl/7.68.0\r\n\
        Accept: */*\r\n\r\n\r\n\
    ";
    client.write_all(data).await.unwrap();
    let mut response = String::new();
    client.read_to_string(&mut response).await.unwrap();
    assert!(response.contains("百度一下"), "{}", response);
}

#[tokio::test]
async fn shutdown_bad_request() {
    let port = 1082;
    tokio::spawn(socks5::start(port));
    _ = tokio::spawn(async {}).await;
    let mut client = TcpStream::connect(format!("127.0.0.1:{port}"))
        .await
        .unwrap();

    // Negotiation
    client.write_all(&[VER, 0]).await.unwrap();
    assert_eq!(
        client.read_exact_bytes().await.unwrap(),
        [VER, NO_ACCEPTABLE_METHODS]
    );

    let mut buf = [0; 1];
    assert_eq!(client.read(&mut buf).await.unwrap(), 0);
}

#[tokio::test]
async fn user_credential_authentication() {
    let port = 1083;
    tokio::spawn(socks5::start(port));
    _ = tokio::spawn(async {}).await;
    let mut client = TcpStream::connect(format!("127.0.0.1:{port}"))
        .await
        .unwrap();

    // Negotiation
    client
        .write_all(&[VER, 2, NO_AUTH, CREDENTIAL_AUTH])
        .await
        .unwrap();
    assert_eq!(
        client.read_exact_bytes().await.unwrap(),
        [VER, CREDENTIAL_AUTH]
    );

    // Authentication
    client.write_all(&[VER]).await.unwrap();
    client.write_all(&[6]).await.unwrap();
    client.write_all(b"socks5").await.unwrap();
    client.write_all(&[8]).await.unwrap();
    client.write_all(b"password").await.unwrap();
    assert_eq!(client.read_exact_bytes().await.unwrap(), [VER, OK]);

    // Connect
    client.write_all(&[VER, CONNECT, RSV, IPV4]).await.unwrap();
    client
        .write_all(resolve("www.baidu.com:80").unwrap().as_slice())
        .await
        .unwrap();
    let response = client.read_exact_bytes::<10>().await.unwrap();
    assert_eq!(response[..4], [VER, OK, RSV, IPV4]);
    assert_eq!(response[4..], UNSPECIFIED_SOCKET_ADDR);

    // Send
    let data = b"\
        GET / HTTP/1.1\r\n\
        Host: www.baidu.com\r\n\
        User-Agent: curl/7.68.0\r\n\
        Accept: */*\r\n\r\n\r\n\
    ";
    client.write_all(data).await.unwrap();
    let mut response = String::new();
    client.read_to_string(&mut response).await.unwrap();
    assert!(response.contains("百度一下"), "{}", response);
}

#[tokio::test]
async fn fails_with_bad_credential() {
    let port = 1084;
    tokio::spawn(socks5::start(port));
    _ = tokio::spawn(async {}).await;
    let mut client = TcpStream::connect(format!("127.0.0.1:{port}"))
        .await
        .unwrap();

    // Negotiation
    client
        .write_all(&[VER, 2, NO_AUTH, CREDENTIAL_AUTH])
        .await
        .unwrap();
    assert_eq!(
        client.read_exact_bytes().await.unwrap(),
        [VER, CREDENTIAL_AUTH]
    );

    // Authentication
    client.write_all(&[VER]).await.unwrap();
    client.write_all(&[6]).await.unwrap();
    client.write_all(b"socks5").await.unwrap();
    client.write_all(&[3]).await.unwrap();
    client.write_all(b"bad").await.unwrap();
    assert_eq!(client.read_exact_bytes().await.unwrap(), [VER, AUTH_ERROR]);

    let mut buf = [0; 1];
    assert_eq!(client.read(&mut buf).await.unwrap(), 0, "Closed");
}

fn resolve<T: ToSocketAddrs>(addr: T) -> io::Result<[u8; 6]> {
    addr.to_socket_addrs()?
        .find(SocketAddr::is_ipv4)
        .ok_or_else(|| io::Error::new(io::ErrorKind::Other, "Empty"))
        .map(|addr| {
            if let SocketAddr::V4(addr) = addr {
                let mut out = [0; 6];
                const BOUNDARY: usize = 4;
                out[..BOUNDARY].copy_from_slice(&addr.ip().octets());
                out[BOUNDARY..].copy_from_slice(&addr.port().to_be_bytes());
                return out;
            }
            unreachable!()
        })
}
