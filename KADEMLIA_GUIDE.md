# Comprehensive Kademlia P2P Guide

A complete guide to using the Kademlia-based P2P networking system.

## Table of Contents

1. [Introduction](#introduction)
2. [Architecture Overview](#architecture-overview)
3. [Getting Started](#getting-started)
4. [Connection Examples](#connection-examples)
5. [Usage Patterns](#usage-patterns)
6. [Troubleshooting](#troubleshooting)
7. [Advanced Topics](#advanced-topics)

## Introduction

This project implements a peer-to-peer networking system using **Kademlia DHT** (Distributed Hash Table) for decentralized peer discovery. Unlike centralized systems, Kademlia allows peers to discover each other without relying on a central server.

### Key Concepts

- **DHT (Distributed Hash Table)**: A decentralized key-value store distributed across all peers
- **Bootstrap Node**: Initial entry point to join the network
- **Namespace**: Logical grouping of peers (like "rooms" or "channels")
- **Peer ID**: Unique identifier for each peer in the network

## Architecture Overview

### How Kademlia Works

```
┌─────────────┐
│  Bootstrap   │
│    Node      │
└──────┬───────┘
       │
       ├─────────────────┐
       │                 │
┌──────▼──────┐    ┌─────▼──────┐
│   Peer A    │◄──►│   Peer B   │
│ (Listener)  │    │  (Dialer)  │
└─────────────┘    └────────────┘
       │                 │
       └────────┬────────┘
                │
         ┌──────▼──────┐
         │  Kademlia   │
         │     DHT     │
         └────────────┘
```

1. **Bootstrap**: Peers connect to bootstrap nodes to join the network
2. **Discovery**: Peers query the DHT to find other peers
3. **Connection**: Direct P2P connections are established
4. **Messaging**: JSON messages exchanged over encrypted connections

### Components

1. **Bootstrap Node (`server`)**: Helps peers join the network
2. **Listener (`listener`)**: Joins network and waits for connections
3. **Dialer (`dialer`)**: Discovers and connects to peers
4. **Client Helper (`P2PClient`)**: High-level API for applications

## Getting Started

### Prerequisites

- Rust 1.70+ installed
- Network connectivity
- Port 51820 (or custom port) accessible

### Installation

```bash
# Clone or download the project
cd punch-simple

# Build all binaries
cargo build --release
```

### Quick Test

**Terminal 1 - Start Bootstrap:**
```bash
cargo run --release --bin server
```

**Terminal 2 - Start Listener:**
```bash
cargo run --release --bin listener -- --namespace test
```

**Terminal 3 - Start Dialer:**
```bash
cargo run --release --bin dialer -- --namespace test
```

You should see the dialer discover and connect to the listener!

## Connection Examples

### Example 1: Local Development Setup

**Step 1: Start Bootstrap Node**
```bash
cargo run --release --bin server -- --listen-addr 0.0.0.0 --port 51820
```

**Step 2: Start Listener (Terminal 2)**
```bash
cargo run --release --bin listener \
  --bootstrap /ip4/127.0.0.1/tcp/51820 \
  --namespace dev-room
```

**Step 3: Start Dialer (Terminal 3)**
```bash
cargo run --release --bin dialer \
  --bootstrap /ip4/127.0.0.1/tcp/51820 \
  --namespace dev-room
```

**Expected Output:**

Listener:
```
=== Simple Kademlia Listener ===
Peer ID: 12D3KooW...
✓ DHT bootstrapped!
✓ Registered in DHT! Waiting for connections...
Your Peer ID: 12D3KooW...
```

Dialer:
```
=== Simple Kademlia Dialer ===
Local Peer ID: 12D3KooW...
✓ DHT bootstrapped! Discovering peers...
✓✓✓ CONNECTED to peer: 12D3KooW...
[📤 SENT JSON MESSAGE] to peer 12D3KooW...
```

### Example 2: Remote Peers

**Machine A (192.168.1.100) - Bootstrap + Listener:**

```bash
# Terminal 1: Start bootstrap
cargo run --release --bin server -- --port 51820

# Terminal 2: Start listener
cargo run --release --bin listener \
  --bootstrap /ip4/192.168.1.100/tcp/51820 \
  --namespace shared-room
```

**Machine B (192.168.1.101) - Dialer:**

```bash
cargo run --release --bin dialer \
  --bootstrap /ip4/192.168.1.100/tcp/51820 \
  --namespace shared-room
```

### Example 3: Multiple Namespaces

You can run multiple isolated networks using different namespaces:

**Namespace "chat-room-1":**
```bash
# Terminal 1
cargo run --release --bin listener -- --namespace chat-room-1

# Terminal 2
cargo run --release --bin dialer -- --namespace chat-room-1
```

**Namespace "chat-room-2":**
```bash
# Terminal 3
cargo run --release --bin listener -- --namespace chat-room-2

# Terminal 4
cargo run --release --bin dialer -- --namespace chat-room-2
```

Peers in different namespaces won't see each other.

### Example 4: Programmatic Usage

```rust
use client_helper::P2PClient;
use serde_json::json;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Bootstrap to network
    let mut client = P2PClient::new(
        &["/ip4/127.0.0.1/tcp/51820"],
        "my-app"
    ).await?;
    
    // Discover peer
    let peer_id = client.connect_to_peer().await?;
    
    // Send message
    let response = client.send_and_wait(peer_id, json!({
        "from": "app",
        "message": "Hello!",
        "timestamp": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs()
    })).await?;
    
    println!("Response: {}", response["message"]);
    Ok(())
}
```

## Usage Patterns

### Pattern 1: Client-Server Model

**Server (Listener):**
```bash
cargo run --release --bin listener \
  --namespace my-service \
  --bootstrap /ip4/192.168.1.100/tcp/51820
```

**Client (Dialer):**
```bash
cargo run --release --bin dialer \
  --namespace my-service \
  --bootstrap /ip4/192.168.1.100/tcp/51820
```

### Pattern 2: Peer-to-Peer Chat

Run multiple listeners and dialers in the same namespace:

```bash
# Peer 1
cargo run --release --bin listener -- --namespace chat

# Peer 2
cargo run --release --bin listener -- --namespace chat

# Peer 3
cargo run --release --bin dialer -- --namespace chat
```

### Pattern 3: Integration into Application

```rust
// In your application
mod client_helper;
use client_helper::P2PClient;

struct MyApp {
    p2p_client: P2PClient,
}

impl MyApp {
    async fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let client = P2PClient::new(
            &["/ip4/127.0.0.1/tcp/51820"],
            "my-app-namespace"
        ).await?;
        
        Ok(Self { p2p_client: client })
    }
    
    async fn send_message(&mut self, text: &str) -> Result<(), Box<dyn std::error::Error>> {
        let peer_id = self.p2p_client.connect_to_peer().await?;
        let response = self.p2p_client.send_and_wait(peer_id, json!({
            "from": "my-app",
            "message": text,
            "timestamp": std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_secs()
        })).await?;
        
        println!("Received: {}", response["message"]);
        Ok(())
    }
}
```

## Troubleshooting

### Problem: Peers Can't Find Each Other

**Symptoms:**
- Dialer shows "Discovering peers..." but never connects
- Listener shows "Waiting for connections..." but nothing happens

**Solutions:**

1. **Check Namespace Match**
   ```bash
   # Both must use EXACT same namespace
   listener --namespace my-room
   dialer --namespace my-room  # ✓ Same
   dialer --namespace My-Room  # ✗ Different (case-sensitive)
   ```

2. **Verify Bootstrap Node**
   ```bash
   # Both must bootstrap to same node
   listener --bootstrap /ip4/127.0.0.1/tcp/51820
   dialer --bootstrap /ip4/127.0.0.1/tcp/51820  # ✓ Same
   ```

3. **Wait for DHT Population**
   - Allow 10-30 seconds after bootstrap
   - DHT needs time to discover and store peer information

4. **Check Network Connectivity**
   ```bash
   # Test bootstrap node accessibility
   telnet 127.0.0.1 51820  # Should connect
   ```

### Problem: Connection Timeouts

**Symptoms:**
- "Timeout waiting for response" errors
- Connections established but messages fail

**Solutions:**

1. **Check Peer is Online**
   - Verify listener is still running
   - Check for connection errors in logs

2. **Increase Timeout** (in code)
   ```rust
   // In client_helper.rs, modify timeout duration
   let timeout_duration = Duration::from_secs(30);  // Increase from 10
   ```

3. **Check Firewall**
   - Ensure ports are not blocked
   - Allow both TCP and UDP if needed

### Problem: Bootstrap Node Not Found

**Symptoms:**
- "Failed to dial bootstrap node" errors
- Cannot connect to bootstrap node

**Solutions:**

1. **Verify Bootstrap Node is Running**
   ```bash
   # Check if server is listening
   netstat -an | findstr 51820  # Windows
   netstat -an | grep 51820     # Linux/Mac
   ```

2. **Check Listen Address**
   ```bash
   # Use 0.0.0.0 for remote access
   server --listen-addr 0.0.0.0 --port 51820  # ✓ Correct
   server --listen-addr 127.0.0.1 --port 51820  # ✗ Only localhost
   ```

3. **Verify Address Format**
   ```bash
   # Correct Multiaddr format
   --bootstrap /ip4/192.168.1.100/tcp/51820  # ✓
   --bootstrap 192.168.1.100:51820           # ✗ Wrong format
   ```

### Problem: "DHT not bootstrapped yet"

**Symptoms:**
- Error when calling `connect_to_peer()` immediately after `new()`

**Solution:**

Wait for bootstrap to complete:

```rust
let mut client = P2PClient::new(&["/ip4/127.0.0.1/tcp/51820"], "ns").await?;

// Wait a bit for bootstrap
tokio::time::sleep(Duration::from_secs(2)).await;

// Now safe to discover peers
let peer_id = client.connect_to_peer().await?;
```

## Advanced Topics

### Custom Message Handling

Modify the client helper to customize message handling:

```rust
// In client_helper.rs, modify the request handler
SwarmEvent::Behaviour(BehaviourEvent::RequestResponse(
    request_response::Event::Message { message, .. }
)) => {
    match message {
        request_response::Message::Request { request, channel, .. } => {
            // Custom response logic
            let response = JsonMessage::new(
                "custom-responder".to_string(),
                format!("Processed: {}", request.message),
            );
            self.swarm.behaviour_mut()
                .request_response
                .send_response(channel, response)?;
        }
        // ...
    }
}
```

### Multiple Bootstrap Nodes

For production, use multiple bootstrap nodes:

```rust
let mut client = P2PClient::new(
    &[
        "/ip4/192.168.1.100/tcp/51820",
        "/ip4/192.168.1.101/tcp/51820",
        "/ip4/192.168.1.102/tcp/51820"
    ],
    "production-namespace"
).await?;
```

### Persistent Peer IDs

To maintain the same peer ID across restarts, save and load the keypair:

```rust
use libp2p::identity;

// Save keypair
let key = identity::Keypair::generate_ed25519();
let key_bytes = key.to_protobuf_encoding()?;
std::fs::write("peer.key", key_bytes)?;

// Load keypair
let key_bytes = std::fs::read("peer.key")?;
let key = identity::Keypair::from_protobuf_encoding(&key_bytes)?;
```

### Monitoring DHT State

Add logging to monitor DHT operations:

```rust
// Enable verbose logging
env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("debug")).init();

// Or add custom logging
match event {
    SwarmEvent::Behaviour(BehaviourEvent::Kademlia(kad::Event::RoutingUpdated { .. })) => {
        println!("DHT routing table updated");
    }
    // ...
}
```

## Best Practices

1. **Use Descriptive Namespaces**: `"my-app-v1"` not `"test"`
2. **Handle Errors Gracefully**: Always check `Result` types
3. **Wait for Bootstrap**: Allow time for DHT to populate
4. **Use Multiple Bootstrap Nodes**: For production reliability
5. **Monitor Connections**: Check `connected_peers()` regularly
6. **Handle Disconnections**: Implement reconnection logic
7. **Validate Messages**: Check message format before sending

## Performance Considerations

- **Bootstrap Time**: First connection may take 10-30 seconds
- **DHT Queries**: O(log n) complexity for peer discovery
- **Message Latency**: Direct P2P connections are fast once established
- **Network Overhead**: DHT maintenance requires periodic queries

## Security Notes

- All connections are encrypted using Noise protocol
- Peer IDs are cryptographically derived from keypairs
- DHT records are stored with namespace-based keys
- No authentication beyond peer ID verification

## Further Reading

- [Kademlia Paper](https://pdos.csail.mit.edu/~petar/papers/maymounkov-kademlia-lncs.pdf)
- [libp2p Documentation](https://docs.rs/libp2p/)
- [Multiaddr Specification](https://github.com/multiformats/multiaddr)

## Support

For issues or questions:
1. Check the troubleshooting section
2. Review error messages carefully
3. Verify network connectivity
4. Check that all peers use the same namespace













