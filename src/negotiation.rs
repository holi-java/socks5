use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::connect::Connect;
use crate::constant::{CREDENTIAL_AUTH, NO_AUTH, VER};
use crate::credential::Credential;
use crate::error::Error;
use crate::extract::try_extract_version;
use crate::marker::{Stream, UnpinAsyncRead};
use crate::{read_vec_u8, Result, Sock5};

pub const USERNAME: &str = include_str!("conf/username");
pub const PASSWORD: &str = include_str!("conf/password");

#[derive(Debug)]
pub struct Negotiation;

impl Negotiation {
    pub async fn run<S: Stream, U>(self, mut client: S) -> Result<Sock5<U>> {
        let methods = try_extract_methods(&mut client).await?;

        if methods.contains(&CREDENTIAL_AUTH) {
            client.write_all(&[VER, CREDENTIAL_AUTH]).await?;
            return Ok(Sock5::Authentication(Credential::new(
                USERNAME.trim(),
                PASSWORD.trim(),
            )));
        }

        if !methods.contains(&NO_AUTH) {
            return Err(Error::UnacceptableMethods(methods));
        }
        client.write_all(&[VER, NO_AUTH]).await?;
        Ok(Sock5::Connect(Connect))
    }
}

extract!(nmethods == 0 => Error::NoAuthMethods);

async fn try_extract_methods<T: UnpinAsyncRead>(mut client: T) -> Result<Vec<u8>> {
    _ = try_extract_version(&mut client).await?;
    let nmethods = try_extract_nmethods(&mut client).await.map(usize::from)?;
    Ok(read_vec_u8(client, nmethods).await?)
}

#[cfg(test)]
mod tests {
    use tokio::{io::duplex, net::TcpStream};

    use crate::{
        constant::{CREDENTIAL_AUTH, NO_AUTH, VER},
        credential::Credential,
        test::AsyncExactRead,
    };

    use super::*;
    use crate::error::Error::*;

    #[tokio::test]
    async fn no_auth_negotiation() {
        let (mut client, mut server) = duplex(100);
        let negotiation = Negotiation;
        client.write_all(&[VER, 1, NO_AUTH]).await.unwrap();

        let result = negotiation.run::<_, TcpStream>(&mut server).await;

        assert!(matches!(result, Ok(Sock5::Connect(_))));
        assert_eq!(client.read_exact_bytes().await.unwrap(), [VER, NO_AUTH]);
    }

    #[tokio::test]
    async fn credential_auth_negotiation() {
        let (mut client, mut server) = duplex(100);
        let negotiation = Negotiation;
        client.write_all(&[VER, 1, CREDENTIAL_AUTH]).await.unwrap();

        let result = negotiation.run::<_, TcpStream>(&mut server).await;

        assert!(
            matches!(result, Ok(Sock5::Authentication(credential)) if credential == Credential::new("socks5", "password"))
        );
        assert_eq!(
            client.read_exact_bytes().await.unwrap(),
            [VER, CREDENTIAL_AUTH]
        );
    }

    #[tokio::test]
    async fn fails_with_err_version() {
        let (mut client, mut server) = duplex(100);
        let negotiation = Negotiation;
        client.write_all(&[0x6, 1, NO_AUTH]).await.unwrap();

        let err = negotiation
            .run::<_, TcpStream>(&mut server)
            .await
            .unwrap_err();

        assert!(matches!(err, BadVersion(ver) if ver == 0x6));
    }

    #[tokio::test]
    async fn fails_without_any_authentication_methods() {
        let (mut client, mut server) = duplex(100);
        let negotiation = Negotiation;
        client.write_all(&[VER, 0]).await.unwrap();

        let err = negotiation
            .run::<_, TcpStream>(&mut server)
            .await
            .unwrap_err();

        assert!(matches!(err, NoAuthMethods));
    }

    #[tokio::test]
    async fn fails_with_unacceptable_methods() {
        let (mut client, mut server) = duplex(100);
        let negotiation = Negotiation;
        client.write_all(&[VER, 1, 0x3]).await.unwrap();

        let err = negotiation
            .run::<_, TcpStream>(&mut server)
            .await
            .unwrap_err();

        assert!(matches!(err, UnacceptableMethods(methods) if methods == [0x3]));
    }
}
