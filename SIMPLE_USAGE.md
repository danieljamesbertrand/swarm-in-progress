# Simple Usage: P2P JSON Messaging with Kademlia

Quick-start guide for using P2P JSON messaging with Kademlia DHT.

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

### Step 2: Copy Message Types

Copy the `message.rs` file from this project, or include these types:

```rust
use serde::{Deserialize, Serialize};
use libp2p::request_response::Codec;
use libp2p::StreamProtocol;
use libp2p::futures::{AsyncRead, AsyncWrite};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonMessage {
    pub from: String,
    pub message: String,
    pub timestamp: u64,
}

impl JsonMessage {
    pub fn new(from: String, message: String) -> Self {
        Self {
            from,
            message,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct JsonCodec;

#[async_trait::async_trait]
impl Codec for JsonCodec {
    type Request = JsonMessage;
    type Response = JsonMessage;
    type Protocol = StreamProtocol;

    async fn read_request<T>(&mut self, _: &Self::Protocol, io: &mut T) -> std::io::Result<Self::Request>
    where
        T: AsyncRead + Unpin + Send,
    {
        use libp2p::futures::AsyncReadExt;
        let mut buffer = Vec::new();
        io.read_to_end(&mut buffer).await?;
        serde_json::from_slice(&buffer).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }

    async fn read_response<T>(&mut self, _: &Self::Protocol, io: &mut T) -> std::io::Result<Self::Response>
    where
        T: AsyncRead + Unpin + Send,
    {
        use libp2p::futures::AsyncReadExt;
        let mut buffer = Vec::new();
        io.read_to_end(&mut buffer).await?;
        serde_json::from_slice(&buffer).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }

    async fn write_request<T>(&mut self, _: &Self::Protocol, io: &mut T, req: Self::Request) -> std::io::Result<()>
    where
        T: AsyncWrite + Unpin + Send,
    {
        use libp2p::futures::AsyncWriteExt;
        let json = serde_json::to_vec(&req).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        io.write_all(&json).await?;
        Ok(())
    }

    async fn write_response<T>(&mut self, _: &Self::Protocol, io: &mut T, res: Self::Response) -> std::io::Result<()>
    where
        T: AsyncWrite + Unpin + Send,
    {
        use libp2p::futures::AsyncWriteExt;
        let json = serde_json::to_vec(&res).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        io.write_all(&json).await?;
        Ok(())
    }
}
```

### Step 3: Use the Client Helper

```rust
use client_helper::P2PClient;
use serde_json::json;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Bootstrap to DHT network
    let mut client = P2PClient::new(
        &["/ip4/127.0.0.1/tcp/51820"],  // Bootstrap node
        "my-namespace"                  // Namespace
    ).await?;
    
    // Discover and connect to a peer
    let peer_id = client.connect_to_peer().await?;
    
    // Send a message
    let response = client.send_and_wait(peer_id, json!({
        "from": "my-app",
        "message": "Hello from my application!",
        "timestamp": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs()
    })).await?;
    
    println!("Received response: {}", serde_json::to_string_pretty(&response)?);
    
    Ok(())
}
```

## What It Does

1. **Bootstraps** to the Kademlia DHT network via bootstrap nodes
2. **Discovers** peers in the specified namespace using DHT queries
3. **Connects** to the first available peer
4. **Sends** your JSON message over encrypted connection
5. **Waits** for a response (10 second timeout)
6. **Returns** the response as JSON

## Response Format

The response will be a JSON object:
```json
{
  "from": "peer-name",
  "message": "response text",
  "timestamp": 1234567890
}
```

## Key Differences from Rendezvous

| Aspect | Old (Rendezvous) | New (Kademlia) |
|--------|------------------|----------------|
| **API** | `P2PClient::new(server, namespace)` | `P2PClient::new(bootstrap_nodes, namespace)` |
| **Server** | Central rendezvous server required | Bootstrap nodes (decentralized) |
| **Address Format** | `"127.0.0.1:51820"` | `"/ip4/127.0.0.1/tcp/51820"` (Multiaddr) |
| **Discovery** | Server maintains peer list | DHT queries for peers |

## Notes

- The function blocks until a peer is found and connected
- Timeout is 10 seconds for receiving a response
- The peer must be running and in the same namespace
- Messages are sent over encrypted libp2p connections using Noise protocol
- DHT discovery may take 10-30 seconds after bootstrap

## Full Example

See `INTEGRATION_EXAMPLE.md` for complete integration examples and API reference.
