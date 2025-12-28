# Integration Guide: P2P JSON Messaging with Kademlia

This guide shows how to integrate P2P JSON messaging into your Rust application using the Kademlia DHT-based peer discovery system.

## Overview

The `P2PClient` helper provides a simple, high-level API for:
1. Bootstrapping to the Kademlia DHT network
2. Discovering peers in a namespace
3. Sending JSON messages and receiving responses

## Quick Start

### Step 1: Add Dependencies

Add to your `Cargo.toml`:

```toml
[dependencies]
libp2p = { version = "0.53", features = ["quic", "noise", "tcp", "dns", "macros", "kad", "identify", "yamux", "tokio", "request-response"] }
tokio = { version = "1.35", features = ["full"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
async-trait = "0.1"
sha2 = "0.10"
```

### Step 2: Include the Client Helper

Copy `src/client_helper.rs` and `src/message.rs` into your project, or add this project as a dependency.

### Step 3: Basic Usage

```rust
use client_helper::P2PClient;
use serde_json::json;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Bootstrap to DHT network
    let mut client = P2PClient::new(
        &["/ip4/127.0.0.1/tcp/51820"],  // Bootstrap nodes
        "my-app-namespace"              // Namespace
    ).await?;
    
    println!("My Peer ID: {}", client.local_peer_id());
    
    // Discover and connect to a peer
    println!("Discovering peers...");
    let peer_id = client.connect_to_peer().await?;
    println!("Connected to peer: {}", peer_id);
    
    // Send a JSON message
    let request = json!({
        "from": "my-app",
        "message": "Hello from my application!",
        "timestamp": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs()
    });
    
    println!("Sending: {}", serde_json::to_string_pretty(&request)?);
    let response = client.send_and_wait(peer_id, request).await?;
    println!("Received: {}", serde_json::to_string_pretty(&response)?);
    
    Ok(())
}
```

## API Reference

### `P2PClient::new(bootstrap_nodes: &[&str], namespace: &str) -> Result<Self, Box<dyn Error>>`

Creates a new P2P client and bootstraps to the Kademlia DHT network.

**Parameters:**
- `bootstrap_nodes`: Array of bootstrap node addresses in Multiaddr format
  - Example: `&["/ip4/127.0.0.1/tcp/51820"]`
  - Example: `&["/ip4/192.168.1.100/tcp/8080", "/ip4/192.168.1.101/tcp/8080"]`
- `namespace`: Namespace string for peer discovery
  - Peers must use the **same namespace** to discover each other
  - Examples: `"my-app"`, `"chat-room-1"`, `"game-lobby"`

**Returns:** A bootstrapped `P2PClient` instance

**Errors:**
- Network errors (bootstrap nodes unreachable)
- Invalid bootstrap node address format
- Transport setup errors

**Example:**
```rust
// Single bootstrap node
let mut client = P2PClient::new(
    &["/ip4/127.0.0.1/tcp/51820"],
    "my-namespace"
).await?;

// Multiple bootstrap nodes (for redundancy)
let mut client = P2PClient::new(
    &[
        "/ip4/192.168.1.100/tcp/51820",
        "/ip4/192.168.1.101/tcp/51820"
    ],
    "my-namespace"
).await?;
```

### `connect_to_peer(&mut self) -> Result<PeerId, Box<dyn Error>>`

Discovers peers in the namespace via DHT and connects to the first available peer.

**Returns:** The `PeerId` of the connected peer

**Note:** This method **blocks** until a peer is found and connected. If no peers are available, it will wait indefinitely.

**Example:**
```rust
// This blocks until a peer is found
let peer_id = client.connect_to_peer().await?;
println!("Connected to peer: {}", peer_id);
```

### `send_and_wait(&mut self, peer_id: PeerId, json_message: serde_json::Value) -> Result<serde_json::Value, Box<dyn Error>>`

Sends a JSON message to a peer and waits for a response.

**Parameters:**
- `peer_id`: The peer to send the message to (from `connect_to_peer()`)
- `json_message`: A `serde_json::Value` with the following structure:
  ```json
  {
    "from": "sender-name",      // Required: String
    "message": "message text",   // Required: String
    "timestamp": 1234567890      // Optional: Number (u64)
  }
  ```

**Returns:** The response as a `serde_json::Value` with the same structure

