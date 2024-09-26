#[derive(Debug, Clone, Copy)]
#[repr(i16)]
pub enum ErrorCode {
    UnsupportedVersion = 35,
}

pub mod requests {
    use bytes::buf::Buf;

    #[derive(Debug, thiserror::Error)]
    pub enum ParseError {
        #[error("Invalid buffer size <{0}> expected <{1}>")]
        InvalidSize(usize, usize),
        #[error("Error size too large <{0}>")]
        SizeTooLarge(#[from] core::num::TryFromIntError),
    }

    #[derive(Debug, Clone)]
    pub struct V0 {
        pub request_api_key: i16,
        pub request_api_version: i16,
        pub correlation_id: i32,
    }

    impl TryFrom<&[u8]> for V0 {
        type Error = ParseError;

        fn try_from(mut buf: &[u8]) -> Result<Self, Self::Error> {
            let size: usize = buf.get_i32().try_into()?;
            if size != buf.len() {
                return Err(ParseError::InvalidSize(size, buf.len()));
            }

            Ok(Self {
                request_api_key: buf.get_i16(),
                request_api_version: buf.get_i16(),
                correlation_id: buf.get_i32(),
            })
        }
    }
}

pub mod responses {
    use bytes::buf::BufMut;

    use super::ErrorCode;

    #[derive(Debug, Clone)]
    pub enum V0 {
        ApiVersionsRequest {
            correlation_id: i32,
            error_code: Option<ErrorCode>,
        },
    }

    impl V0 {
        pub fn write(&self, mut buf: &mut [u8]) -> anyhow::Result<usize> {
            let len = buf.len();
            let mut second = &mut buf[4..];
            match self {
                V0::ApiVersionsRequest {
                    correlation_id,
                    error_code,
                } => {
                    second.put_i32(*correlation_id);
                    if let Some(err) = error_code {
                        second.put_i16(*err as _)
                    }
                }
            }

            let len = len - second.len();
            buf.put_i32(len as _);

            debug_assert_eq!(len, 8);

            Ok(len)
        }
    }
}
