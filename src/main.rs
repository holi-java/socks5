use std::env::args;

use socks5::Credential;

const DEFAULT_SOCKS5_PORT: u16 = 1080;

#[tokio::main]
async fn main() {
    let mut args = args().skip(1);
    let credential = args.next().and_then(|it| {
        it.split_once(':')
            .map(|(name, pass)| Credential::new(name, pass))
    });
    let port = args
        .next()
        .map(|it| it.parse::<u16>())
        .unwrap_or(Ok(DEFAULT_SOCKS5_PORT));
    socks5::run(port.unwrap(), credential).await.unwrap();
}
