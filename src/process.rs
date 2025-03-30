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
