use tokio::select;

async fn accept_loop() -> anyhow::Result<()> {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:9092").await?;
    loop {
        let (stream, addr) = listener.accept().await?;
        todo!();
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // You can use print statements as follows for debugging, they'll be visible when running tests.
    println!("Logs from your program will appear here!");

    let signal = tokio::signal::ctrl_c();

    select! {
        _ = accept_loop() => {
            // will do nothing
        }
        _ = signal => {
            // will break the application
        }
    };

    Ok(())
}
