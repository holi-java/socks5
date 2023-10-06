use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::constant::{NO_AUTH, VER};
use crate::error::Error;
use crate::extract::try_extract_version;
use crate::marker::{Stream, UnpinAsyncRead};
use crate::Result;

#[derive(Debug)]
pub struct Negotiation;

impl Negotiation {
    pub async fn run<S: Stream>(self, mut client: S) -> Result<()> {
        let _nmethods = try_extract_methods(&mut client).await?;
        client.write_all(&[VER, NO_AUTH]).await?;
        Ok(())
    }
}

extract!(nmethods == 0 => Error::NoAuthMethods);

async fn try_extract_methods<T: UnpinAsyncRead>(client: &mut T) -> Result<Vec<u8>> {
    _ = try_extract_version(&mut *client).await?;
    let nmethods = try_extract_nmethods(&mut *client).await.map(usize::from)?;
    let mut methods = unsafe {
        let mut buf = Vec::with_capacity(nmethods);
        #[allow(clippy::uninit_vec)]
        buf.set_len(nmethods);
        buf
    };
    client.read_exact(&mut methods).await?;
    if !methods.contains(&NO_AUTH) {
        return Err(Error::UnacceptableMethods(methods));
    }
    Ok(methods)
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
        let negotiation = Negotiation;
        client.write_all(&[VER, 1, NO_AUTH]).await.unwrap();

        let result = negotiation.run(&mut server).await;

        assert!(result.is_ok());
        assert_eq!(client.read_exact_bytes().await.unwrap(), [VER, NO_AUTH]);
    }

    #[tokio::test]
    async fn fails_with_err_version() {
        let (mut client, mut server) = duplex(100);
        let negotiation = Negotiation;
        client.write_all(&[0x6, 1, NO_AUTH]).await.unwrap();

        let err = negotiation.run(&mut server).await.unwrap_err();

        assert!(matches!(err, BadVersion(ver) if ver == 0x6));
    }

    #[tokio::test]
    async fn fails_without_any_authentication_methods() {
        let (mut client, mut server) = duplex(100);
        let negotiation = Negotiation;
        client.write_all(&[VER, 0]).await.unwrap();

        let err = negotiation.run(&mut server).await.unwrap_err();

        assert!(matches!(err, NoAuthMethods));
    }

    #[tokio::test]
    async fn fails_with_unacceptable_methods() {
        let (mut client, mut server) = duplex(100);
        let negotiation = Negotiation;
        client.write_all(&[VER, 1, 0x3]).await.unwrap();

        let err = negotiation.run(&mut server).await.unwrap_err();

        assert!(matches!(err, UnacceptableMethods(methods) if methods == [0x3]));
    }
}
