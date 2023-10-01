use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::constant::{OK, VER};
use crate::error::Error;
use crate::extract::try_extract_version;
use crate::marker::{Stream, UnpinAsyncRead};
use crate::Result;

#[derive(Debug)]
pub struct Negotiation<T>(pub T);

impl<T: Stream> Negotiation<T> {
    pub async fn run(self) -> Result<T> {
        let (mut client, _nmethods) = try_extract_methods(self.0).await?;
        client.write_all(&[VER, OK]).await?;
        Ok(client)
    }
}

extractor!(nmethods == 0 => Error::NoAuthMethods);

async fn try_extract_methods<T: UnpinAsyncRead>(client: T) -> Result<(T, Vec<u8>)> {
    let client = try_extract_version(client).await?.0;
    let (mut client, nmethods) = try_extract_nmethods(client)
        .await
        .map(|(a, b)| (a, b as usize))?;
    let mut buf = unsafe {
        let mut buf = Vec::with_capacity(nmethods);
        #[allow(clippy::uninit_vec)]
        buf.set_len(nmethods);
        buf
    };
    client.read_exact(&mut buf).await?;
    Ok((client, buf))
}

#[cfg(test)]
mod tests {
    use tokio::io::duplex;

    use crate::{
        constant::{NO_AUTH, VER},
        test::AsyncExactRead,
    };

    use super::*;
    use crate::error::Error::*;

    #[tokio::test]
    async fn no_auth_negotiation() {
        let (mut client, server) = duplex(100);
        let negotiation = Negotiation(server);
        client.write_all(&[VER, 1, NO_AUTH]).await.unwrap();

        let result = negotiation.run().await;

        assert!(result.is_ok());
        assert_eq!(client.read_exact_bytes().await.unwrap(), [VER, OK]);
    }

    #[tokio::test]
    async fn fails_with_err_version() {
        let (mut client, server) = duplex(100);
        let negotiation = Negotiation(server);
        client.write_all(&[0x6, 1, NO_AUTH]).await.unwrap();

        let err = negotiation.run().await.unwrap_err();

        assert!(matches!(err, BadVersion(ver) if ver == 0x6));
    }

    #[tokio::test]
    async fn fails_without_any_authentication_methods() {
        let (mut client, server) = duplex(100);
        let negotiation = Negotiation(server);
        client.write_all(&[0x5, 0]).await.unwrap();

        let err = negotiation.run().await.unwrap_err();

        assert!(matches!(err, NoAuthMethods));
    }
}
