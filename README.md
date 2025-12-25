# Punch Simple - Kademlia P2P Network

A simple libp2p-based peer-to-peer networking library using **Kademlia DHT** for decentralized peer discovery and connection.

## Overview

This project provides a decentralized P2P networking solution with three main components:

- **server** - Bootstrap node that helps peers join the Kademlia DHT network
- **listener** - Peer that joins the DHT and waits for incoming connections
- **dialer** - Peer that discovers and connects to other peers via the DHT

## Key Features

- **Decentralized**: Uses Kademlia DHT - no central server required after bootstrap
- **libp2p 0.53** compatible with modern networking stack
- **Encrypted**: TCP transport with Noise encryption and Yamux multiplexing
- **JSON Messaging**: Built-in request-response protocol for JSON message exchange
- **Cross-platform**: Works on Windows, Linux, and macOS
- **Simple API**: Easy-to-use client helper for integration into your applications

## Architecture

### Kademlia DHT

Unlike centralized rendezvous systems, Kademlia is a **distributed hash table (DHT)** protocol:

- **Decentralized**: Peers discover each other without a central authority
- **Scalable**: Network grows organically as more peers join
- **Resilient**: No single point of failure
- **Efficient**: O(log n) lookup complexity

### How It Works

1. **Bootstrap**: Peers connect to bootstrap nodes to join the network
2. **Discovery**: Peers query the DHT to find other peers in the same namespace
3. **Connection**: Direct peer-to-peer connections are established
4. **Messaging**: JSON messages are exchanged over encrypted connections

## Building

```bash
cargo build --release
```

This builds three binaries:
- `target/release/server.exe` (or `server` on Unix)
- `target/release/listener.exe`
- `target/release/dialer.exe`

## Quick Start

### 1. Start Bootstrap Node

The bootstrap node helps peers discover each other initially:

```bash
# Default (listens on 0.0.0.0:51820)
cargo run --release --bin server

# Custom address and port
cargo run --release --bin server -- --listen-addr 0.0.0.0 --port 8080
```

**Output:**
```
=== Simple Kademlia Bootstrap Node ===

Configuration:
  Listen Address: 0.0.0.0:51820

Local peer id: 12D3KooW...

✅ Bootstrap node started! Waiting for connections...

Clients can bootstrap to this node using:
  --bootstrap /ip4/0.0.0.0/tcp/51820
```

### 2. Start Listener (Peer A)

A listener joins the DHT and waits for connections:

```bash
# Default (bootstrap to localhost:51820, namespace: simple-chat)
cargo run --release --bin listener

# Custom bootstrap node and namespace
cargo run --release --bin listener -- --bootstrap /ip4/127.0.0.1/tcp/51820 --namespace my-app
```

**Output:**
```
=== Simple Kademlia Listener ===

Configuration:
  Bootstrap: /ip4/127.0.0.1/tcp/51820
  Namespace: my-app

Peer ID: 12D3KooW...

✓ DHT bootstrapped!
✓ Registered in DHT! Waiting for connections...

Your Peer ID: 12D3KooW...
```

### 3. Start Dialer (Peer B)

A dialer discovers and connects to peers:

```bash
# Default (bootstrap to localhost:51820, namespace: simple-chat)
cargo run --release --bin dialer

# Custom bootstrap node and namespace
cargo run --release --bin dialer -- --bootstrap /ip4/127.0.0.1/tcp/51820 --namespace my-app
```

**Output:**
```
=== Simple Kademlia Dialer ===

Configuration:
  Bootstrap: /ip4/127.0.0.1/tcp/51820
  Namespace: my-app

Local Peer ID: 12D3KooW...

✓ DHT bootstrapped! Discovering peers...
✓✓✓ CONNECTED to peer: 12D3KooW...

[📤 SENT JSON MESSAGE] to peer 12D3KooW...
  From: dialer-12D3KooW
  Message: Hello from dialer! Message #1
```

## Command-Line Arguments

### Server (Bootstrap Node)

```bash
cargo run --release --bin server [OPTIONS]
```

Options:
- `--listen-addr <ADDR>` - Address to listen on (default: `0.0.0.0`)
- `--port <PORT>` - Port to listen on (default: `51820`)

### Listener

```bash
cargo run --release --bin listener [OPTIONS]
```

