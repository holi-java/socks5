use std::io;

#[derive(Debug)]
pub enum Error {
    BadVersion(u8),
    NoAuthMethods,
    BadCommand(u8),
    BadRSV(u8),
    IO(io::Error),
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Self {
        Self::IO(err)
    }
}
