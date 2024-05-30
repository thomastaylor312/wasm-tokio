use ::core::future::Future;

use tokio::io::{AsyncRead, AsyncReadExt as _, AsyncWrite, AsyncWriteExt as _};
use tokio_util::bytes::BytesMut;
use tokio_util::codec::Encoder;

fn invalid_utf8() -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidInput, "value is not valid UTF8")
}

pub trait AsyncReadUtf8: AsyncRead {
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(level = "trace", ret, skip_all)
    )]
    fn read_char_utf8(&mut self) -> impl Future<Output = std::io::Result<char>>
    where
        Self: Unpin,
    {
        async move {
            let b = self.read_u8().await?;
            let i = if b & 0x80 == 0 {
                u32::from(b)
            } else if b & 0b1110_0000 == 0b1100_0000 {
                let b2 = self.read_u8().await?;
                if b2 & 0b1100_0000 != 0b1000_0000 {
                    return Err(invalid_utf8());
                }
                u32::from(b & 0b0001_1111) << 6 | u32::from(b2 & 0b0011_1111)
            } else if b & 0b1111_0000 == 0b1110_0000 {
                let mut buf = [0; 2];
                self.read_exact(&mut buf).await?;
                if buf[0] & 0b1100_0000 != 0b1000_0000 || buf[1] & 0b1100_0000 != 0b1000_0000 {
                    return Err(invalid_utf8());
                }
                u32::from(b & 0b0000_1111) << 12
                    | u32::from(buf[0] & 0b0011_1111) << 6
                    | u32::from(buf[1] & 0b0011_1111)
            } else if b & 0b1111_1000 == 0b1111_0000 {
                let mut buf = [0; 3];
                self.read_exact(&mut buf).await?;
                if buf[0] & 0b1100_0000 != 0b1000_0000
                    || buf[1] & 0b1100_0000 != 0b1000_0000
                    || buf[2] & 0b1100_0000 != 0b1000_0000
                {
                    return Err(invalid_utf8());
                }
                u32::from(b & 0b0000_0111) << 18
                    | u32::from(buf[0] & 0b0011_1111) << 12
                    | u32::from(buf[1] & 0b0011_1111) << 6
                    | u32::from(buf[2] & 0b0011_1111)
            } else {
                return Err(invalid_utf8());
            };
            i.try_into()
                .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidInput, err))
        }
    }
}

impl<T: AsyncRead> AsyncReadUtf8 for T {}

pub trait AsyncWriteUtf8: AsyncWrite {
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(level = "trace", ret, skip_all)
    )]
    fn write_char_utf8(&mut self, x: char) -> impl Future<Output = std::io::Result<()>>
    where
        Self: Unpin,
    {
        async move { self.write_all(x.encode_utf8(&mut [0; 4]).as_bytes()).await }
    }
}

impl<T: AsyncWrite> AsyncWriteUtf8 for T {}

pub struct Utf8Encoder;

impl Encoder<char> for Utf8Encoder {
    type Error = std::io::Error;

    fn encode(&mut self, x: char, dst: &mut BytesMut) -> Result<(), Self::Error> {
        dst.extend_from_slice(x.encode_utf8(&mut [0; 4]).as_bytes());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn codec() {
        let v = '$'
            .encode_utf8(&mut [0; 1])
            .as_bytes()
            .read_char_utf8()
            .await
            .expect("failed to read `$`");
        assert_eq!(v, '$');

        let v = '@'
            .encode_utf8(&mut [0; 1])
            .as_bytes()
            .read_char_utf8()
            .await
            .expect("failed to read `@`");
        assert_eq!(v, '@');

        let v = 'И'
            .encode_utf8(&mut [0; 2])
            .as_bytes()
            .read_char_utf8()
            .await
            .expect("failed to read `И`");
        assert_eq!(v, 'И');

        let v = 'ह'
            .encode_utf8(&mut [0; 3])
            .as_bytes()
            .read_char_utf8()
            .await
            .expect("failed to read `ह`");
        assert_eq!(v, 'ह');

        let v = '€'
            .encode_utf8(&mut [0; 3])
            .as_bytes()
            .read_char_utf8()
            .await
            .expect("failed to read `€`");
        assert_eq!(v, '€');

        let v = '한'
            .encode_utf8(&mut [0; 3])
            .as_bytes()
            .read_char_utf8()
            .await
            .expect("failed to read `한`");
        assert_eq!(v, '한');

        let v = '𐍈'
            .encode_utf8(&mut [0; 4])
            .as_bytes()
            .read_char_utf8()
            .await
            .expect("failed to read `𐍈`");
        assert_eq!(v, '𐍈');
    }
}
