use crate::{
    messages::{
        primitives::{CompactArray, TaggedFields},
        requests,
        responses::{self, api_version::ApiKey},
        ApiKeys, ErrorCode,
    },
    meta::Meta,
};

static SUPPORTED_COMMANDS: [ApiKey; 2] = [
    ApiKey {
        api_key: ApiKeys::Fetch,
        min_version: 16,
        max_version: 16,
        _tagged_fields: TaggedFields,
    },
    ApiKey {
        min_version: 4,
        max_version: 4,
        api_key: ApiKeys::ApiVersions,
        _tagged_fields: TaggedFields {},
    },
];

pub fn process(msg: &[u8], msg_out: &mut Vec<u8>, meta: &Meta) -> anyhow::Result<usize> {
    let req = requests::Request::try_from(msg)?;

    let res = handle_request(req, meta)?;

    let s = res.write(msg_out)?;
    Ok(s)
}

fn handle_request(req: requests::Request, meta: &Meta) -> anyhow::Result<responses::Response> {
    use responses::ResponseType;
    let header = responses::Header::V1 {
        correlation_id: req.header.correlation_id,
        _tagged_fields: TaggedFields,
    };

    let response = match req.request {
        requests::RequestType::ApiVersions(api) => {
            // uses special header

            let header = responses::Header::V0 {
                correlation_id: req.header.correlation_id,
            };
            let response = handle_api_version(req.header, api).map(ResponseType::ApiVersions)?;

            return Ok(responses::Response { header, response });
        }
        requests::RequestType::Fetch(fetch) => {
            fetch::handle_fetch(req.header, fetch, meta).map(ResponseType::Fetch)?
        }
    };

    Ok(responses::Response { header, response })
}

mod fetch {

    use std::path::PathBuf;

    use crate::{
        messages::{
            primitives::CompactArray,
            requests,
            responses::{self, fetch::Response},
            ErrorCode,
        },
        meta::Meta,
    };
    use responses::fetch::*;

    pub fn handle_fetch(
        header: requests::Header,
        fetch_request: requests::fetch::Fetch,
        meta: &Meta,
    ) -> anyhow::Result<responses::fetch::Fetch> {
        if header.request_api_version != 16 {
            return Ok(Fetch {
                session_id: fetch_request.session_id,
                error_code: ErrorCode::UnsupportedVersion,
                ..Default::default()
            });
        }

        let responses = handle_responses(&fetch_request, meta)?;

        let responses = CompactArray { vec: responses };

        Ok(Fetch {
            responses,
            session_id: fetch_request.session_id,
            ..Default::default()
        })
    }

    fn handle_responses(
        fetch_request: &requests::fetch::Fetch,
        meta: &Meta,
    ) -> anyhow::Result<Vec<Response>> {
        let topics = meta.topic_map();

        let mut res = Vec::with_capacity(fetch_request.topics.vec.len());

        for topic in &fetch_request.topics.vec {
            let error_code = match topics.contains_key(&topic.topic_id) {
                true => ErrorCode::NoError,
                false => ErrorCode::UnknownTopic,
            };

            let mut partitions = vec![];
            for p in &topic.partitions.vec {
                let path = meta.path.clone();
                eprintln!("{}", path.display());

                let part = Partition {
                    partition_index: p.partition,
                    error_code,
                    ..Default::default()
                };
                partitions.push(part);
            }

            let partitions = CompactArray { vec: partitions };
            let response = Response {
                topic_id: topic.topic_id,
                partitions,
                ..Default::default()
            };

            res.push(response);
        }
        Ok(res)
    }
}

fn handle_api_version(
    header: requests::Header,
    _api: requests::api_versions::ApiVersions,
) -> anyhow::Result<responses::api_version::ApiVersions> {
    use responses::api_version::*;
    let api = match header.request_api_version {
        4 => ApiVersions {
            api_keys: CompactArray {
                vec: SUPPORTED_COMMANDS.to_vec(),
            },
            ..Default::default()
        },
        _ => ApiVersions {
            error_code: ErrorCode::UnsupportedVersion,
            ..Default::default()
        },
    };

    Ok(api)
}

