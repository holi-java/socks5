//! Trais in current module just used as typealias only for bounded generic parameters.
use tokio::io::{AsyncRead, AsyncWrite};

pub trait UnpinAsyncRead: AsyncRead + Unpin {}
pub trait UnpinAsyncWrite: AsyncWrite + Unpin {}
pub trait Stream: AsyncRead + AsyncWrite + Unpin {}

impl<T: ?Sized> UnpinAsyncRead for T where T: AsyncRead + Unpin {}
impl<T: ?Sized> UnpinAsyncWrite for T where T: AsyncWrite + Unpin {}
impl<T: ?Sized> Stream for T where T: AsyncRead + AsyncWrite + Unpin {}
