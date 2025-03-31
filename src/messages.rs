#[derive(Debug, Clone, Copy)]
#[repr(i16)]
pub enum ErrorCode {
    UnsupportedVersion = 35,
}

pub mod requests {

    pub mod parse {
        use bytes::Buf;

        use crate::messages::responses::{Serialize, SerializeError};

        #[derive(Debug, thiserror::Error, PartialEq)]
        pub enum ParseError {
            #[error("Invalid buffer size <{0}> expected <{1}>")]
            InvalidSize(usize, usize),
            #[error("Error size too large <{0}>")]
            SizeTooLarge(#[from] core::num::TryFromIntError),
            #[error("Invalid Utf8 String <{0}>")]
            InvalidUtf8(#[from] std::str::Utf8Error),
        }
        pub(super) trait Deserialize: Sized {
            type Error;

            fn parse(v: &[u8]) -> Result<(Self, usize), Self::Error>;
        }

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub struct UnsignedVarint {
            pub val: u32,
        }

        impl Deserialize for UnsignedVarint {
            type Error = ParseError;

            fn parse(v: &[u8]) -> Result<(Self, usize), Self::Error> {
                let mut s = 0;
                let mut res = 0;

                const MASK_MSB: u8 = 0x01 << 7;
                const MASK: u8 = !MASK_MSB;

                loop {
                    let m = (v[s] & MASK) as u32;
                    res |= m << (7 * s);

                    if v[s] & MASK_MSB == 0 {
                        break;
                    }
                    s += 1;
                }

                Ok((Self { val: res }, s + 1))
            }
        }

        impl Serialize for UnsignedVarint {
            type Error = SerializeError;

            fn write(&self, buf: &mut [u8]) -> Result<usize, Self::Error> {
                const MASK_MSB: u8 = 0x01 << 7;
                const MASK: u8 = !MASK_MSB;

                // ex: 0x01 0x00 0x00 0x01
                // 0b_1000_0001 0b1000_0000 0b1000_0000 0b0000_0001
                let mut bytes = std::mem::size_of_val(&self.val) - 1;
                while bytes > 0 {
                    if self.val & (0xFF << (8 * bytes)) > 0 {
                        break;
                    }
                    bytes -= 1;
                }

                todo!()
            }
        }

        #[test]
        fn test_deserialize_unsigned_varint() {
            let org = [0b10010110, 0b00000001];
            let exp = Ok((UnsignedVarint { val: 150 }, 2));
            let got = UnsignedVarint::parse(&org);
            assert_eq!(exp, got);
        }

        #[test]
        fn test_serialize_unsigned_varint() {
            let org = UnsignedVarint { val: 150 };

            let mut buf = [0; 4];
            let exp = [0b10010110, 0b00000001];
            let got = org.write(&mut buf);
            assert_eq!(Ok(2), got);
            assert_eq!(exp, buf[..2]);

            let org = UnsignedVarint { val: 0x01_00_00_01 };

            let mut buf = [0; 4];
            let exp = [0b0001_0000, 0b1000_0000, 0b1000_0000, 0b0000_0001];
            let got = org.write(&mut buf);
            assert_eq!(Ok(4), got);
            assert_eq!(exp, buf[..4]);
        }

        /// Represents a sequence of characters or null. For non-null strings, first the length N
        /// is given as an INT16. Then N bytes follow which are the UTF-8 encoding of the character
        /// sequence. A null value is encoded with length of -1 and there are no following bytes.
        #[derive(Debug, Clone)]
        pub struct NullableString {
            pub str: Option<String>,
        }

        impl Deserialize for NullableString {
            type Error = ParseError;

            fn parse(mut v: &[u8]) -> Result<(Self, usize), Self::Error> {
                let len = v.get_i16().min(0) as usize;
                let mut str = None;
                if len > 0 {
                    let s = std::str::from_utf8(&v[2..][..len])?;
                    str = Some(s.to_string());
                }
                Ok((Self { str }, len))
            }
        }

        /// Represents a sequence of characters. First the length N + 1 is given as an
        /// UNSIGNED_VARINT . Then N bytes follow which are the UTF-8 encoding of the character
        /// sequence.
        #[derive(Debug, Clone)]
        pub struct CompactString {
            pub str: String,
        }

        impl Deserialize for CompactString {
            type Error = ParseError;

            fn parse(v: &[u8]) -> Result<(Self, usize), Self::Error> {
                let (size, used) = UnsignedVarint::parse(v)?;
                let s = std::str::from_utf8(&v[used..][..size.val as usize])?.to_string();
                Ok((Self { str: s }, used + size.val as usize))
            }
        }
    }

