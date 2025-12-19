# Simple Usage: P2P JSON Messaging Code Fragment

This is a **standalone code fragment** you can copy into your program to enable P2P JSON messaging.

## Quick Copy-Paste Solution

### Step 1: Add Dependencies

Add to your `Cargo.toml`:

```toml
[dependencies]
libp2p = { version = "0.53", features = ["quic", "noise", "tcp", "dns", "macros", "rendezvous", "identify", "yamux", "tokio", "request-response"] }
tokio = { version = "1.35", features = ["full"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
async-trait = "0.1"
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

### Step 3: Use the Simple Function

```rust
use serde_json::json;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Send a message and wait for response
    let response = send_p2p_json_message(
        "127.0.0.1:51820",  // Rendezvous server
        "simple-chat",      // Namespace
        "Hello from my application!"  // Your message
    ).await?;
    
    println!("Received response: {}", serde_json::to_string_pretty(&response)?);
    
    Ok(())
}
```

## What It Does

1. **Connects** to the rendezvous server
2. **Discovers** peers in the specified namespace
3. **Connects** to the first available peer
4. **Sends** your JSON message
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

## Notes

- The function blocks until a peer is found and connected
- Timeout is 10 seconds for receiving a response
- The peer must be running and registered in the same namespace
- Messages are sent over encrypted libp2p connections

## Full Example

See `CODE_FRAGMENT.rs` for the complete implementation that you can copy into your project.