Options:
- `--bootstrap <ADDR>` - Bootstrap node address in Multiaddr format (default: `/ip4/127.0.0.1/tcp/51820`)
- `--namespace <NAMESPACE>` - Namespace for peer discovery (default: `simple-chat`)

### Dialer

```bash
cargo run --release --bin dialer [OPTIONS]
```

Options:
- `--bootstrap <ADDR>` - Bootstrap node address in Multiaddr format (default: `/ip4/127.0.0.1/tcp/51820`)
- `--namespace <NAMESPACE>` - Namespace for peer discovery (default: `simple-chat`)

## Multiaddr Format

Bootstrap addresses use the **Multiaddr** format:

```
/ip4/127.0.0.1/tcp/51820          # IPv4 localhost
/ip4/192.168.1.100/tcp/8080       # IPv4 with custom port
/ip6/::1/tcp/51820                # IPv6 localhost
/dns4/example.com/tcp/51820       # DNS name (IPv4)
```

## Example Workflows

### Local Testing

**Terminal 1 - Bootstrap Node:**
```bash
cargo run --release --bin server
```

**Terminal 2 - Listener:**
```bash
cargo run --release --bin listener -- --namespace test
```

**Terminal 3 - Dialer:**
```bash
cargo run --release --bin dialer -- --namespace test
```

The dialer will discover and connect to the listener automatically.

### Remote Peers

**Machine A (Bootstrap + Listener):**
```bash
# Start bootstrap node
cargo run --release --bin server -- --port 51820

# In another terminal, start listener
cargo run --release --bin listener -- --bootstrap /ip4/0.0.0.0/tcp/51820
```

**Machine B (Dialer):**
```bash
# Connect to Machine A's bootstrap node
cargo run --release --bin dialer -- \
  --bootstrap /ip4/MACHINE_A_IP/tcp/51820 \
  --namespace simple-chat
```

### Multiple Bootstrap Nodes

For better reliability, you can specify multiple bootstrap nodes:

```bash
# Note: The current implementation uses a single bootstrap node
# Multiple bootstrap support can be added by modifying the code
```

## Integration

See [INTEGRATION_EXAMPLE.md](INTEGRATION_EXAMPLE.md) for detailed integration examples using the `P2PClient` helper.

### Quick Integration Example

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
    
    // Discover and connect to a peer
    let peer_id = client.connect_to_peer().await?;
    
    // Send a JSON message
    let response = client.send_and_wait(peer_id, json!({
        "from": "my-app",
        "message": "Hello!",
        "timestamp": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs()
    })).await?;
    
    println!("Response: {}", response);
    Ok(())
}
```

## Dependencies

- **libp2p 0.53** - P2P networking stack with Kademlia DHT
- **tokio 1.35** - Async runtime
- **serde/serde_json** - JSON serialization
- **clap 4.4** - Command-line argument parsing

## How Kademlia Differs from Rendezvous

| Feature | Rendezvous (Old) | Kademlia (Current) |
|---------|------------------|-------------------|
| **Architecture** | Centralized server | Decentralized DHT |
| **Discovery** | Server maintains peer list | Peers query DHT |
| **Scalability** | Limited by server capacity | Scales with network size |
| **Reliability** | Single point of failure | No single point of failure |
| **Bootstrap** | Connect to server | Connect to bootstrap nodes |
| **Configuration** | `--server HOST --port PORT` | `--bootstrap /ip4/HOST/tcp/PORT` |

## Troubleshooting

### Peers Can't Find Each Other

1. **Check namespace**: Both peers must use the **same namespace**
2. **Check bootstrap**: Both peers must bootstrap to the **same network**
3. **Wait for bootstrap**: Allow 10-30 seconds for DHT to populate
4. **Check firewall**: Ensure ports are accessible

### Connection Timeouts

- Verify bootstrap node is running and accessible
- Check network connectivity between peers
- Ensure both peers are in the same namespace
- Try increasing query timeout in code (default: 60 seconds)

### Bootstrap Node Not Found

- Verify the bootstrap node is listening on the correct address
- Check firewall rules allow incoming connections
- Use `0.0.0.0` as listen address (not `127.0.0.1`) for remote access

## License

This project is provided as-is for demonstration purposes.