#[cfg(test)]
mod test {
    use pretty_assertions::assert_eq;

    use tempfile::{tempdir, TempDir};

    use crate::{
        messages::{disk::RecordBatch, primitives::Deserialize},
        meta,
        test_files::{write_files, FILES},
    };

    use super::*;

    #[test]
    fn test_full_parse_api_version() {
        let (meta, _tmp_dir) = fetch_file();

        let arr = [
            0x00, 0x00, 0x00, 0x23, // len = 0x23 => 35
            0x00, 0x12, // request_api_key = 0x12 => 18
            0x00, 0x04, // request_api_version = 0x04 => 4
            0x6d, 0xfe, 0xa9, 0x9a, // correlation_id = 0x6d 0xfe 0xa9 0x9a
            0x00, 0x09, // client_id len = 0x09
            0x6b, 0x61, 0x66, 0x6b, 0x61, 0x2d, 0x63, 0x6c, 0x69, // kafka-cli
            0x00, // _tagged_fields
            0x0a, // client_software_name len = 0x0a => 10 ==> 10 - 1 (CompactString)
            0x6b, 0x61, 0x66, 0x6b, 0x61, 0x2d, 0x63, 0x6c, 0x69, // kafka-cli
            0x04, // client_software_version len = 0x04 => 4 ==> 4 - 1 (CompactString)
            0x30, 0x2e, 0x31, // 0.1
            0x0,  // _tagged_fields
        ];

        let exp = [
            0x00, 0x00, 0x00, 0x1a, // len
            0x6d, 0xfe, 0xa9, 0x9a, // correlation_id = 0x6d 0xfe 0xa9 0x9a
            0x00, 0x00, // error code = no error
            0x03, // len api_keys (compact array N + 1)
            0x00, 0x01, // api_key
            0x00, 0x10, // min_version
            0x00, 0x10, // max_version
            0x00, // _tagged_fields
            0x00, 0x12, // api_key
            0x00, 0x04, // min_version
            0x00, 0x04, // max_version
            0x00, // _tagged_fields
            0x00, 0x00, 0x00, 0x00, // throttle_time_ms
            0x00, // _tagged_fields
        ];

        let mut buf = Vec::with_capacity(150);
        process(&arr, &mut buf, &meta).expect("unable to extract");
        assert_eq!(exp, &buf[..]);
    }

    fn fetch_file() -> (Meta, TempDir) {
        let tmp_dir = write_files();
        (
            Meta::from_cluster_metadata(meta::PATH).expect("able to read meta cluster"),
            tmp_dir,
        )
    }

    #[test]
    fn test_fetch_file() {
        let (meta, _) = fetch_file();
        assert_eq!(meta.rec.vec.vec[0].base_offset, 0x01);
    }

    #[test]
    fn test_parse_record_batch() {
        let arr = [
            // -- RecordBatch
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, // baseOffset
            0x00, 0x00, 0x00, 0x4F, // batchLength
            0x00, 0x00, 0x00, 0x01, // partitionLeaderEpoch
            0x02, // magic
            0xB0, 0x69, 0x45, 0x7C, // CRC
            0x00, 0x00, // attributes
            0x00, 0x00, 0x00, 0x00, // lastOffsetDelta
            0x00, 0x00, 0x01, 0x91, 0xE0, 0x5A, 0xF8, 0x18, // baseTimestamp
            0x00, 0x00, 0x01, 0x91, 0xE0, 0x5A, 0xF8, 0x18, // maxTimestamp
            0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, // producerId
            0xFF, 0xFF, // producerEpoch
            0xFF, 0xFF, 0xFF, 0xFF, // baseSequence
            0x00, 0x00, 0x00, 0x01, // recordsCount
            // -- Record
            0x3A, // length -> 29
            0x00, // attribute
            0x00, // timestampDelta
            0x00, // offsetDelta
            0x01, // keyLenght -> -1
            0x2E, // valueLenght 23 =>
            0x01, // frameVersion
            0x0C, // record type
            0x00, 0x11, 0x6D, 0x65, 0x74, 0x61, 0x64, 0x61, 0x74, 0x61, 0x2E, 0x76, 0x65, 0x72,
            0x73, 0x69, 0x6F, 0x6E, 0x00, 0x14, 0x00, // value
            0x00, // header count
        ];
        let mut arr = std::io::Cursor::new(arr);

        let rb = RecordBatch::parse(&mut arr).expect("able to parse record batch");
        assert_eq!(rb.base_offset, 0x01);
    }

