//! Usage: `ta-server [ADDRESS]` (default `0.0.0.0:7878`).

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let address = std::env::args()
        .nth(1)
        .unwrap_or_else(|| format!("0.0.0.0:{}", ta_net::DEFAULT_PORT));
    let listener = tokio::net::TcpListener::bind(&address).await?;
    println!("ta-server listening on ws://{}", listener.local_addr()?);
    ta_server::serve(listener).await
}
