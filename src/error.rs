use std::{io, str::Utf8Error};

use tokio::io::AsyncWriteExt;
use trust_dns_resolver::error::ResolveError;

use crate::{
    constant::{
        AUTH_ERROR, CONNECTION_REFUSED, NO_ACCEPTABLE_METHODS, TARGET_SERVER_UNREACHABLE,
        UNSUPPORTED_COMMAND, VER,
    },
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
    InvalidAtype(u8),
    InvalidDomainName(Utf8Error),
    ResolveDomainError(ResolveError),
    IO(io::Error),
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Self {
        Self::IO(err)
    }
}

impl From<ResolveError> for Error {
    fn from(err: ResolveError) -> Self {
        Self::ResolveDomainError(err)
    }
}

impl Error {
    pub async fn write<W: UnpinAsyncWrite>(self, mut client: W) -> IOResult<()> {
        match self {
            Error::BadVersion(_) | Error::NoAuthMethods | Error::UnacceptableMethods(_) => {
                client.write_all(&[VER, NO_ACCEPTABLE_METHODS]).await
            }
            Error::BadCredential => client.write_all(&[VER, AUTH_ERROR]).await,
            Error::BadRSV(_) | Error::InvalidAtype(_) | Error::InvalidDomainName(_) => {
                client.write_all(&[VER, CONNECTION_REFUSED]).await
            }
            Error::BadCommand(_) => client.write_all(&[VER, UNSUPPORTED_COMMAND]).await,
            Error::ResolveDomainError(_) => {
                client.write_all(&[VER, TARGET_SERVER_UNREACHABLE]).await
            }
            Error::IO(err) => Err(err),
        }
    }
}

#[cfg(test)]
mod tests {

    use trust_dns_resolver::error::ResolveErrorKind;

    use crate::{
        constant::{
            AUTH_ERROR, CONNECTION_REFUSED, NO_ACCEPTABLE_METHODS, TARGET_SERVER_UNREACHABLE,
            UNSUPPORTED_COMMAND, VER,
        },
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

        assert_eq!(out, [VER, UNSUPPORTED_COMMAND]);
    }

    #[tokio::test]
    async fn bad_rsv() {
        let err = Error::BadRSV(0x2);
        let mut out = vec![];
        err.write(&mut out).await.unwrap();

        assert_eq!(out, [VER, CONNECTION_REFUSED]);
    }

    #[tokio::test]
    async fn invalid_domain_name() {
        #[allow(invalid_from_utf8)]
        let err = Error::InvalidDomainName(std::str::from_utf8(&[0, 159]).unwrap_err());
        let mut out = vec![];
        err.write(&mut out).await.unwrap();

        assert_eq!(out, [VER, CONNECTION_REFUSED]);
    }

    #[tokio::test]
    async fn resolve_domain_error() {
        let err = Error::ResolveDomainError(ResolveErrorKind::Timeout.into());
        let mut out = vec![];
        err.write(&mut out).await.unwrap();

        assert_eq!(out, [VER, TARGET_SERVER_UNREACHABLE]);
    }

    #[tokio::test]
    async fn invalid_atype_error() {
        let err = Error::InvalidAtype(0x2);
        let mut out = vec![];
        err.write(&mut out).await.unwrap();

        assert_eq!(out, [VER, CONNECTION_REFUSED]);
    }
}
