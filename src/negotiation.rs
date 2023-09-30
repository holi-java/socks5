use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::constant::{OK, VER};
use crate::extract::{try_extract_nmethods, try_extract_version};
use crate::marker::{UnpinAsyncRead, UnpinAsyncWrite};
use crate::Result;

#[derive(Debug)]
pub struct Negotiation<T>(pub T);

impl<T: UnpinAsyncRead> Negotiation<T> {
    pub async fn try_unpack(client: T) -> Result<Self> {
        let client = try_extract_methods(client).await?;
        Ok(Negotiation(client))
    }
}

impl<T: UnpinAsyncWrite> Negotiation<T> {
    pub async fn run(self) -> Result<T> {
        let mut client = self.0;
        client.write_all(&[VER, OK]).await?;
        Ok(client)
    }
}

async fn try_extract_methods<T: UnpinAsyncRead>(client: T) -> Result<T> {
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
    Ok(client)
}

#[cfg(test)]
mod tests {
    use crate::constant::{NO_AUTH, VER};

    use super::*;
    use crate::error::Error::*;

    #[tokio::test]
    async fn parse_no_auth_negotiation() {
        let result: Result<Negotiation<_>> =
            Negotiation::try_unpack([VER, 1, NO_AUTH].as_slice()).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn fails_with_err_version() {
        let err = Negotiation::try_unpack([0x6, 1, NO_AUTH].as_slice())
            .await
            .unwrap_err();
        assert!(matches!(err, BadVersion(ver) if ver == 0x6));
    }

    #[tokio::test]
    async fn fails_without_any_authentication_methods() {
        let err = Negotiation::try_unpack([VER, 0].as_slice())
            .await
            .unwrap_err();
        assert!(matches!(err, NoAuthMethods));
    }

    #[tokio::test]
    async fn write_ack_response() {
        let it = Negotiation(Vec::<u8>::new());
        let result: Result<Vec<u8>> = it.run().await;

        assert_eq!(result.unwrap(), [VER, OK]);
    }
}
