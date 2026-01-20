//! AI Query Client Example
//!
//! This example demonstrates how to send AI inference queries to the
//! Promethos-AI Swarm web server via WebSocket.
//!
//! Usage:
//!   cargo run --example ai_query_client -- "What is artificial intelligence?"
//!
//! Or run with default query:
//!   cargo run --example ai_query_client

use futures_util::{SinkExt, StreamExt};
use serde_json::json;
use std::env;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio_tungstenite::{connect_async, tungstenite::Message};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Get query from command line or use default
    let query = env::args()
        .nth(1)
        .unwrap_or_else(|| "What is artificial intelligence?".to_string());

    println!("\n╔══════════════════════════════════════════════════════════════╗");
    println!("║          🔥 PROMETHOS-AI QUERY CLIENT 🔥                     ║");
    println!("╠══════════════════════════════════════════════════════════════╣");
    println!("║  WebSocket: ws://localhost:8081                             ║");
    println!("╚══════════════════════════════════════════════════════════════╝\n");

    println!("Query: \"{}\"", query);
    println!();

    // Connect to WebSocket server
    println!("[1/4] Connecting to WebSocket server...");
    let url = url::Url::parse("ws://localhost:8081")?;
    let (ws_stream, _) = connect_async(url).await?;
    println!("  ✓ Connected to WebSocket");

    let (mut write, mut read) = ws_stream.split();

    // Create query request with timestamp-based request ID
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let request_id = format!("query-{}", timestamp);
    let query_request = json!({
        "query": query,
        "request_id": request_id
    });

    println!("\n[2/4] Sending inference request...");
    println!("  Request ID: {}", request_id);
    println!("  Request: {}", serde_json::to_string_pretty(&query_request)?);

    // Send query
    write
        .send(Message::Text(query_request.to_string()))
        .await?;
    println!("  ✓ Request sent");

    println!("\n[3/4] Waiting for response (120 second timeout)...");
    println!("  This may take 30-90 seconds for distributed inference...");

    // Receive response with timeout
    let timeout = tokio::time::Duration::from_secs(120);
    let response = tokio::time::timeout(timeout, read.next()).await;

    match response {
        Ok(Some(Ok(Message::Text(text)))) => {
            println!("\n[4/4] Response received!");
            println!("\n{}", "═".repeat(70));
            println!("AI RESPONSE:");
            println!("{}", "═".repeat(70));

            // Try to parse JSON response
            match serde_json::from_str::<serde_json::Value>(&text) {
                Ok(json) => {
                    if let Some(response_text) = json.get("response").and_then(|v| v.as_str()) {
                        println!("{}", response_text);
                    } else if let Some(error) = json.get("error").and_then(|v| v.as_str()) {
                        println!("Error: {}", error);
                    } else {
                        println!("{}", serde_json::to_string_pretty(&json)?);
                    }
                }
                Err(_) => {
                    // Not JSON, print as-is
                    println!("{}", text);
                }
            }

            println!("{}", "═".repeat(70));
            println!("\n✓ Query completed successfully!");
        }
        Ok(Some(Ok(Message::Close(_)))) => {
            println!("  ⚠ Connection closed by server");
        }
        Ok(Some(Ok(Message::Binary(_)))) => {
            println!("  ⚠ Received binary message (unexpected)");
        }
        Ok(Some(Ok(Message::Ping(_)))) => {
            println!("  ℹ Received ping (handled automatically)");
        }
        Ok(Some(Ok(Message::Pong(_)))) => {
            println!("  ℹ Received pong (handled automatically)");
        }
        Ok(Some(Ok(Message::Frame(_)))) => {
            println!("  ⚠ Received raw frame (unexpected)");
        }
        Ok(Some(Err(e))) => {
            eprintln!("  ✗ WebSocket error: {}", e);
            return Err(e.into());
        }
        Ok(None) => {
            println!("  ⚠ Connection closed (no message)");
        }
        Err(_) => {
            eprintln!("  ✗ Timeout waiting for response (120 seconds)");
            return Err("Timeout waiting for response".into());
        }
    }

    // Close connection
    write.close().await?;
    println!("\nConnection closed.");

    Ok(())
}
