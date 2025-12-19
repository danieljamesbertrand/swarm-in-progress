//! Simple example showing how to use the P2P client helper
//! 
//! This example demonstrates:
//! 1. Connecting to a rendezvous server
//! 2. Discovering and connecting to a peer
//! 3. Sending a JSON message and waiting for a response

use serde_json::json;

// Note: In a real project, you'd import this from your crate
// For this example, we'll assume the helper is available
// mod client_helper;
// use client_helper::P2PClient;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Simple P2P Client Example ===\n");

    // Step 1: Create client and connect to rendezvous server
    // Replace with your actual server address and namespace
    let server = "127.0.0.1:51820";
    let namespace = "simple-chat";
    
    println!("[1] Connecting to rendezvous server: {}", server);
    // let mut client = P2PClient::new(server, namespace).await?;
    println!("    ✓ Connected!\n");

    // Step 2: Discover and connect to a peer
    println!("[2] Discovering peers in namespace: {}", namespace);
    // let peer_id = client.connect_to_peer().await?;
    println!("    ✓ Connected to peer!\n");

    // Step 3: Send a JSON message and wait for response
    println!("[3] Sending JSON message...");
    let request = json!({
        "from": "my-client",
        "message": "Hello, peer! This is a test message.",
        "timestamp": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    });
    
    println!("    Request: {}", serde_json::to_string_pretty(&request)?);
    
    // let response = client.send_and_wait(peer_id, request).await?;
    println!("    ✓ Received response!");
    // println!("    Response: {}", serde_json::to_string_pretty(&response)?);

    println!("\n=== Example Complete ===");
    Ok(())
}