    use bytes::buf::Buf;
    use parse::{Deserialize, NullableString, ParseError};

    /// Request Header v2 => request_api_key request_api_version correlation_id client_id _tagged_fields
    ///   request_api_key => INT16
    ///   request_api_version => INT16
    ///   correlation_id => INT32
    ///   client_id => NULLABLE_STRING
    #[derive(Debug, Clone)]
    pub struct Header {
        pub request_api_key: i16,
        pub request_api_version: i16,
        pub correlation_id: i32,
        pub client_id: NullableString,
    }

    impl Deserialize for Header {
        type Error = ParseError;

        fn parse(mut buf: &[u8]) -> Result<(Self, usize), Self::Error> {
            let len = buf.len();

            let request_api_key = buf.get_i16();
            let request_api_version = buf.get_i16();
            let correlation_id = buf.get_i32();

            // TODO: parse correctly
            let rem = len - buf.remaining();
            let (client_id, s) = NullableString::parse(&buf[rem..])?;

            let len = len - buf.remaining() - s;

            Ok((
                Self {
                    request_api_key,
                    request_api_version,
                    correlation_id,
                    client_id,
                },
                len,
            ))
        }
    }

    #[derive(Debug, Clone)]
    pub struct Request {
        pub header: Header,
        pub request: RequestType,
    }

    #[derive(Debug, Clone)]
    pub enum RequestType {
        ApiVersions(ApiVersions),
    }

    impl TryFrom<&[u8]> for Request {
        type Error = ParseError;

        fn try_from(mut buf: &[u8]) -> Result<Self, Self::Error> {
            let size: usize = buf.get_i32().try_into()?;

            if size != buf.len() {
                return Err(ParseError::InvalidSize(size, buf.len()));
            }

            let (header, s) = Header::parse(buf)?;
            let (rt, _ss) = RequestType::parse(header.request_api_key, &buf[s..])?;

            Ok(Self {
                header,
                request: rt,
            })
        }
    }

    impl RequestType {
        fn parse(request_api_key: i16, v: &[u8]) -> Result<(Self, usize), ParseError> {
            let s = match request_api_key {
                18 => {
                    let s = ApiVersions::parse(v)?;
                    (RequestType::ApiVersions(s.0), s.1)
                }
                _ => unimplemented!("no such request key"),
            };
            Ok(s)
        }
    }
    #[derive(Debug, Clone)]
    pub struct ApiVersions {
        pub client_software_name: parse::CompactString,
        pub client_software_version: parse::CompactString,
    }

    impl Deserialize for ApiVersions {
        type Error = ParseError;

        fn parse(v: &[u8]) -> Result<(Self, usize), Self::Error> {
            let (client_software_name, s) = parse::CompactString::parse(v)?;
            let (client_software_version, a) = parse::CompactString::parse(&v[s..])?;
            Ok((
                Self {
                    client_software_name,
                    client_software_version,
                },
                s + a,
            ))
        }
    }
}

pub mod responses {
    use bytes::buf::BufMut;

    #[derive(Debug, Clone, thiserror::Error, PartialEq)]
    pub enum SerializeError {}

