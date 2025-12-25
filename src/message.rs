//! Shared message types for JSON communication between listener and dialer

use serde::{Deserialize, Serialize};
use libp2p::request_response::Codec;
use libp2p::StreamProtocol;
use libp2p::futures::{AsyncRead, AsyncWrite};
use std::io;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonMessage {
    pub from: String,
    pub message: String,
    pub timestamp: u64,
    #[serde(default)]
    pub send_time_ms: Option<u64>, // For latency tracking
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
            send_time_ms: Some(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_millis() as u64,
            ),
        }
    }
}

/// JSON codec for request-response protocol
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



