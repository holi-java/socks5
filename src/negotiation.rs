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
        let mut client = self.0;
        let _nmethods = try_extract_methods(&mut client).await?;
        client.write_all(&[VER, OK]).await?;
        Ok(client)
    }
}

extract!(nmethods == 0 => Error::NoAuthMethods);

async fn try_extract_methods<T: UnpinAsyncRead>(client: &mut T) -> Result<Vec<u8>> {
    _ = try_extract_version(&mut *client).await?;
    let nmethods = try_extract_nmethods(&mut *client).await.map(usize::from)?;
    let mut buf = unsafe {
        let mut buf = Vec::with_capacity(nmethods);
        #[allow(clippy::uninit_vec)]
        buf.set_len(nmethods);
        buf
    };
    client.read_exact(&mut buf).await?;
    Ok(buf)
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
        let (mut client, mut server) = duplex(100);
        let negotiation = Negotiation(&mut server);
        client.write_all(&[VER, 1, NO_AUTH]).await.unwrap();

        let result = negotiation.run().await;

        assert!(result.is_ok());
        assert_eq!(client.read_exact_bytes().await.unwrap(), [VER, OK]);
    }

    #[tokio::test]
    async fn fails_with_err_version() {
        let (mut client, mut server) = duplex(100);
        let negotiation = Negotiation(&mut server);
        client.write_all(&[0x6, 1, NO_AUTH]).await.unwrap();

        let err = negotiation.run().await.unwrap_err();

        assert!(matches!(err, BadVersion(ver) if ver == 0x6));
    }

    #[tokio::test]
    async fn fails_without_any_authentication_methods() {
        let (mut client, mut server) = duplex(100);
        let negotiation = Negotiation(&mut server);
        client.write_all(&[0x5, 0]).await.unwrap();

        let err = negotiation.run().await.unwrap_err();

        assert!(matches!(err, NoAuthMethods));
    }
}
