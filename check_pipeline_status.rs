//! Quick pipeline status checker
//! Checks if shards are online by querying the web server

use futures_util::{SinkExt, StreamExt};
use serde_json::json;
use std::time::Duration;
use tokio_tungstenite::{connect_async, tungstenite::Message};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Try to connect to WebSocket
    let url = match url::Url::parse("ws://localhost:8081") {
        Ok(u) => u,
        Err(_) => {
            eprintln!("ERROR: Web server not responding");
            std::process::exit(1);
        }
    };

    let (ws_stream, _) = match connect_async(url).await {
        Ok(stream) => stream,
        Err(_) => {
            eprintln!("ERROR: Could not connect to WebSocket");
            std::process::exit(1);
        }
    };

    let (mut write, mut read) = ws_stream.split();

    // Send a test query
    let query_request = json!({
        "query": "test",
        "request_id": "status-check"
    });

    write.send(Message::Text(query_request.to_string())).await?;

    // Wait for response with short timeout
    let timeout = Duration::from_secs(5);
    let response = tokio::time::timeout(timeout, read.next()).await;

    match response {
        Ok(Some(Ok(Message::Text(_)))) => {
            println!("OK: System responding");
            std::process::exit(0);
        }
        Ok(Some(Ok(Message::Close(_)))) => {
            eprintln!("ERROR: Connection closed");
            std::process::exit(1);
        }
        Err(_) => {
            eprintln!("ERROR: Timeout - shards may not be online");
            std::process::exit(1);
        }
        _ => {
            eprintln!("ERROR: Unexpected response");
            std::process::exit(1);
        }
    }
}
