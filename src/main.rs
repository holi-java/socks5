use std::env::args;

const DEFAULT_SOCKS5_PORT: u16 = 1080;

#[tokio::main]
async fn main() {
    let port = args()
        .nth(1)
        .map(|it| it.parse::<u16>())
        .unwrap_or(Ok(DEFAULT_SOCKS5_PORT));
    socks5::start(port.unwrap()).await.unwrap();
}
