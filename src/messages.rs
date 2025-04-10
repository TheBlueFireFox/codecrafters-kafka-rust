#![allow(dead_code)]

#[derive(Debug, Clone, Copy, Default)]
#[repr(i16)]
pub enum ApiKeys {
    Fetch = 1,
    #[default]
    ApiVersions = 18,
}

#[derive(Debug, Clone, Copy, Default)]
#[repr(i16)]
pub enum ErrorCode {
    #[default]
    NoError = 0,
    UnsupportedVersion = 35,
    UnknownTopic = 100,
}

pub mod primitives {
    use bytes::{Buf, BufMut};

    #[derive(Debug, thiserror::Error, PartialEq)]
    pub enum ParseError {
        #[error("Invalid buffer size <{0}> expected <{1}>")]
        InvalidSize(usize, usize),
        #[error("Varint is larger then expected")]
        InvalidVarint,
        #[error("Varlong is larger then expected")]
        InvalidVarlong,
        #[error("Error size too large <{0}>")]
        SizeTooLarge(#[from] core::num::TryFromIntError),
        #[error("Invalid Utf8 String <{0}>")]
        InvalidUtf8Str(#[from] std::str::Utf8Error),
        #[error("Invalid Utf8 String <{0}>")]
        InvalidUtf8(#[from] std::string::FromUtf8Error),
    }

    pub trait Deserialize: Sized {
        type Error;

        fn parse(buf: &[u8]) -> Result<(Self, usize), Self::Error>;
    }

    pub trait Serialize {
        type Error;
        fn write(&self, buf: &mut [u8]) -> Result<usize, Self::Error>;
    }

    #[derive(Debug, Clone, thiserror::Error, PartialEq)]
    pub enum SerializeError {}

    macro_rules! VariableInt {
        ( ($t:ident, $i:ident) ) => {
            #[derive(Debug, Copy, Clone, PartialEq, Eq, Default)]
            pub struct $t {
                pub val: $i,
            }

            impl Deserialize for $t {
                type Error = ParseError;

                fn parse(v: &[u8]) -> Result<(Self, usize), Self::Error> {
                    let mut s = 0;
                    let mut res = 0;

                    const MASK_MSB: u8 = 0x01 << 7;
                    const MASK: u8 = !MASK_MSB;

                    loop {
                        if s > std::mem::size_of::<$i>() {
                            return Err(ParseError::InvalidVarint);
                        }

                        let m = (v[s] & MASK) as $i;
                        res |= m << (7 * s);

                        if v[s] & MASK_MSB == 0 {
                            break;
                        }
                        s += 1;
                    }

                    // (n >> 1) ^ (-(n & 1))
                    let res = (res >> 1) ^ (-(res & 1));

                    Ok((Self { val: res }, s + 1))
                }
            }

            impl Serialize for $t {
                type Error = SerializeError;

                fn write(&self, buf: &mut [u8]) -> Result<usize, Self::Error> {
                    const MASK_MSB: u8 = 0x80;
                    const MASK: u8 = !MASK_MSB;

                    // ex: 0x01 0x00 0x00 0x01
                    // => 0b1000_0001
                    // 0b_1000_0001 0b1000_0000 0b1000_0000 0b0000_0001
                    let mut count = 0;
                    let mut val = self.val;
                    // (n << 1) ^ (n >> 31)
                    val = (val << 1) ^ (val >> (std::mem::size_of::<$i>() * 8 - 1));

                    loop {
                        let mut b = val & (MASK as $i);

                        val >>= 7;

                        if val > 0 {
                            b |= 0x80;
                        }

                        buf[count] = b as u8;

                        count += 1;

                        if val == 0 {
                            break Ok(count);
                        }
                    }
                }
            }
        };
    }

    macro_rules! VariableUInt {
        ( ($t:ident, $i:ident) ) => {
            #[derive(Debug, Copy, Clone, PartialEq, Eq, Default)]
            pub struct $t {
                pub val: $i,
            }

            impl Deserialize for $t {
                type Error = ParseError;

                fn parse(v: &[u8]) -> Result<(Self, usize), Self::Error> {
                    let mut s = 0;
                    let mut res = 0;

                    const MASK_MSB: u8 = 0x01 << 7;
                    const MASK: u8 = !MASK_MSB;

                    loop {
                        if s > std::mem::size_of::<$i>() {
                            return Err(ParseError::InvalidVarint);
                        }

                        let m = (v[s] & MASK) as $i;
                        res |= m << (7 * s);

                        if v[s] & MASK_MSB == 0 {
                            break;
                        }
                        s += 1;
                    }

                    Ok((Self { val: res }, s + 1))
                }
            }

            impl Serialize for $t {
                type Error = SerializeError;

                fn write(&self, buf: &mut [u8]) -> Result<usize, Self::Error> {
                    const MASK_MSB: u8 = 0x80;
                    const MASK: u8 = !MASK_MSB;

                    // ex: 0x01 0x00 0x00 0x01
                    // => 0b1000_0001
                    // 0b_1000_0001 0b1000_0000 0b1000_0000 0b0000_0001
                    let mut count = 0;
                    let mut val = self.val;

                    loop {
                        let mut b = val & (MASK as $i);

                        val >>= 7;

                        if val > 0 {
                            b |= 0x80;
                        }

                        buf[count] = b as u8;

                        count += 1;

                        if val == 0 {
                            break Ok(count);
                        }
                    }
                }
            }
        };
    }

    VariableInt!((Varint, i32));
    VariableInt!((Varlong, i64));
    VariableUInt!((Varuint, u32));
    VariableUInt!((Varulong, u64));

    #[test]
    fn test_deserialize_signed_varint() {
        let org = [0x3A];
        let exp = Ok((Varint { val: 29 }, 1));
        let got = Varint::parse(&org);
        assert_eq!(exp, got);
    }

    #[test]
    fn test_deserialize_unsigned_varint() {
        let org = [0b10010110, 0b00000001];
        let exp = Ok((Varuint { val: 150 }, 2));
        let got = Varuint::parse(&org);
        assert_eq!(exp, got);
    }

    #[test]
    fn test_serialize_unsigned_varint_a() {
        let org = Varuint { val: 150 };

        let mut buf = [0; 4];
        let exp = [0b10010110, 0b00000001];
        let got = org.write(&mut buf);
        assert_eq!(Ok(2), got);
        assert_eq!(exp, buf[..2]);
    }

    #[test]
    fn test_serialize_unsigned_varint_b() {
        let org = Varuint { val: 0x01_00_00_01 };

        let mut buf = [0; 4];
        let exp = [0b1000_0001, 0b1000_0000, 0b1000_0000, 0b0000_1000];

        let got = org.write(&mut buf);
        assert_eq!(Ok(4), got);
        assert_eq!(exp, buf);
    }

    #[derive(Debug, Copy, Clone, Default)]
    pub struct Uuid {
        pub uuid: u128,
    }

    impl Deserialize for Uuid {
        type Error = ParseError;

        fn parse(mut buf: &[u8]) -> Result<(Self, usize), Self::Error> {
            const LEN: usize = std::mem::size_of::<u128>();

            if buf.len() <= LEN {
                return Err(ParseError::InvalidSize(buf.len(), LEN));
            }

            let uuid = buf.get_u128();

            Ok((Self { uuid }, LEN))
        }
    }

    impl Serialize for Uuid {
        type Error = SerializeError;

        fn write(&self, mut buf: &mut [u8]) -> Result<usize, Self::Error> {
            const LEN: usize = std::mem::size_of::<u128>();

            buf.put_u128(self.uuid);

            Ok(LEN)
        }
    }

    /// Represents a sequence of characters or null. For non-null strings, first the length N
    /// is given as an INT16. Then N bytes follow which are the UTF-8 encoding of the character
    /// sequence. A null value is encoded with length of -1 and there are no following bytes.
    #[derive(Debug, Clone, Default)]
    pub struct NullableString {
        pub str: Option<String>,
    }

    impl Deserialize for NullableString {
        type Error = ParseError;

        fn parse(mut buf: &[u8]) -> Result<(Self, usize), Self::Error> {
            let len = buf.get_i16();
            let len = len.max(0) as usize;
            let mut str = None;
            if len > 0 {
                let s = std::str::from_utf8(&buf[..len])?;
                str = Some(s.to_string());
            }
            Ok((Self { str }, len + 2))
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

        fn parse(buf: &[u8]) -> Result<(Self, usize), Self::Error> {
            let (arr, used) = CompactArray::parse(buf)?;
            let s = Self {
                str: String::from_utf8(arr.vec)?,
            };

            Ok((s, used))
        }
    }

    /// Represents a sequence of objects of a given type T. Type T can be either a primitive type
    /// (e.g. STRING) or a structure. First, the length N + 1 is given as an UNSIGNED_VARINT. Then
    /// N instances of type T follow. A null array is represented with a length of 0. In protocol
    /// documentation an array of T instances is referred to as [T].
    #[derive(Debug, Clone)]
    pub struct CompactArray<T> {
        pub vec: Vec<T>,
    }

    impl<T: Default> Default for CompactArray<T> {
        fn default() -> Self {
            Self {
                vec: Default::default(),
            }
        }
    }

    impl Deserialize for u8 {
        type Error = ParseError;

        fn parse(mut buf: &[u8]) -> Result<(Self, usize), Self::Error> {
            let len = buf.len();
            let val = buf.get_u8();
            Ok((val, len - buf.remaining()))
        }
    }

    impl Deserialize for i32 {
        type Error = ParseError;

        fn parse(mut buf: &[u8]) -> Result<(Self, usize), Self::Error> {
            let len = buf.len();
            let val = buf.get_i32();
            Ok((val, len - buf.remaining()))
        }
    }

    impl<T: Serialize<Error = SerializeError>> Serialize for CompactArray<T> {
        type Error = SerializeError;

        fn write(&self, buf: &mut [u8]) -> Result<usize, Self::Error> {
            // CompactArray format is N + 1
            let size = self.vec.len() + 1;
            let size = Varuint { val: size as _ };
            let mut s = size.write(buf)?;

            for key in &self.vec {
                s += key.write(&mut buf[s..])?;
            }

            Ok(s)
        }
    }

    impl<T: Deserialize<Error = ParseError>> Deserialize for CompactArray<T> {
        type Error = ParseError;

        fn parse(buf: &[u8]) -> Result<(Self, usize), Self::Error> {
            let (count, mut used) = Varuint::parse(buf)?;

            let count = count.val as usize;

            let count = count.saturating_sub(1);

            if count == 0 {
                return Ok((Self { vec: vec![] }, used));
            }

            let mut vec = Vec::with_capacity(count);
            for _ in 0..count {
                let (e, s) = T::parse(&buf[used..])?;
                used += s;
                vec.push(e);
            }

            Ok((Self { vec }, used))
        }
    }

    /// Tag Headers
    ///
    /// The tag header contains two unsigned variable-length integers.  The first one contains the
    /// field's tag.  The second one is the length of the field.
    /// number of tagged fields UNSIGNED_VARINT
    /// fields x tag: UNSIGNED_VARINT
    /// field 1 len UNSIGNED_VARINT
    /// Data <field 1 type>
    #[derive(Debug, Clone, Default)]
    pub struct TaggedFields;

    impl Deserialize for TaggedFields {
        type Error = ParseError;

        fn parse(v: &[u8]) -> Result<(Self, usize), Self::Error> {
            if v[0] == 0 {
                return Ok((TaggedFields, 1));
            }

            unimplemented!("The TaggedFields only support the empty variation")
        }
    }

    impl Serialize for TaggedFields {
        type Error = SerializeError;

        fn write(&self, buf: &mut [u8]) -> Result<usize, Self::Error> {
            buf[0] = 0;
            Ok(1)
        }
    }
}

pub mod disk {
    use bytes::Buf;

    use super::primitives::{
        CompactArray, Deserialize, ParseError, Serialize, SerializeError, Varint, Varlong,
    };

    #[derive(Debug, Clone, Default)]
    pub struct CompactRecords {
        pub vec: CompactArray<RecordBatch>,
    }

    impl CompactRecords {
        pub fn from_cluster_meta(mut buf: &[u8]) -> Result<Self, ParseError> {
            let mut v = Vec::new();

            while !buf.is_empty() {
                let (rec, s) = RecordBatch::parse(buf)?;

                buf.advance(s);
                v.push(rec);
            }

            Ok(Self {
                vec: CompactArray { vec: v },
            })
        }
    }

    impl Serialize for CompactRecords {
        type Error = SerializeError;

        fn write(&self, buf: &mut [u8]) -> Result<usize, Self::Error> {
            // we can use the compact array serialization for this
            self.vec.write(buf)
        }
    }

    impl Deserialize for CompactRecords {
        type Error = ParseError;

        fn parse(buf: &[u8]) -> Result<(Self, usize), Self::Error> {
            // we can use the compact array deserialization for this
            let (vec, s) = CompactArray::parse(buf)?;

            Ok((Self { vec }, s))
        }
    }

    /// Represents a sequence of Kafka records as COMPACT_NULLABLE_BYTES. For a detailed
    /// description of records see Message Sets.
    /// baseOffset: int64
    /// batchLength: int32
    /// partitionLeaderEpoch: int32
    /// magic: int8 (current magic value is 2)
    /// crc: uint32
    /// attributes: int16
    ///     bit 0~2:
    ///         0: no compression
    ///         1: gzip
    ///         2: snappy
    ///         3: lz4
    ///         4: zstd
    ///     bit 3: timestampType
    ///     bit 4: isTransactional (0 means not transactional)
    ///     bit 5: isControlBatch (0 means not a control batch)
    ///     bit 6: hasDeleteHorizonMs (0 means baseTimestamp is not set as the delete horizon for compaction)
    ///     bit 7~15: unused
    /// lastOffsetDelta: int32
    /// baseTimestamp: int64
    /// maxTimestamp: int64
    /// producerId: int64
    /// producerEpoch: int16
    /// baseSequence: int32
    /// recordsCount: int32
    /// records: [Record]
    #[derive(Debug, Clone, Default)]
    pub struct RecordBatch {
        pub base_offset: i64,
        pub batch_length: i32,
        pub partition_leader_epoch: i32,
        pub magic: i8,
        pub crc: u32,
        pub attributes: RecordsAttribute,
        pub last_offset_delta: i32,
        pub base_timestamp: i64,
        pub max_timestamp: i64,
        pub producer_id: i64,
        pub producer_epoch: i16,
        pub base_sequence: i32,
        pub records_count: i32,
        pub records: Vec<Record>,
    }

    impl Deserialize for RecordBatch {
        type Error = ParseError;

        fn parse(mut buf: &[u8]) -> Result<(Self, usize), Self::Error> {
            let len = buf.len();

            let base_offset = buf.get_i64();
            let batch_length = buf.get_i32();

            // everything from here to batch_length

            let partition_leader_epoch = buf.get_i32();
            let magic = buf.get_i8();
            debug_assert_eq!(magic, 2);

            let crc = buf.get_u32();

            let (attributes, s) = RecordsAttribute::parse(buf)?;
            buf.advance(s);

            let last_offset_delta = buf.get_i32();
            let base_timestamp = buf.get_i64();
            let max_timestamp = buf.get_i64();
            let producer_id = buf.get_i64();
            let producer_epoch = buf.get_i16();
            let base_sequence = buf.get_i32();
            let records_count = buf.get_i32();

            let mut records = Vec::with_capacity(records_count as _);
            for _ in 0..records_count {
                let (record, s) = Record::parse(buf)?;
                records.push(record);

                buf.advance(s);
            }

            // this should be batch_length + 8 + 4

            let len = len - buf.remaining();

            debug_assert_eq!(len, batch_length as usize + 8 + 4);

            let s = Self {
                base_offset,
                batch_length,
                partition_leader_epoch,
                magic,
                crc,
                attributes,
                last_offset_delta,
                base_timestamp,
                max_timestamp,
                producer_id,
                producer_epoch,
                base_sequence,
                records_count,
                records,
            };

            Ok((s, len))
        }
    }

    impl Serialize for RecordBatch {
        type Error = SerializeError;

        fn write(&self, buf: &mut [u8]) -> Result<usize, Self::Error> {
            todo!()
        }
    }

    /// attributes: int16
    ///     bit 0-2:
    ///         0: no compression
    ///         1: gzip
    ///         2: snappy
    ///         3: lz4
    ///         4: zstd
    ///     bit 3: timestampType
    ///     bit 4: isTransactional (0 means not transactional)
    ///     bit 5: isControlBatch (0 means not a control batch)
    ///     bit 6: hasDeleteHorizonMs (0 means baseTimestamp is not set as the delete horizon for compaction)
    ///     bit 7-15: unused
    #[derive(Debug, Clone, Default)]
    pub struct RecordsAttribute {
        pub val: i16,
    }

    impl RecordsAttribute {
        pub fn compression(&self) -> RecordsCompression {
            let p = self.val & 0b11;
            match p {
                0b00 => RecordsCompression::NoCompression,
                0b01 => RecordsCompression::Gzip,
                0b10 => RecordsCompression::Snappy,
                0b11 => RecordsCompression::Lz4,
                _ => unreachable!("this branch should never be reached"),
            }
        }

        pub fn timestamp_type(&self) -> bool {
            (self.val & 0b100) > 1
        }

        pub fn is_transactional(&self) -> bool {
            (self.val & 0b1000) > 1
        }

        pub fn is_control_batch(&self) -> bool {
            (self.val & 0b1_0000) > 1
        }

        pub fn has_delete_horizon_ms(&self) -> bool {
            (self.val & 0b10_0000) > 1
        }
    }

    impl Deserialize for RecordsAttribute {
        type Error = ParseError;

        fn parse(mut buf: &[u8]) -> Result<(Self, usize), Self::Error> {
            let val = buf.get_i16();
            Ok((Self { val }, 2))
        }
    }

    #[derive(Debug, Clone, Default)]
    pub enum RecordsCompression {
        #[default]
        NoCompression,
        Gzip,
        Snappy,
        Lz4,
        Zstd,
    }

    /// length: varint
    /// attributes: int8
    ///     bit 0~7: unused
    /// timestampDelta: varlong
    /// offsetDelta: varint
    /// keyLength: varint
    /// key: byte[]
    /// valueLength: varint
    /// value: byte[]
    /// headersCount: varint
    /// Headers => [Header]
    #[derive(Debug, Clone, Default)]
    pub struct Record {
        pub length: Varint,
        pub attributes: i8,
        pub timestamp_delta: Varlong,
        pub offset_delta: Varint,
        pub key_length: Varint,
        pub key: Vec<u8>,
        pub value_length: Varint,
        pub value: Vec<u8>,
        pub headers_count: Varint,
        pub headers: Vec<RecordHeader>,
    }

    impl Serialize for Record {
        type Error = SerializeError;

        fn write(&self, buf: &mut [u8]) -> Result<usize, Self::Error> {
            todo!()
        }
    }

    impl Deserialize for Record {
        type Error = ParseError;

        fn parse(mut buf: &[u8]) -> Result<(Self, usize), Self::Error> {
            let len = buf.len();
            let (length, s) = Varint::parse(buf)?;
            buf.advance(s);

            let attributes = buf.get_i8();

            let (timestamp_delta, s) = Varlong::parse(buf)?;
            buf.advance(s);

            let (offset_delta, s) = Varint::parse(buf)?;
            buf.advance(s);

            let (key_length, s) = Varint::parse(buf)?;
            buf.advance(s);

            let mut key = vec![];
            if key_length.val > 0 {
                key.extend_from_slice(&buf[..(key_length.val as usize)]);
                buf.advance(key_length.val as usize);
            }

            let (value_length, s) = Varint::parse(buf)?;
            buf.advance(s);

            let mut value = vec![];
            if value_length.val > 0 {
                value.extend_from_slice(&buf[..(value_length.val as usize)]);
                buf.advance(value_length.val as usize);
            }

            let (headers_count, s) = Varint::parse(buf)?;
            buf.advance(s);

            let mut headers = Vec::with_capacity(headers_count.val as usize);
            for _ in 0..headers_count.val {
                let (header, s) = RecordHeader::parse(buf)?;
                buf.advance(s);

                headers.push(header);
            }

            let s = Self {
                length,
                attributes,
                timestamp_delta,
                offset_delta,
                key_length,
                key,
                value_length,
                value,
                headers_count,
                headers,
            };

            let len = len - buf.remaining();

            Ok((s, len))
        }
    }

    /// headerKeyLength: varint
    /// headerKey: String
    /// headerValueLength: varint
    /// Value: byte[]
    #[derive(Debug, Clone, Default)]
    pub struct RecordHeader {
        pub header_key_length: Varint,
        pub header_key: String,
        pub header_value_length: Varint,
        pub value: Vec<u8>,
    }

    impl Deserialize for RecordHeader {
        type Error = ParseError;

        fn parse(buf: &[u8]) -> Result<(Self, usize), Self::Error> {
            todo!()
        }
    }

    impl Serialize for RecordHeader {
        type Error = SerializeError;

        fn write(&self, buf: &mut [u8]) -> Result<usize, Self::Error> {
            todo!()
        }
    }
}

pub mod requests {
    use super::primitives::{CompactString, Deserialize, NullableString, ParseError, TaggedFields};
    use bytes::buf::Buf;

    /// Request Header v1 => request_api_key request_api_version correlation_id client_id
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
        pub _tagged_fields: TaggedFields,
    }

    impl Deserialize for Header {
        type Error = ParseError;

        fn parse(mut buf: &[u8]) -> Result<(Self, usize), Self::Error> {
            let len = buf.len();

            let request_api_key = buf.get_i16();
            let request_api_version = buf.get_i16();
            let correlation_id = buf.get_i32();

            let (client_id, s) = NullableString::parse(buf)?;
            buf.advance(s);

            let (_tagged_fields, s) = TaggedFields::parse(buf)?;
            buf.advance(s);

            let len = len - buf.remaining();

            Ok((
                Self {
                    request_api_key,
                    request_api_version,
                    correlation_id,
                    client_id,
                    _tagged_fields,
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
        Fetch(fetch::Fetch),
        ApiVersions(api_versions::ApiVersions),
    }

    impl TryFrom<&[u8]> for Request {
        type Error = ParseError;

        fn try_from(mut buf: &[u8]) -> Result<Self, Self::Error> {
            let size: usize = buf.get_i32().try_into()?;

            if size != buf.len() {
                return Err(ParseError::InvalidSize(size, buf.len()));
            }

            let (header, s) = Header::parse(buf)?;
            buf.advance(s);
            let (rt, _ss) = RequestType::parse(header.request_api_key, buf)?;

            Ok(Self {
                header,
                request: rt,
            })
        }
    }

    impl RequestType {
        fn parse(request_api_key: i16, buf: &[u8]) -> Result<(Self, usize), ParseError> {
            let s = match request_api_key {
                1 => {
                    let s = fetch::Fetch::parse(buf)?;
                    (RequestType::Fetch(s.0), s.1)
                }
                18 => {
                    let s = api_versions::ApiVersions::parse(buf)?;
                    (RequestType::ApiVersions(s.0), s.1)
                }
                _ => unimplemented!("no such request key"),
            };
            Ok(s)
        }
    }

    pub mod fetch {
        use crate::messages::primitives::{CompactArray, TaggedFields, Uuid};

        use super::*;

        /// Fetch Request (Version: 16) => max_wait_ms min_bytes max_bytes isolation_level session_id session_epoch [topics] [forgotten_topics_data] rack_id _tagged_fields
        ///   max_wait_ms => INT32
        ///   min_bytes => INT32
        ///   max_bytes => INT32
        ///   isolation_level => INT8
        ///   session_id => INT32
        ///   session_epoch => INT32
        ///   topics => topic_id [partitions] _tagged_fields
        ///     topic_id => UUID
        ///     partitions => partition current_leader_epoch fetch_offset last_fetched_epoch log_start_offset partition_max_bytes _tagged_fields
        ///       partition => INT32
        ///       current_leader_epoch => INT32
        ///       fetch_offset => INT64
        ///       last_fetched_epoch => INT32
        ///       log_start_offset => INT64
        ///       partition_max_bytes => INT32
        ///   forgotten_topics_data => topic_id [partitions] _tagged_fields
        ///     topic_id => UUID
        ///     partitions => INT32
        ///   rack_id => COMPACT_STRING
        #[derive(Debug, Clone)]
        pub struct Fetch {
            pub max_wait_ms: i32,
            pub min_bytes: i32,
            pub max_bytes: i32,
            pub isolation_level: i8,
            pub session_id: i32,
            pub session_epoch: i32,
            pub topics: CompactArray<Topic>,
            pub forgotten_topics_data: CompactArray<ForgottenTopicsData>,
            pub rack_id: CompactString,
            pub _tagged_fields: TaggedFields,
        }

        impl Deserialize for Fetch {
            type Error = ParseError;

            fn parse(mut buf: &[u8]) -> Result<(Self, usize), Self::Error> {
                let len = buf.len();
                let max_wait_ms = buf.get_i32();
                let min_bytes = buf.get_i32();
                let max_bytes = buf.get_i32();
                let isolation_level = buf.get_i8();
                let session_id = buf.get_i32();
                let session_epoch = buf.get_i32();

                let (topics, s) = CompactArray::parse(buf)?;
                buf.advance(s);

                let (forgotten_topics_data, s) = CompactArray::parse(buf)?;
                buf.advance(s);

                let (rack_id, s) = CompactString::parse(buf)?;
                buf.advance(s);

                let (_tagged_fields, s) = TaggedFields::parse(buf)?;
                buf.advance(s);

                let len = len - buf.remaining();

                let fetch = Self {
                    max_wait_ms,
                    min_bytes,
                    max_bytes,
                    isolation_level,
                    session_id,
                    session_epoch,
                    topics,
                    forgotten_topics_data,
                    rack_id,
                    _tagged_fields,
                };

                Ok((fetch, len))
            }
        }

        ///   topics => topic_id [partitions] _tagged_fields
        ///     topic_id => UUID
        ///     partitions => partition current_leader_epoch fetch_offset last_fetched_epoch log_start_offset partition_max_bytes _tagged_fields
        ///       partition => INT32
        ///       current_leader_epoch => INT32
        ///       fetch_offset => INT64
        ///       last_fetched_epoch => INT32
        ///       log_start_offset => INT64
        ///       partition_max_bytes => INT32
        #[derive(Debug, Clone)]
        pub struct Topic {
            pub topic_id: Uuid,
            pub partitions: CompactArray<Partition>,
            pub _tagged_fields: TaggedFields,
        }

        impl Deserialize for Topic {
            type Error = ParseError;

            fn parse(mut buf: &[u8]) -> Result<(Self, usize), Self::Error> {
                let len = buf.len();

                let (topic_id, s) = Uuid::parse(buf)?;
                buf.advance(s);

                let (partitions, s) = CompactArray::parse(buf)?;
                buf.advance(s);

                let (_tagged_fields, s) = TaggedFields::parse(buf)?;
                buf.advance(s);

                let topics = Self {
                    topic_id,
                    partitions,
                    _tagged_fields,
                };

                Ok((topics, len - buf.remaining()))
            }
        }

        ///     partitions => partition current_leader_epoch fetch_offset last_fetched_epoch log_start_offset partition_max_bytes _tagged_fields
        ///       partition => INT32
        ///       current_leader_epoch => INT32
        ///       fetch_offset => INT64
        ///       last_fetched_epoch => INT32
        ///       log_start_offset => INT64
        ///       partition_max_bytes => INT32
        #[derive(Debug, Clone, Default)]
        pub struct Partition {
            pub partition: i32,
            pub current_leader_epoch: i32,
            pub fetch_offset: i64,
            pub last_fetched_epoch: i32,
            pub log_start_offset: i64,
            pub partition_max_bytes: i32,
            pub _tagged_fields: TaggedFields,
        }

        impl Deserialize for Partition {
            type Error = ParseError;

            fn parse(mut buf: &[u8]) -> Result<(Self, usize), Self::Error> {
                let len = buf.len();

                let partition = buf.get_i32();
                let current_leader_epoch = buf.get_i32();
                let fetch_offset = buf.get_i64();
                let last_fetched_epoch = buf.get_i32();
                let log_start_offset = buf.get_i64();
                let partition_max_bytes = buf.get_i32();

                let (_tagged_fields, s) = TaggedFields::parse(buf)?;
                buf.advance(s);

                let p = Self {
                    partition,
                    current_leader_epoch,
                    fetch_offset,
                    last_fetched_epoch,
                    log_start_offset,
                    partition_max_bytes,
                    _tagged_fields,
                };

                Ok((p, len - buf.remaining()))
            }
        }

        ///   forgotten_topics_data => topic_id [partitions] _tagged_fields
        ///     topic_id => UUID
        ///     partitions => INT32
        #[derive(Debug, Clone)]
        pub struct ForgottenTopicsData {
            pub topic_id: Uuid,
            pub partitions: CompactArray<i32>,
            pub _tagged_fields: TaggedFields,
        }

        impl Deserialize for ForgottenTopicsData {
            type Error = ParseError;

            fn parse(mut buf: &[u8]) -> Result<(Self, usize), Self::Error> {
                let len = buf.len();

                let (uuid, s) = Uuid::parse(buf)?;
                buf.advance(s);

                let (partitions, s) = CompactArray::parse(buf)?;
                buf.advance(s);

                let (_tagged_fields, s) = TaggedFields::parse(buf)?;
                buf.advance(s);

                Ok((
                    Self {
                        topic_id: uuid,
                        partitions,
                        _tagged_fields,
                    },
                    len - buf.remaining(),
                ))
            }
        }
    }

    pub mod api_versions {
        use super::*;
        #[derive(Debug, Clone)]
        pub struct ApiVersions {
            pub client_software_name: CompactString,
            pub client_software_version: CompactString,
        }

        impl Deserialize for ApiVersions {
            type Error = ParseError;

            fn parse(buf: &[u8]) -> Result<(Self, usize), Self::Error> {
                let (client_software_name, s) = CompactString::parse(buf)?;
                let (client_software_version, a) = CompactString::parse(&buf[s..])?;
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
}

pub mod responses {
    use super::primitives::{CompactArray, Serialize, SerializeError, TaggedFields};
    use super::{ApiKeys, ErrorCode};
    use bytes::buf::BufMut;

    #[derive(Debug, Clone)]
    pub struct Response {
        pub header: Header,
        pub response: ResponseType,
    }

    impl Serialize for Response {
        type Error = SerializeError;

        fn write(&self, mut buf: &mut [u8]) -> Result<usize, Self::Error> {
            let mut s = 0;
            s += self.header.write(&mut buf[4..])?;
            s += self.response.write(&mut buf[4 + s..])?;
            buf.put_i32(s as i32);

            Ok(4 + s)
        }
    }

    /// Response Header v0 => correlation_id
    ///   correlation_id => INT32
    /// Response Header v1 => correlation_id _tagged_fields
    ///   correlation_id => INT32
    #[derive(Debug, Clone)]
    pub enum Header {
        V0 {
            correlation_id: i32,
        },
        V1 {
            correlation_id: i32,
            _tagged_fields: TaggedFields,
        },
    }

    impl Serialize for Header {
        type Error = SerializeError;

        fn write(&self, mut buf: &mut [u8]) -> Result<usize, Self::Error> {
            match self {
                Header::V0 { correlation_id } => {
                    let len = buf.len();
                    buf.put_i32(*correlation_id);
                    let s = len - buf.remaining_mut();

                    Ok(s)
                }
                Header::V1 {
                    correlation_id,
                    _tagged_fields,
                } => {
                    let len = buf.len();
                    buf.put_i32(*correlation_id);
                    let mut s = len - buf.remaining_mut();
                    s += _tagged_fields.write(buf)?;

                    Ok(s)
                }
            }
        }
    }

    #[derive(Debug, Clone)]
    pub enum ResponseType {
        Fetch(fetch::Fetch),
        ApiVersions(api_version::ApiVersions),
    }

    impl Serialize for ResponseType {
        type Error = SerializeError;

        fn write(&self, buf: &mut [u8]) -> Result<usize, Self::Error> {
            match self {
                ResponseType::Fetch(fetch) => fetch.write(buf),
                ResponseType::ApiVersions(api) => api.write(buf),
            }
        }
    }

    pub mod api_version {
        use super::*;

        /// ApiVersions Response (Version: 3) => error_code [api_keys] throttle_time_ms _tagged_fields
        ///   error_code => INT16
        ///   api_keys => api_key min_version max_version _tagged_fields
        ///     api_key => INT16
        ///     min_version => INT16
        ///     max_version => INT16
        ///   throttle_time_ms => INT32
        #[derive(Debug, Clone, Default)]
        pub struct ApiVersions {
            pub error_code: ErrorCode,
            pub api_keys: CompactArray<ApiKey>,
            pub throttle_time_ms: i32,
            pub _tagged_fields: TaggedFields,
        }

        impl Serialize for ApiVersions {
            type Error = SerializeError;

            fn write(&self, buf: &mut [u8]) -> Result<usize, Self::Error> {
                (&mut buf[..]).put_i16(self.error_code as i16);
                let mut s = 2;

                s += self.api_keys.write(&mut buf[s..])?;

                (&mut buf[s..]).put_i32(self.throttle_time_ms);
                s += 4;

                s += self._tagged_fields.write(&mut buf[s..])?;

                Ok(s)
            }
        }

        ///   api_keys => api_key min_version max_version _tagged_fields
        ///     api_key => INT16
        ///     min_version => INT16
        ///     max_version => INT16
        #[derive(Debug, Clone, Default)]
        pub struct ApiKey {
            pub api_key: ApiKeys,
            pub min_version: i16,
            pub max_version: i16,
            pub _tagged_fields: TaggedFields,
        }

        impl Serialize for ApiKey {
            type Error = SerializeError;

            fn write(&self, mut buf: &mut [u8]) -> Result<usize, Self::Error> {
                let len = buf.len();
                buf.put_i16(self.api_key as i16);
                buf.put_i16(self.min_version);
                buf.put_i16(self.max_version);

                // _tagged_fields
                let mut len = len - buf.remaining_mut();
                len += self._tagged_fields.write(buf)?;
                Ok(len)
            }
        }
    }

    pub mod fetch {
        use crate::messages::disk::CompactRecords;
        use crate::messages::primitives::Uuid;

        use super::*;

        /// Fetch Response (Version: 16) => throttle_time_ms error_code session_id [responses] _tagged_fields
        ///   throttle_time_ms => INT32
        ///   error_code => INT16
        ///   session_id => INT32
        ///   responses => topic_id [partitions] _tagged_fields
        ///     topic_id => UUID
        ///     partitions => partition_index error_code high_watermark last_stable_offset log_start_offset [aborted_transactions] preferred_read_replica records _tagged_fields
        ///       partition_index => INT32
        ///       error_code => INT16
        ///       high_watermark => INT64
        ///       last_stable_offset => INT64
        ///       log_start_offset => INT64
        ///       aborted_transactions => producer_id first_offset _tagged_fields
        ///         producer_id => INT64
        ///         first_offset => INT64
        ///       preferred_read_replica => INT32
        ///       records => COMPACT_RECORDS
        #[derive(Debug, Clone, Default)]
        pub struct Fetch {
            pub throttle_time_ms: i32,
            pub error_code: ErrorCode,
            pub session_id: i32,
            pub responses: CompactArray<Response>,
            pub _tagged_fields: TaggedFields,
        }

        impl Serialize for Fetch {
            type Error = SerializeError;

            fn write(&self, mut buf: &mut [u8]) -> Result<usize, Self::Error> {
                let len = buf.len();

                buf.put_i32(self.throttle_time_ms);
                buf.put_i16(self.error_code as i16);
                buf.put_i32(self.session_id);

                let diff = len - buf.remaining_mut();

                let s = self.responses.write(buf)?;
                let ss = self._tagged_fields.write(&mut buf[s..])?;

                Ok(diff + s + ss)
            }
        }

        ///   responses => topic_id [partitions] _tagged_fields
        ///     topic_id => UUID
        ///     partitions => partition_index error_code high_watermark last_stable_offset log_start_offset [aborted_transactions] preferred_read_replica records _tagged_fields
        ///       partition_index => INT32
        ///       error_code => INT16
        ///       high_watermark => INT64
        ///       last_stable_offset => INT64
        ///       log_start_offset => INT64
        ///       aborted_transactions => producer_id first_offset _tagged_fields
        ///         producer_id => INT64
        ///         first_offset => INT64
        ///       preferred_read_replica => INT32
        ///       records => COMPACT_RECORDS
        #[derive(Debug, Clone, Default)]
        pub struct Response {
            pub topic_id: Uuid,
            pub partitions: CompactArray<Partition>,
            pub _tagged_fields: TaggedFields,
        }

        impl Serialize for Response {
            type Error = SerializeError;

            fn write(&self, buf: &mut [u8]) -> Result<usize, Self::Error> {
                let mut s = self.topic_id.write(buf)?;
                s += self.partitions.write(&mut buf[s..])?;
                s += self._tagged_fields.write(&mut buf[s..])?;
                Ok(s)
            }
        }

        ///     partitions => partition_index error_code high_watermark last_stable_offset log_start_offset [aborted_transactions] preferred_read_replica records _tagged_fields
        ///       partition_index => INT32
        ///       error_code => INT16
        ///       high_watermark => INT64
        ///       last_stable_offset => INT64
        ///       log_start_offset => INT64
        ///       aborted_transactions => producer_id first_offset _tagged_fields
        ///         producer_id => INT64
        ///         first_offset => INT64
        ///       preferred_read_replica => INT32
        ///       records => COMPACT_RECORDS
        #[derive(Debug, Clone, Default)]
        pub struct Partition {
            pub partition_index: i32,
            pub error_code: ErrorCode,
            pub high_watermark: i64,
            pub last_stable_offset: i64,
            pub log_start_offset: i64,
            pub aborted_transactions: CompactArray<AbortedTransactions>,
            pub preferred_read_replica: i32,
            pub records: CompactRecords,
            pub _tagged_fields: TaggedFields,
        }

        impl Serialize for Partition {
            type Error = SerializeError;

            fn write(&self, mut buf: &mut [u8]) -> Result<usize, Self::Error> {
                let len = buf.len();
                buf.put_i32(self.partition_index);
                buf.put_i16(self.error_code as i16);
                buf.put_i64(self.high_watermark);
                buf.put_i64(self.last_stable_offset);
                buf.put_i64(self.log_start_offset);

                let mut s = len - buf.remaining_mut();
                s += self.aborted_transactions.write(buf)?;
                (&mut buf[s..]).put_i32(self.preferred_read_replica);
                s += std::mem::size_of::<i32>();
                s += self.records.write(&mut buf[s..])?;
                s += self._tagged_fields.write(&mut buf[s..])?;

                Ok(s)
            }
        }

        ///       aborted_transactions => producer_id first_offset _tagged_fields
        ///         producer_id => INT64
        ///         first_offset => INT64
        #[derive(Debug, Clone, Default)]
        pub struct AbortedTransactions {
            pub producer_id: i64,
            pub first_offset: i64,
            pub _tagged_fields: TaggedFields,
        }

        impl Serialize for AbortedTransactions {
            type Error = SerializeError;

            fn write(&self, mut buf: &mut [u8]) -> Result<usize, Self::Error> {
                let len = buf.len();
                buf.put_i64(self.producer_id);
                buf.put_i64(self.first_offset);

                let len = len - buf.remaining_mut();
                let s = self._tagged_fields.write(buf)?;

                Ok(len + s)
            }
        }
    }
}
