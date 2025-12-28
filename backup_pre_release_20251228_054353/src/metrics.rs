//! Metrics reporting protocol for peers to send metrics to monitor
//! Uses a separate request-response protocol from JSON messaging

use serde::{Deserialize, Serialize};
use libp2p::request_response::Codec;
use libp2p::StreamProtocol;
use libp2p::futures::{AsyncRead, AsyncWrite};
use std::io;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerMetrics {
    pub peer_id: String,
    pub namespace: String,
    pub messages_sent: u64,
    pub messages_received: u64,
    pub latency_samples: Vec<f64>, // Recent latency samples (ms)
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub message_errors: u64,
    pub timeout_errors: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsRequest {
    // Empty request - monitor just wants metrics
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsResponse {
    pub success: bool,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metrics: Option<PeerMetrics>,
}

/// Metrics codec for request-response protocol
#[derive(Clone, Debug)]
pub struct MetricsCodec;

#[async_trait::async_trait]
impl Codec for MetricsCodec {
    type Request = MetricsRequest;
    type Response = MetricsResponse;
    type Protocol = StreamProtocol;

    async fn read_request<T>(&mut self, _: &Self::Protocol, io: &mut T) -> std::io::Result<Self::Request>
    where
        T: AsyncRead + Unpin + Send,
    {
        use libp2p::futures::AsyncReadExt;
        let mut buffer = Vec::new();
        io.read_to_end(&mut buffer).await?;
        if buffer.is_empty() {
            // Empty request is valid
            Ok(MetricsRequest {})
        } else {
            serde_json::from_slice(&buffer).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
        }
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
        // Send empty request or minimal JSON
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

