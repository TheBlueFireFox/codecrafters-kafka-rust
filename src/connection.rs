use tokio::io::AsyncWriteExt;

use crate::meta::{self, Meta};

#[derive(Debug, thiserror::Error)]
enum Error {
    #[error("IO Error <{0}>")]
    IO(#[from] std::io::Error),
    #[error("Closed Connection")]
    ConnectionClosed,
}

pub async fn handle_client(mut stream: tokio::net::TcpStream) -> anyhow::Result<()> {
    let mut msg = Vec::with_capacity(1024);
    let mut msg_out = Vec::with_capacity(1024);
    let meta = Meta::from_cluster_metadata(meta::PATH)?;

    loop {
        msg.clear();
        msg_out.clear();
        match read_stream(&stream, &mut msg).await {
            Ok(_) => {
                let s = super::process::process(&msg, &mut msg_out, &meta)?;
                stream.write_all(&msg_out[..s]).await?;
            }
            Err(Error::ConnectionClosed) => break Ok(()),
            Err(err) => {
                break Err(err.into());
            }
        }
    }
}

async fn read_stream(stream: &tokio::net::TcpStream, msg: &mut Vec<u8>) -> Result<(), Error> {
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
            Ok(0) => {
                return Err(Error::ConnectionClosed);
            }
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
