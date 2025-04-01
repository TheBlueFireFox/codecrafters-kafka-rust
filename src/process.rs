use crate::messages::{
    primitives::{CompactArray, Serialize, TaggedFields},
    requests,
    responses::{self, api_version::ApiKey},
    ApiKeys, ErrorCode,
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

pub fn process(msg: &[u8], msg_out: &mut [u8]) -> anyhow::Result<usize> {
    let req = requests::Request::try_from(msg)?;

    let res = handle_request(req)?;

    let s = res.write(msg_out)?;
    Ok(s)
}

fn handle_request(req: requests::Request) -> anyhow::Result<responses::Response> {
    let header = responses::Header {
        correlation_id: req.header.correlation_id,
    };
    let response = match req.request {
        requests::RequestType::Fetch(fetch) => handle_fetch(req.header, fetch)?,
        requests::RequestType::ApiVersions(api) => handle_api_version(req.header, api)?,
    };

    Ok(responses::Response { header, response })
}

fn handle_fetch(
    _header: requests::Header,
    _fetch: requests::fetch::Fetch,
) -> anyhow::Result<responses::ResponseType> {
    todo!()
}

fn handle_api_version(
    header: requests::Header,
    _api: requests::api_versions::ApiVersions,
) -> anyhow::Result<responses::ResponseType> {
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

    Ok(responses::ResponseType::ApiVersions(api))
}

#[test]
fn test_full_parse() {
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

    let mut buf = [0; 100];
    let len = process(&arr, &mut buf).expect("unable to extract");
    assert_eq!(exp, &buf[..len]);
}