    #[test]
    fn test_only_parse_fetch_no_topics() {
        let arr = [
            0x00, 0x00, 0x00, 0x30, // len
            0x00, 0x01, // request_api_key = 0x01
            0x00, 0x10, // request_api_version = 16
            0x1a, 0xf6, 0xe0, 0x6e, // correlation_id
            0x00, 0x0c, // client_id
            0x6b, 0x61, 0x66, 0x6b, 0x61, 0x2d, 0x74, 0x65, 0x73, 0x74, 0x65,
            0x72, // kafka-tester
            0x00, // _tagged_fields
            0x00, 0x00, 0x01, 0xf4, // max_wait_ms
            0x00, 0x00, 0x00, 0x01, // min_bytes
            0x03, 0x20, 0x00, 0x00, // max_bytes
            0x00, // isolation_level
            0x02, 0x02, 0x02, 0x02, // session_id
            0x00, 0x00, 0x00, 0x00, // session_epoch
            0x01, // topic count
            0x01, // forgotten_topics_data count
            0x01, // rack_id count
            0x00, // _tagged_fields
        ];

        let req = requests::Request::try_from(&arr[..]).expect("able to parse");
        assert_eq!(
            req.header.correlation_id,
            i32::from_be_bytes([0x1a, 0xf6, 0xe0, 0x6e])
        );
    }

    #[test]
    fn test_full_parse_fetch_no_topics() {
        let (meta, _tmp_dir) = fetch_file();
        let arr = [
            0x00, 0x00, 0x00, 0x30, // len
            0x00, 0x01, // request_api_key = 0x01
            0x00, 0x10, // request_api_version = 16
            0x1a, 0xf6, 0xe0, 0x6e, // correlation_id
            0x00, 0x0c, // client_id
            0x6b, 0x61, 0x66, 0x6b, 0x61, 0x2d, 0x74, 0x65, 0x73, 0x74, 0x65,
            0x72, // kafka-tester
            0x00, // _tagged_fields
            0x00, 0x00, 0x01, 0xf4, // max_wait_ms
            0x00, 0x00, 0x00, 0x01, // min_bytes
            0x03, 0x20, 0x00, 0x00, // max_bytes
            0x00, // isolation_level
            0x02, 0x02, 0x02, 0x02, // session_id
            0x00, 0x00, 0x00, 0x00, // session_epoch
            0x01, // topic count
            0x01, // forgotten_topics_data count
            0x01, // rack_id count
            0x00, // _tagged_fields
        ];

        let exp = [
            0x00, 0x00, 0x00, 0x11, // len
            0x1a, 0xf6, 0xe0, 0x6e, // correlation_id
            0x00, // _tagged_fields
            0x00, 0x00, 0x00, 0x00, // throttle_time_ms
            0x00, 0x00, // error code
            0x02, 0x02, 0x02, 0x02, // session_id
            0x01, // responses count
            0x00, // _tagged_fields
        ];
        let mut buf = Vec::with_capacity(150);
        process(&arr, &mut buf, &meta).expect("unable to extract");

        assert_eq!(exp, &buf[..]);
    }

