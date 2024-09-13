use tokio::io::AsyncWriteExt;

use crate::messages::{requests, responses};

pub async fn handle_client(mut stream: tokio::net::TcpStream) -> anyhow::Result<()> {
    let mut msg = vec![0; 1024];
    let mut msg_out = vec![0; 1024];

    // TODO: put this into a loop
    read_stream(&stream, &mut msg).await?;
    let req = requests::V0::try_from(&msg[..])?;
    let res = responses::V0 {
        correlation_id: req.correlation_id,
    };

    let s = res.write(&mut msg_out)?;
    stream.write_all(&msg_out[..s]).await?;

    todo!()
}

async fn read_stream(stream: &tokio::net::TcpStream, msg: &mut Vec<u8>) -> anyhow::Result<()> {
    // reset to base lenght
    msg.resize(1024, 0);
    msg.fill(0);

    let mut len = 0;
    loop {
        // Wait for the socket to be readable
        stream.readable().await?;

        // Try to read data, this may still fail with `WouldBlock`
        // if the readiness event is a false positive.
        match stream.try_read(&mut msg[len..]) {
            Ok(n) => {
                len += n;
                // second round should always be in here
                if len <= msg.len() {
                    msg.truncate(len);
                    break Ok(());
                }

                let size = i32::from_be_bytes(msg[..4].try_into().unwrap());
                let size: usize = size.try_into().expect("able to convert to usize");

                assert!(msg.len() < size);
                msg.resize(size, 0);
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                continue;
            }
            Err(e) => {
                return Err(e.into());
            }
        }
    }
}
