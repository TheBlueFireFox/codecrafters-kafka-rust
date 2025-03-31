use crate::messages::{
    requests,
    responses::{self, Serialize},
    ErrorCode,
};

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
        requests::RequestType::ApiVersions(_api) => {
            let api = match req.header.request_api_version {
                4 => responses::ApiVersions {
                    error_code: None,
                    api_keys: responses::ApiKeys {
                        keys: vec![responses::ApiKey {
                            min_version: 4,
                            max_version: 4,
                            api_key: 0x12,
                        }],
                    },
                    throttle_time_ms: 0,
                },
                _ => responses::ApiVersions {
                    error_code: Some(ErrorCode::UnsupportedVersion),
                    api_keys: responses::ApiKeys { keys: vec![] },
                    throttle_time_ms: 0,
                },
            };

            responses::ResponseType::ApiVersions(api)
        }
    };

    Ok(responses::Response { header, response })
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
        0x00, 0x00, 0x00, 0x13, // len
        0x6d, 0xfe, 0xa9, 0x9a, // correlation_id = 0x6d 0xfe 0xa9 0x9a
        // 0x00, // _tagged_fields
        0x00, 0x00, // error code = no error
        0x02, // len api_keys (compact array N + 1)
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
