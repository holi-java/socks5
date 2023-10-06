use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::{
    connect::Connect,
    constant::{OK, VER},
    error::Error,
    marker::{Stream, UnpinAsyncRead},
    Result, Sock5,
};

#[derive(Debug, PartialEq)]
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

    pub async fn run<S: Stream, U>(self, mut client: S) -> Result<Sock5<U>> {
        let (username, password) = try_extract_credential(&mut client).await?;
        if self.username.as_bytes() != username || self.password.as_bytes() != password {
            return Err(Error::BadCredential);
        }
        client.write_all(&[VER, OK]).await?;
        Ok(Sock5::Connect(Connect))
    }
}

extract!(version > VER => Error::BadVersion(version));

async fn try_extract_credential<R: UnpinAsyncRead>(mut client: R) -> Result<(Vec<u8>, Vec<u8>)> {
    try_extract_version(&mut client).await?;
    let mut buf = [0; 0xFF];
    let username = {
        let ulen = client.read_u8().await? as usize;
        client.read_exact(&mut buf[..ulen]).await?;
        buf[..ulen].to_vec()
    };
    let password = {
        let plen = client.read_u8().await? as usize;
        client.read_exact(&mut buf[..plen]).await?;
        buf[..plen].to_vec()
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
        Sock5,
    };

    #[tokio::test]
    async fn authenticate_with_valid_credential() {
        let it = Credential::new("root", "pass");
        let (mut a, b) = duplex(usize::MAX);

        a.write_all(&[VER]).await.unwrap();
        a.write_all(&[4]).await.unwrap();
        a.write_all(b"root").await.unwrap();
        a.write_all(&[4]).await.unwrap();
        a.write_all(b"pass").await.unwrap();

        let result = it.run::<_, TcpStream>(b).await;
        assert!(matches!(result, Ok(Sock5::Connect(_))));
        assert_eq!(a.read_exact_bytes().await.unwrap(), [VER, OK]);
    }

    #[tokio::test]
    async fn authenticate_with_lower_version_is_ok() {
        let it = Credential::new("root", "pass");
        let (mut a, b) = duplex(usize::MAX);

        a.write_all(&[0x1]).await.unwrap();
        a.write_all(&[4]).await.unwrap();
        a.write_all(b"root").await.unwrap();
        a.write_all(&[4]).await.unwrap();
        a.write_all(b"pass").await.unwrap();

        let result = it.run::<_, TcpStream>(b).await;
        assert!(matches!(result, Ok(Sock5::Connect(_))));
        assert_eq!(a.read_exact_bytes().await.unwrap(), [VER, OK]);
    }

    #[tokio::test]
    async fn fails_with_bad_version() {
        let it = Credential::new("root", "pass");
        let (mut a, b) = duplex(usize::MAX);

        a.write_all(&[0x06]).await.unwrap();

        let result = it.run::<_, TcpStream>(b).await;
        assert!(matches!(result, Err(Error::BadVersion(0x6))));
    }

    #[tokio::test]
    async fn fails_with_bad_credential() {
        let it = Credential::new("root", "pass");
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