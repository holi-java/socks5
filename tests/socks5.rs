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
