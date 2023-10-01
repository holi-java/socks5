use tokio::io::copy_bidirectional;

use crate::marker::Stream;
use crate::Result;

pub struct Forward<A, B>(pub A, pub B);

impl<A: Stream, B: Stream> Forward<A, B> {
    pub async fn run(self) -> Result<()> {
        Self::process(self.0, self.1).await
    }

    pub async fn process<D: Stream, U: Stream>(mut client: D, mut upstream: U) -> Result<()> {
        copy_bidirectional(&mut client, &mut upstream).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use tokio::io::{duplex, AsyncWriteExt};

    use crate::test::AsyncExactRead;

    use super::Forward;

    #[tokio::test]
    async fn copy_bidirectional() {
        let (mut a, a2) = duplex(usize::MAX);
        let (mut b, b2) = duplex(usize::MAX);
        let forward = Forward(a2, b2);
        tokio::spawn(forward.run());

        a.write_all(&[1, 2]).await.unwrap();
        b.write_all(&[3, 4]).await.unwrap();

        assert_eq!(a.read_exact_bytes().await.unwrap(), [3, 4]);
        assert_eq!(b.read_exact_bytes().await.unwrap(), [1, 2]);
    }
}
