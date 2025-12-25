//! Simple example showing how to use the P2P client helper with Kademlia DHT
//! 
//! This example demonstrates:
//! 1. Bootstrapping to the Kademlia DHT network
//! 2. Discovering and connecting to a peer
//! 3. Sending a JSON message and waiting for a response

mod client_helper;
use client_helper::P2PClient;
use serde_json::json;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Simple P2P Client Example ===\n");

    // Step 1: Bootstrap to DHT network
    // Replace with your actual bootstrap node address
    let bootstrap_nodes = &["/ip4/127.0.0.1/tcp/51820"];
    let namespace = "simple-chat";
    
    println!("[1] Bootstrapping to DHT network...");
    println!("    Bootstrap: {:?}", bootstrap_nodes);
    println!("    Namespace: {}", namespace);
    
    let mut client = P2PClient::new(bootstrap_nodes, namespace).await?;
    println!("    ✓ Bootstrapped! My Peer ID: {}\n", client.local_peer_id());

    // Step 2: Discover and connect to a peer
    println!("[2] Discovering peers in namespace: {}", namespace);
    println!("    (This will block until a peer is found...)");
    let peer_id = client.connect_to_peer().await?;
    println!("    ✓ Connected to peer: {}\n", peer_id);

    // Step 3: Send a JSON message and wait for response
    println!("[3] Sending JSON message...");
    let request = json!({
        "from": "my-client",
        "message": "Hello, peer! This is a test message from the example client.",
        "timestamp": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    });
    
    println!("    Request: {}", serde_json::to_string_pretty(&request)?);
    
    let response = client.send_and_wait(peer_id, request).await?;
    println!("    ✓ Received response!");
    println!("    Response: {}", serde_json::to_string_pretty(&response)?);

    println!("\n=== Example Complete ===");
    Ok(())
}
