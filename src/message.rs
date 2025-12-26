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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_json_message_new() {
        let msg = JsonMessage::new("sender".to_string(), "Hello".to_string());
        assert_eq!(msg.from, "sender");
        assert_eq!(msg.message, "Hello");
        assert!(msg.timestamp > 0);
        assert!(msg.send_time_ms.is_some());
    }

    #[tokio::test]
    async fn test_json_codec_serialization() {
        let mut codec = JsonCodec;
        let protocol = StreamProtocol::new("/test/1.0");
        
        let message = JsonMessage::new("test".to_string(), "message".to_string());
        
        // Test serialization
        let mut buffer = Vec::new();
        codec.write_request(&protocol, &mut buffer, message.clone()).await.unwrap();
        
        // Test deserialization using &[u8] which implements AsyncRead
        let mut reader: &[u8] = &buffer;
        let deserialized = codec.read_request(&protocol, &mut reader).await.unwrap();
        
        assert_eq!(message.from, deserialized.from);
        assert_eq!(message.message, deserialized.message);
    }

    #[test]
    fn test_json_message_timestamp() {
        let msg1 = JsonMessage::new("sender".to_string(), "msg1".to_string());
        std::thread::sleep(std::time::Duration::from_millis(10));
        let msg2 = JsonMessage::new("sender".to_string(), "msg2".to_string());
        
        // Timestamps should be different (or very close)
        assert!(msg2.timestamp >= msg1.timestamp);
    }
}



