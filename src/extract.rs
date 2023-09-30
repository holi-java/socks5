use crate::error::Error::*;
use crate::{constant::VER, marker::UnpinAsyncRead, Result};
use tokio::io::AsyncReadExt;

macro_rules! extractor {
    ($name: ident $op:tt $expected: expr => $err: expr) => {
        ::concat_idents::concat_idents!(method = try_extract, _, $name {
            #[cold]
            #[inline(always)]
            pub async fn method<T: UnpinAsyncRead>(mut client: T) -> Result<(T, u8)> {
                let $name = client.read_u8().await?;
                if $name $op $expected {
                    return Err($err);
                }
                return Ok((client, $name));
            }
        });
    };
}

extractor!(version != VER => BadVersion(version));
extractor!(nmethods == 0 => NoAuthMethods);