**Timeout:** 10 seconds (configurable in the implementation)

**Example:**
```rust
let request = json!({
    "from": "my-client",
    "message": "Hello, peer!",
    "timestamp": std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs()
});

match client.send_and_wait(peer_id, request).await {
    Ok(response) => {
        println!("Response from {}: {}", 
            response["from"].as_str().unwrap(),
            response["message"].as_str().unwrap()
        );
    }
    Err(e) => {
        eprintln!("Error: {}", e);
    }
}
```

### `local_peer_id(&self) -> PeerId`

Returns your local peer ID.

**Example:**
```rust
let my_id = client.local_peer_id();
println!("My Peer ID: {}", my_id);
```

### `connected_peers(&self) -> Vec<PeerId>`

Returns a list of all currently connected peers.

**Example:**
```rust
let peers = client.connected_peers();
println!("Connected to {} peer(s)", peers.len());
for peer in peers {
    println!("  - {}", peer);
}
```

## Complete Examples

### Example 1: Simple Message Exchange

```rust
use client_helper::P2PClient;
use serde_json::json;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Configuration
    let bootstrap_nodes = &["/ip4/127.0.0.1/tcp/51820"];
    let namespace = "my-app-namespace";
    
    // Create client and bootstrap
    let mut client = P2PClient::new(bootstrap_nodes, namespace).await?;
    println!("Connected to DHT network");
    println!("My Peer ID: {}", client.local_peer_id());
    
    // Discover and connect to a peer
    println!("Discovering peers...");
    let peer_id = client.connect_to_peer().await?;
    println!("Connected to peer: {}", peer_id);
    
    // Send a message
    let message = json!({
        "from": "my-client",
        "message": "Hello from my application!",
        "timestamp": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs()
    });
    
    println!("Sending: {}", serde_json::to_string_pretty(&message)?);
    let response = client.send_and_wait(peer_id, message).await?;
    println!("Received: {}", serde_json::to_string_pretty(&response)?);
    
    Ok(())
}
```

### Example 2: Multiple Messages

```rust
use client_helper::P2PClient;
use serde_json::json;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut client = P2PClient::new(
        &["/ip4/127.0.0.1/tcp/51820"],
        "chat-room"
    ).await?;
    
    let peer_id = client.connect_to_peer().await?;
    
    // Send multiple messages
    for i in 1..=5 {
        let msg = json!({
            "from": "sender",
            "message": format!("Message #{}", i),
            "timestamp": std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_secs()
        });
        
        let response = client.send_and_wait(peer_id, msg).await?;
        println!("Response #{}: {}", i, response["message"]);
    }
    
    Ok(())
}
```

### Example 3: Error Handling

```rust
use client_helper::P2PClient;
use serde_json::json;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Bootstrap with error handling
    let mut client = match P2PClient::new(
        &["/ip4/127.0.0.1/tcp/51820"],
        "my-namespace"
    ).await {
        Ok(client) => {
            println!("Successfully bootstrapped to DHT");
            client
        }
        Err(e) => {
            eprintln!("Failed to bootstrap: {}", e);
            eprintln!("Make sure bootstrap node is running!");
            return Err(e);
        }
    };
    
    // Connect to peer with timeout handling
    let peer_id = match tokio::time::timeout(
        std::time::Duration::from_secs(30),
        client.connect_to_peer()
    ).await {
        Ok(Ok(peer_id)) => {
            println!("Connected to peer: {}", peer_id);
            peer_id
        }
        Ok(Err(e)) => {
            eprintln!("Connection error: {}", e);
            return Err(e);
        }
        Err(_) => {
            eprintln!("Timeout: No peer found within 30 seconds");
            return Err("Timeout".into());
        }
    };
    
    // Send message with error handling
    let request = json!({
        "from": "my-app",
        "message": "Hello!",
        "timestamp": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs()
    });
    
    match client.send_and_wait(peer_id, request).await {
        Ok(response) => {
            println!("Success! Response: {}", response["message"]);
        }
        Err(e) => {
            eprintln!("Failed to send message: {}", e);
            // Handle error (retry, log, etc.)
        }
    }
    
    Ok(())
}
```

## Message Format

### Request Format

All messages must follow this JSON structure:

```json
{
  "from": "sender-identifier",    // Required: String - Your app/peer name
  "message": "message content",    // Required: String - Your message text
  "timestamp": 1234567890          // Optional: Number - Unix timestamp (u64)
}
```

### Response Format

Responses use the same structure:

```json
{
  "from": "responder-name",
  "message": "response text",
  "timestamp": 1234567890
}
```

### Custom Message Fields

You can add additional fields, but `from` and `message` are required:

```json
{
  "from": "my-app",
  "message": "Hello!",
  "timestamp": 1234567890,
  "custom_field": "custom_value",  // Optional: Any JSON value
  "metadata": {                     // Optional: Nested objects
    "version": "1.0",
    "type": "greeting"
  }
}
```

## Bootstrap Node Configuration

### Local Development

For local testing, use localhost:

```rust
let mut client = P2PClient::new(
    &["/ip4/127.0.0.1/tcp/51820"],
    "dev-namespace"
).await?;
```

### Production

For production, use your bootstrap node's public IP:

```rust
let mut client = P2PClient::new(
    &["/ip4/203.0.113.1/tcp/51820"],  // Public IP
    "production-namespace"
).await?;
```

### Multiple Bootstrap Nodes

You can specify multiple bootstrap nodes for redundancy:

```rust
let mut client = P2PClient::new(
    &[
        "/ip4/192.168.1.100/tcp/51820",
        "/ip4/192.168.1.101/tcp/51820",
        "/ip4/192.168.1.102/tcp/51820"
    ],
    "my-namespace"
).await?;
```

## Namespace Best Practices

Namespaces act like "rooms" or "channels" - only peers in the same namespace can discover each other.

### Good Namespace Names

- `"my-app-v1"` - Application-specific
- `"chat-room-123"` - Room/channel-based
- `"game-lobby-abc"` - Session-based
- `"user-alice-chat"` - User-specific

### Namespace Guidelines

1. **Use descriptive names**: Make it clear what the namespace is for
2. **Include version**: `"my-app-v2"` helps with migrations
3. **Keep it short**: Long names waste DHT storage
4. **Be consistent**: All peers must use the exact same string

## Error Handling

The client handles various error conditions:

### Bootstrap Errors

```rust
match P2PClient::new(&["/ip4/127.0.0.1/tcp/51820"], "ns").await {
    Ok(client) => { /* Success */ }
    Err(e) => {
        // Possible causes:
        // - Bootstrap node not running
        // - Network unreachable
        // - Invalid address format
    }
}
```

### Peer Discovery Errors

```rust
match client.connect_to_peer().await {
    Ok(peer_id) => { /* Success */ }
    Err(e) => {
        // Possible causes:
        // - No peers in namespace
        // - DHT not fully bootstrapped
        // - Network issues
    }
}
```

### Message Send Errors

```rust
match client.send_and_wait(peer_id, message).await {
    Ok(response) => { /* Success */ }
    Err(e) => {
        // Possible causes:
        // - Peer disconnected
        // - Timeout (10 seconds)
        // - Invalid message format
        // - Network error
    }
}
```

## Advanced Usage

### Checking Connection Status

```rust
let peers = client.connected_peers();
if peers.is_empty() {
    println!("No peers connected");
} else {
    println!("Connected to {} peer(s)", peers.len());
}
```

### Sending to Multiple Peers

```rust
let peer_ids = client.connected_peers();
for peer_id in peer_ids {
    let msg = json!({
        "from": "broadcaster",
        "message": "Hello!",
        "timestamp": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs()
    });
    
    if let Ok(response) = client.send_and_wait(peer_id, msg).await {
        println!("Response from {}: {}", peer_id, response["message"]);
    }
}
```

## Troubleshooting

### "DHT not bootstrapped yet"

- Wait a few seconds after creating the client
- Check that bootstrap node is accessible
- Verify network connectivity

### "No peers found"

- Ensure another peer is running in the same namespace
- Wait 10-30 seconds for DHT to populate
- Check that both peers bootstrapped to the same network

### Connection Timeouts

- Verify peer is still online
- Check network connectivity
- Increase timeout in code if needed

## Notes

- The client automatically responds to incoming requests with an echo response
- You can customize the auto-response behavior by modifying the client helper code
- The client maintains connections to all discovered peers
- Messages are sent over encrypted libp2p connections using Noise protocol
- The DHT is decentralized - no central server needed after bootstrap