    #[test]
    fn test_full_parse_fetch_unknown_topic() {
        let (meta, _tmp_dir) = fetch_file();
        let arr = [
            0x00, 0x00, 0x00, 0x60, // len
            0x00, 0x01, // request_api_key = 0x01 -> fetch
            0x00, 0x10, // request_api_version = 0x10 = 16
            0x02, 0x7e, 0x9f, 0xe7, // correlation_id
            0x00, 0x09, // client_id
            0x6b, 0x61, 0x66, 0x6b, 0x61, 0x2d, 0x63, 0x6c, 0x69, // kafka-cli
            0x00, // _tagged_fields
            0x00, 0x00, 0x01, 0xf4, // max_wait_ms
            0x00, 0x00, 0x00, 0x01, // min_bytes
            0x03, 0x20, 0x00, 0x00, // max_bytes
            0x00, // isolation_level
            0x00, 0x00, 0x00, 0x00, // session_id
            0x00, 0x00, 0x00, 0x00, // session_epoch
            0x02, // topic count = N + 1 -> 1 topic
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // UUID (part 1)
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x34, 0x73, // UUID (part 2)
            0x02, // partition count = N + 1 -> 1 parition
            0x00, 0x00, 0x00, 0x00, // parition
            0xff, 0xff, 0xff, 0xff, // current_leader_epoch
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // fetch_offset
            0xff, 0xff, 0xff, 0xff, // last_fetched_offset
            0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, // log_start_offset
            0x00, 0x10, 0x00, 0x00, // partition_max_bytes
            0x00, // _tagged_fields (partitions)
            0x00, // _tagged_fields (topics)
            0x01, // forgotten_topics_data count
            0x01, // rack_id count
            0x00, // _tagged_fields
        ];

        let exp = [
            0x00, 0x00, 0x00, 0x48, // len
            0x02, 0x7e, 0x9f, 0xe7, // correlation_id
            0x00, // _tagged_fields
            0x00, 0x00, 0x00, 0x00, // throttle_time_ms
            0x00, 0x00, // error_code
            0x00, 0x00, 0x00, 0x00, // session_id
            0x02, // responses count
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // UUID (part 1)
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x34, 0x73, // UUID (part 2)
            0x02, // partition count
            0x00, 0x00, 0x00, 0x00, // parition_index
            0x00, 0x64, // error_code
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // high_watermark
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // last_stable_offset
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // log_start_offset
            0x01, // aborted_transactions count
            0x00, 0x00, 0x00, 0x00, // preferred_read_replica
            0x01, // records
            0x00, // _tagged_fields
            0x00, // _tagged_fields
            0x00, // _tagged_fields
        ];

        let mut buf = Vec::with_capacity(150);
        process(&arr, &mut buf, &meta).expect("unable to extract");

        assert_eq!(exp, &buf[..]);
    }

    #[test]
    fn test_full_parse_fetch_empty_topic() {
        let (meta, _tmp_dir) = fetch_file();
        let arr = [
            0x00, 0x00, 0x00, 0x60, // len
            0x00, 0x01, // request_api_key -> fetch
            0x00, 0x10, // request_api_version -> 16
            0x25, 0xfc, 0x97, 0x90, // correlation_id
            0x00, 0x09, // client_id
            0x6b, 0x61, 0x66, 0x6b, 0x61, 0x2d, 0x63, 0x6c, 0x69, // kafka-cli
            0x00, // _tagged_fields
            0x00, 0x00, 0x01, 0xf4, // max_wait_ms
            0x00, 0x00, 0x00, 0x01, // min_bytes
            0x03, 0x20, 0x00, 0x00, // max_bytes
            0x00, // isolation_level
            0x00, 0x00, 0x00, 0x00, // session_id
            0x00, 0x00, 0x00, 0x00, // session_epoch
            0x02, // topic count => N + 1 -> 1 topic
            // [0, 0, 0, 0, 0, 0, 64, 0, 128, 0, 0, 0, 0, 0, 0, 103]
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x40, 0x00, // UUID (part 1)
            0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x67, // UUID (part 2)
            0x02, // paritions -> N + 1 -> 1 parition
            0x00, 0x00, 0x00, 0x00, // partition
            0xff, 0xff, 0xff, 0xff, // current_leader_epoch
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // fetch_offset
            0xff, 0xff, 0xff, 0xff, // last_fetched_offset
            0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, // log_start_offset
            0x00, 0x10, 0x00, 0x00, // partition_max_bytes
            0x00, // _tagged_fields
            0x00, // _tagged_fields
            0x01, // forgotten_topics_data count
            0x01, // rack_id count
            0x00, // _tagged_fields
        ];

        let mut buf = Vec::with_capacity(150);
        let _len = process(&arr, &mut buf, &meta).expect("unable to extract");
    }
}