    use super::ErrorCode;

    pub trait Serialize {
        type Error;
        fn write(&self, buf: &mut [u8]) -> Result<usize, Self::Error>;
    }

    #[derive(Debug, Clone)]
    pub struct Response {
        pub header: Header,
        pub response: ResponseType,
    }

    impl Serialize for Response {
        type Error = SerializeError;

        fn write(&self, mut buf: &mut [u8]) -> Result<usize, Self::Error> {
            let s = self.header.write(&mut buf[4..])?;
            let ss = self.response.write(&mut buf[4 + s..])?;

            let size = s + ss;

            buf.put_i32(size as _);

            Ok(4 + size)
        }
    }

    #[derive(Debug, Clone)]
    pub struct Header {
        pub correlation_id: i32,
    }

    impl Serialize for Header {
        type Error = SerializeError;

        fn write(&self, mut buf: &mut [u8]) -> Result<usize, Self::Error> {
            let len = buf.len();
            buf.put_i32(self.correlation_id);

            Ok(len - buf.remaining_mut())
        }
    }

    #[derive(Debug, Clone)]
    pub enum ResponseType {
        ApiVersions(ApiVersions),
    }

    impl Serialize for ResponseType {
        type Error = SerializeError;

        fn write(&self, buf: &mut [u8]) -> Result<usize, Self::Error> {
            match self {
                ResponseType::ApiVersions(api) => api.write(buf),
            }
        }
    }

    /// ApiVersions Response (Version: 3) => error_code [api_keys] throttle_time_ms _tagged_fields
    ///   error_code => INT16
    ///   api_keys => api_key min_version max_version _tagged_fields
    ///     api_key => INT16
    ///     min_version => INT16
    ///     max_version => INT16
    ///   throttle_time_ms => INT32
    #[derive(Debug, Clone)]
    pub struct ApiVersions {
        pub error_code: Option<ErrorCode>,
        pub api_keys: ApiKeys,
        pub throttle_time_ms: i32,
    }

    impl Serialize for ApiVersions {
        type Error = SerializeError;

        fn write(&self, buf: &mut [u8]) -> Result<usize, Self::Error> {
            (&mut buf[..]).put_i16(self.error_code.map(|s| s as i16).unwrap_or_default());

            let mut s = 2;
            s += self.api_keys.write(&mut buf[s..])?;

            (&mut buf[s..]).put_i32(self.throttle_time_ms);
            Ok(s + 4)
        }
    }

    ///   api_keys => api_key min_version max_version _tagged_fields
    ///     api_key => INT16
    ///     min_version => INT16
    ///     max_version => INT16
    #[derive(Debug, Clone)]
    pub struct ApiKeys {
        pub keys: Vec<ApiKey>,
    }

    impl Serialize for ApiKeys {
        type Error = SerializeError;

        fn write(&self, buf: &mut [u8]) -> Result<usize, Self::Error> {
            (&mut buf[..]).put_i32(self.keys.len() as i32);
            let mut s = 4;

            for (i, key) in self.keys.iter().enumerate() {
                (&mut buf[s..]).put_i16(i as _);
                s += 2;
                s += key.write(&mut buf[s + 2..])?;
            }

            (&mut buf[s..]).put_u8(0);

            Ok(s + 1)
        }
    }

    ///   api_keys => api_key min_version max_version _tagged_fields
    ///     api_key => INT16
    ///     min_version => INT16
    ///     max_version => INT16
    #[derive(Debug, Clone)]
    pub struct ApiKey {
        pub min_version: i16,
        pub max_version: i16,
    }

    impl Serialize for ApiKey {
        type Error = SerializeError;

        fn write(&self, mut buf: &mut [u8]) -> Result<usize, Self::Error> {
            buf.put_i16(self.min_version);
            buf.put_i16(self.max_version);

            Ok(buf.len() - buf.remaining_mut())
        }
    }
}
