use crate::messages::{requests, responses, ErrorCode};

pub fn process(msg: &[u8], msg_out: &mut [u8]) -> anyhow::Result<usize> {
    let req = requests::V0::try_from(msg)?;

    let res = handle_request(req)?;

    let s = res.write(msg_out)?;
    Ok(s)
}

fn handle_request(req: requests::V0) -> anyhow::Result<responses::V0> {
    if !matches!(req.request_api_version, 0..=3) {
        let res = responses::V0::ApiVersionsRequest {
            correlation_id: req.correlation_id,
            error_code: Some(ErrorCode::UnsupportedVersion),
        };
        return Ok(res);
    }

    let res = responses::V0::ApiVersionsRequest {
        correlation_id: req.correlation_id,
        error_code: None,
    };

    Ok(res)
}
