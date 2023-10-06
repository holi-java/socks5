use std::io;

use tokio::io::AsyncWriteExt;

use crate::{
    constant::{AUTH_ERROR, CONNECT_DENIED, NO_ACCEPTABLE_METHODS, VER},
    marker::UnpinAsyncWrite,
    IOResult,
};

#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    BadVersion(u8),
    NoAuthMethods,
    UnacceptableMethods(Vec<u8>),
    BadCredential,
    BadCommand(u8),
    BadRSV(u8),
    IO(io::Error),
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Self {
        Self::IO(err)
    }
}

impl Error {
    pub async fn write<W: UnpinAsyncWrite>(self, mut client: W) -> IOResult<()> {
        match self {
            Error::BadVersion(_) | Error::NoAuthMethods | Error::UnacceptableMethods(_) => {
                client.write_all(&[VER, NO_ACCEPTABLE_METHODS]).await
            }
            Error::BadCredential => client.write_all(&[VER, AUTH_ERROR]).await,
            Error::BadRSV(_) | Error::BadCommand(_) => {
                client.write_all(&[VER, CONNECT_DENIED]).await
            }
            Error::IO(err) => Err(err),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        constant::{AUTH_ERROR, CONNECT_DENIED, NO_ACCEPTABLE_METHODS, VER},
        error::Error,
    };

    #[tokio::test]
    async fn bad_version() {
        let err = Error::BadVersion(0x1);
        let mut out = vec![];
        err.write(&mut out).await.unwrap();

        assert_eq!(out, [VER, NO_ACCEPTABLE_METHODS]);
    }

    #[tokio::test]
    async fn no_auth_methods_error() {
        let err = Error::NoAuthMethods;
        let mut out = vec![];
        err.write(&mut out).await.unwrap();

        assert_eq!(out, [VER, NO_ACCEPTABLE_METHODS]);
    }

    #[tokio::test]
    async fn unacceptable_methods_error() {
        let err = Error::UnacceptableMethods(vec![0x3]);
        let mut out = vec![];
        err.write(&mut out).await.unwrap();

        assert_eq!(out, [VER, NO_ACCEPTABLE_METHODS]);
    }

    #[tokio::test]
    async fn bad_credential_error() {
        let err = Error::BadCredential;
        let mut out = vec![];
        err.write(&mut out).await.unwrap();

        assert_eq!(out, [VER, AUTH_ERROR]);
    }

    #[tokio::test]
    async fn bad_command_error() {
        let err = Error::BadCommand(0x2);
        let mut out = vec![];
        err.write(&mut out).await.unwrap();

        assert_eq!(out, [VER, CONNECT_DENIED]);
    }

    #[tokio::test]
    async fn bad_rsv() {
        let err = Error::BadRSV(0x2);
        let mut out = vec![];
        err.write(&mut out).await.unwrap();

        assert_eq!(out, [VER, CONNECT_DENIED]);
    }
}
