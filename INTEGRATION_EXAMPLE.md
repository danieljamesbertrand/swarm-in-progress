# Integration Example: P2P JSON Messaging

This document shows how to integrate P2P JSON messaging into your program.

## Quick Start Code Fragment

Here's a minimal code fragment you can include in your program:

```rust
use serde_json::json;
use std::time::Duration;

// Assuming you have access to the P2PClient helper
// mod client_helper;
// use client_helper::P2PClient;

async fn send_p2p_message(
    server: &str,
    namespace: &str,
    message_text: &str,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    // Step 1: Create client and connect to rendezvous server
    let mut client = P2PClient::new(server, namespace).await?;
    
    // Step 2: Discover and connect to a peer (blocks until peer found)
    let peer_id = client.connect_to_peer().await?;
    
    // Step 3: Create your JSON message
    let request = json!({
        "from": "my-app",
        "message": message_text,
        "timestamp": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    });
    
    // Step 4: Send and wait for response (10 second timeout)
    let response = client.send_and_wait(peer_id, request).await?;
    
    Ok(response)
}
```

## Complete Example

```rust
use serde_json::json;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Configuration
    let rendezvous_server = "127.0.0.1:51820";
    let namespace = "my-app-namespace";
    
    // Create client
    let mut client = P2PClient::new(rendezvous_server, namespace).await?;
    println!("Connected to rendezvous server");
    println!("My Peer ID: {}", client.local_peer_id());
    
    // Connect to a peer
    println!("Discovering peers...");
    let peer_id = client.connect_to_peer().await?;
    println!("Connected to peer: {}", peer_id);
    
    // Send a message
    let message = json!({
        "from": "my-client",
        "message": "Hello from my application!",
        "timestamp": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    });
    
    println!("Sending: {}", serde_json::to_string_pretty(&message)?);
    let response = client.send_and_wait(peer_id, message).await?;
    println!("Received: {}", serde_json::to_string_pretty(&response)?);
    
    Ok(())
}
```

## API Reference

### `P2PClient::new(server: &str, namespace: &str) -> Result<Self, Box<dyn Error>>`

Creates a new P2P client and connects to the rendezvous server.

**Parameters:**
- `server`: Rendezvous server address (e.g., `"127.0.0.1:51820"`)
- `namespace`: Namespace for peer discovery (e.g., `"my-app"`)

**Returns:** A connected `P2PClient` instance

### `connect_to_peer(&mut self) -> Result<PeerId, Box<dyn Error>>`

Discovers peers in the namespace and connects to the first available peer.

**Returns:** The `PeerId` of the connected peer

**Note:** This method blocks until a peer is found and connected.

### `send_and_wait(&mut self, peer_id: PeerId, json_message: serde_json::Value) -> Result<serde_json::Value, Box<dyn Error>>`

Sends a JSON message to a peer and waits for a response.

**Parameters:**
- `peer_id`: The peer to send the message to
- `json_message`: A `serde_json::Value` with the following structure:
  ```json
  {
    "from": "sender-name",
    "message": "message text",
    "timestamp": 1234567890
  }
  ```

**Returns:** The response as a `serde_json::Value` with the same structure

**Timeout:** 10 seconds (configurable in the implementation)

### `local_peer_id(&self) -> PeerId`

Returns the local peer ID.

### `connected_peers(&self) -> Vec<PeerId>`

Returns a list of all currently connected peers.

## Dependencies

Add these to your `Cargo.toml`:

```toml
[dependencies]
libp2p = { version = "0.53", features = ["quic", "noise", "tcp", "dns", "macros", "rendezvous", "identify", "yamux", "tokio", "request-response"] }
tokio = { version = "1.35", features = ["full"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
async-trait = "0.1"
```

## Error Handling

The client handles:
- Connection timeouts
- Network errors
- Missing peers
- Invalid JSON messages

All methods return `Result` types that should be handled appropriately.

## Notes

- The client automatically responds to incoming requests with an echo response
- You can customize the auto-response behavior by modifying the `send_and_wait` method
- The client maintains connections to all discovered peers
- Messages are sent over encrypted libp2p connections using Noise protocol

