use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::{
    connect::Connect,
    constant::{OK, VER},
    error::Error,
    marker::{Stream, UnpinAsyncRead},
    read_vec_u8, Result, Stage,
};

#[derive(Debug, Clone, PartialEq)]
pub struct Credential {
    username: String,
    password: String,
}

impl Credential {
    pub fn new<U, P>(username: U, password: P) -> Self
    where
        U: Into<String>,
        P: Into<String>,
    {
        Credential {
            username: username.into(),
            password: password.into(),
        }
    }

    pub(crate) async fn run<S: Stream, U>(&mut self, mut client: S) -> Result<Stage<U>> {
        let (username, password) = try_extract_credential(&mut client).await?;
        if self.username.as_bytes() != username || self.password.as_bytes() != password {
            return Err(Error::BadCredential);
        }
        client.write_all(&[VER, OK]).await?;
        Ok(Stage::Connect(Connect))
    }
}

extract!(version > VER => Error::BadVersion(version));

async fn try_extract_credential<R: UnpinAsyncRead>(mut client: R) -> Result<(Vec<u8>, Vec<u8>)> {
    try_extract_version(&mut client).await?;
    let username = {
        let ulen = client.read_u8().await? as usize;
        read_vec_u8(&mut client, ulen).await?
    };
    let password = {
        let plen = client.read_u8().await? as usize;
        read_vec_u8(&mut client, plen).await?
    };
    Ok((username, password))
}

#[cfg(test)]
mod tests {
    use tokio::{
        io::{duplex, AsyncWriteExt},
        net::TcpStream,
    };

    use crate::{
        constant::{OK, VER},
        credential::Credential,
        error::Error,
        test::AsyncExactRead,
        Stage,
    };

    #[tokio::test]
    async fn authenticate_with_valid_credential() {
        let mut it = Credential::new("root", "pass");
        let (mut a, b) = duplex(usize::MAX);

        a.write_all(&[VER]).await.unwrap();
        a.write_all(&[4]).await.unwrap();
        a.write_all(b"root").await.unwrap();
        a.write_all(&[4]).await.unwrap();
        a.write_all(b"pass").await.unwrap();

        let result = it.run::<_, TcpStream>(b).await;
        assert!(matches!(result, Ok(Stage::Connect(_))));
        assert_eq!(a.read_exact_bytes().await.unwrap(), [VER, OK]);
    }

    #[tokio::test]
    async fn authenticate_with_lower_version_is_ok() {
        let mut it = Credential::new("root", "pass");
        let (mut a, b) = duplex(usize::MAX);

        a.write_all(&[0x1]).await.unwrap();
        a.write_all(&[4]).await.unwrap();
        a.write_all(b"root").await.unwrap();
        a.write_all(&[4]).await.unwrap();
        a.write_all(b"pass").await.unwrap();

        let result = it.run::<_, TcpStream>(b).await;
        assert!(matches!(result, Ok(Stage::Connect(_))));
        assert_eq!(a.read_exact_bytes().await.unwrap(), [VER, OK]);
    }

    #[tokio::test]
    async fn fails_with_bad_version() {
        let mut it = Credential::new("root", "pass");
        let (mut a, b) = duplex(usize::MAX);

        a.write_all(&[0x06]).await.unwrap();

        let result = it.run::<_, TcpStream>(b).await;
        assert!(matches!(result, Err(Error::BadVersion(0x6))));
    }

    #[tokio::test]
    async fn fails_with_bad_credential() {
        let mut it = Credential::new("root", "pass");
        let (mut a, b) = duplex(usize::MAX);

        a.write_all(&[VER]).await.unwrap();
        a.write_all(&[4]).await.unwrap();
        a.write_all(b"root").await.unwrap();
        a.write_all(&[3]).await.unwrap();
        a.write_all(b"bad").await.unwrap();

        let result = it.run::<_, TcpStream>(b).await;
        assert!(matches!(result, Err(Error::BadCredential)));
    }
}
