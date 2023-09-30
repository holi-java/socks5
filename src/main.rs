const DEFAULT_SOCKS5_PORT: u16 = 1080;

#[tokio::main]
async fn main() {
    socks5::start(DEFAULT_SOCKS5_PORT).await.unwrap();
}
