//! Minimal Copy-Paste Example: P2P JSON Messaging
//! 
//! This is a complete, working example you can copy directly into your program.
//! Every parameter is documented with exactly what to fill in.

use client_helper::P2PClient;
use serde_json::json;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // ========================================================================
    // STEP 1: CONNECT TO RENDEZVOUS SERVER
    // ========================================================================
    
    // PARAMETER 1: Server address
    // Format: "host:port" or "host" (port defaults to 51820)
    // Examples:
    //   "127.0.0.1:51820"     ← Local server
    //   "192.168.1.100:8080"  ← Remote server, custom port
    //   "example.com"          ← Domain name (uses port 51820)
    let server = "127.0.0.1:51820";
    
    // PARAMETER 2: Namespace
    // This is like a "room name" - peers must use the SAME namespace to find each other
    // Examples:
    //   "my-app"              ← Simple namespace
    //   "chat-room-1"         ← Descriptive namespace
    //   "game-lobby-abc123"   ← Unique namespace
    let namespace = "my-namespace";
    
    // Create client - this CONNECTS to the rendezvous server
    // This will BLOCK until connection is established
    // Returns: P2PClient instance, or error if connection fails
    let mut client = P2PClient::new(server, namespace).await?;
    
    println!("✓ Connected to rendezvous server");
    println!("  My Peer ID: {}", client.local_peer_id());
    
    // ========================================================================
    // STEP 2: CONNECT TO A PEER
    // ========================================================================
    
    // This will BLOCK until a peer is found and connected
    // Make sure another peer is running and registered in the same namespace!
    // Returns: PeerId of the connected peer, or error if connection fails
    let peer_id = client.connect_to_peer().await?;
    
    println!("✓ Connected to peer: {}", peer_id);
    
    // ========================================================================
    // STEP 3: SEND MESSAGE AND WAIT FOR RESPONSE
    // ========================================================================
    
    // PARAMETER 3: Create your JSON message
    // REQUIRED FIELDS:
    //   - "from": String    ← Your identifier/name
    //   - "message": String ← Your message text
    // OPTIONAL FIELDS:
    //   - "timestamp": u64  ← Unix timestamp (auto-set if missing)
    let request = json!({
        "from": "my-client",                    // ← REQUIRED: Your name/ID
        "message": "Hello from my application!", // ← REQUIRED: Your message
        "timestamp": std::time::SystemTime::now() // ← OPTIONAL: Current time
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    });
    
    println!("Sending: {}", serde_json::to_string_pretty(&request)?);
    
    // PARAMETER 4: Send to peer and wait for response
    // Parameters:
    //   - peer_id: Use the PeerId from connect_to_peer()
    //   - request: Your JSON message
    // 
    // HOW TO WAIT FOR RESPONSE:
    //   - Just use .await - the function automatically waits!
    //   - It blocks until response is received OR 10 second timeout
    //   - No polling or manual checking needed
    // 
    // Returns: Response JSON, or error if timeout/connection fails
    let response = client.send_and_wait(peer_id, request).await?;
    
    // ========================================================================
    // STEP 4: USE THE RESPONSE
    // ========================================================================
    
    // Response is now available! The function already waited for it.
    // Response format:
    //   {
    //     "from": "peer-name",
    //     "message": "response text",
    //     "timestamp": 1234567890
    //   }
    
    println!("Response received: {}", serde_json::to_string_pretty(&response)?);
    
    // Access individual fields:
    let from = response["from"].as_str().unwrap();
    let message = response["message"].as_str().unwrap();
    let timestamp = response["timestamp"].as_u64().unwrap();
    
    println!("From: {}", from);
    println!("Message: {}", message);
    println!("Timestamp: {}", timestamp);
    
    Ok(())
}

// ============================================================================
// ERROR HANDLING EXAMPLE
// ============================================================================

#[allow(dead_code)]
async fn example_with_error_handling() -> Result<(), Box<dyn std::error::Error>> {
    use std::time::Duration;
    
    // Step 1: Connect with error handling
    let mut client = match P2PClient::new("127.0.0.1:51820", "my-namespace").await {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to connect: {}", e);
            eprintln!("Make sure the rendezvous server is running!");
            return Err(e);
        }
    };
    
    // Step 2: Connect to peer with timeout
    let peer_id = match tokio::time::timeout(
        Duration::from_secs(30),  // Wait up to 30 seconds for a peer
        client.connect_to_peer()
    ).await {
        Ok(Ok(pid)) => pid,
        Ok(Err(e)) => {
            eprintln!("Connection error: {}", e);
            return Err(e);
        }
        Err(_) => {
            eprintln!("Timeout: No peers found in 30 seconds");
            return Err("No peers found".into());
        }
    };
    
    // Step 3: Send message with error handling
    let request = json!({
        "from": "my-client",
        "message": "Hello!"
    });
    
    match client.send_and_wait(peer_id, request).await {
        Ok(response) => {
            println!("Got response: {}", response["message"]);
        }
        Err(e) => {
            if e.to_string().contains("Timeout") {
                eprintln!("Peer didn't respond within 10 seconds");
            } else {
                eprintln!("Error: {}", e);
            }
        }
    }
    
    Ok(())
}
















