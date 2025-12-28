//! QUIC Transport Module for libp2p
//!
//! This module provides QUIC-based transport as an alternative to TCP+Noise+Yamux.
//! QUIC provides built-in encryption (TLS 1.3), multiplexing, and operates over UDP.
//!
//! ## Benefits of QUIC over TCP
//! - **0-RTT connection establishment** (vs 3-way TCP handshake + TLS handshake)
//! - **Built-in encryption** (TLS 1.3) - no separate Noise handshake needed
//! - **Multiplexed streams** - no Yamux overhead
//! - **UDP-based** - better NAT traversal, works with hole punching
//! - **Connection migration** - survives IP address changes
//!
//! ## Usage
//! ```rust,ignore
//! use punch_simple::quic_transport::{create_quic_transport, create_dual_transport};
//! use libp2p::identity::Keypair;
//!
//! let key = Keypair::generate_ed25519();
//! // QUIC-only transport
//! let transport = create_quic_transport(&key).unwrap();
//!
//! // Or dual-stack (TCP fallback)
//! let transport = create_dual_transport(&key).unwrap();
//! ```

use libp2p::{
    core::transport::Transport,
    identity::Keypair,
    quic, noise, tcp, yamux,
    PeerId,
};

/// Error type for transport creation
#[derive(Debug)]
pub enum TransportError {
    QuicConfig(String),
    TcpConfig(String),
    NoiseConfig(String),
}

impl std::fmt::Display for TransportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TransportError::QuicConfig(e) => write!(f, "QUIC config error: {}", e),
            TransportError::TcpConfig(e) => write!(f, "TCP config error: {}", e),
            TransportError::NoiseConfig(e) => write!(f, "Noise config error: {}", e),
        }
    }
}

impl std::error::Error for TransportError {}

/// Create a QUIC-only transport
///
/// This transport uses libp2p's QUIC implementation (based on Quinn).
/// QUIC provides:
/// - Built-in TLS 1.3 encryption (no separate Noise needed)
/// - Built-in multiplexing (no Yamux needed)
/// - UDP-based for better NAT traversal
///
/// ## Example
/// ```rust,no_run
/// use libp2p::identity::Keypair;
/// use punch_simple::quic_transport::create_quic_transport;
///
/// let key = Keypair::generate_ed25519();
/// let transport = create_quic_transport(&key).unwrap();
/// ```
pub fn create_quic_transport(
    keypair: &Keypair,
) -> Result<libp2p::core::transport::Boxed<(PeerId, libp2p::core::muxing::StreamMuxerBox)>, TransportError> {
    // Create QUIC config with default settings
    let quic_config = quic::Config::new(keypair);
    
    // Create QUIC transport
    let transport = quic::tokio::Transport::new(quic_config)
        .map(|(peer_id, muxer), _| (peer_id, libp2p::core::muxing::StreamMuxerBox::new(muxer)))
        .boxed();
    
    Ok(transport)
}

/// Create a TCP transport with Noise encryption and Yamux multiplexing
///
/// This is the traditional libp2p transport stack.
///
/// ## Example
/// ```rust,no_run
/// use libp2p::identity::Keypair;
/// use punch_simple::quic_transport::create_tcp_transport;
///
/// let key = Keypair::generate_ed25519();
/// let transport = create_tcp_transport(&key).unwrap();
/// ```
pub fn create_tcp_transport(
    keypair: &Keypair,
) -> Result<libp2p::core::transport::Boxed<(PeerId, libp2p::core::muxing::StreamMuxerBox)>, TransportError> {
    let noise_config = noise::Config::new(keypair)
        .map_err(|e| TransportError::NoiseConfig(e.to_string()))?;
    
    let transport = tcp::tokio::Transport::default()
        .upgrade(libp2p::core::upgrade::Version::V1)
        .authenticate(noise_config)
        .multiplex(yamux::Config::default())
        .boxed();
    
    Ok(transport)
}

/// Create a dual-stack transport (QUIC + TCP fallback)
///
/// This transport tries QUIC first but falls back to TCP if QUIC fails.
/// This provides the best compatibility while preferring the faster protocol.
///
/// ## Example
/// ```rust,no_run
/// use libp2p::identity::Keypair;
/// use punch_simple::quic_transport::create_dual_transport;
///
/// let key = Keypair::generate_ed25519();
/// let transport = create_dual_transport(&key).unwrap();
/// ```
pub fn create_dual_transport(
    keypair: &Keypair,
) -> Result<libp2p::core::transport::Boxed<(PeerId, libp2p::core::muxing::StreamMuxerBox)>, TransportError> {
    // QUIC transport
    let quic_config = quic::Config::new(keypair);
    let quic_transport = quic::tokio::Transport::new(quic_config)
        .map(|(peer_id, muxer), _| (peer_id, libp2p::core::muxing::StreamMuxerBox::new(muxer)));
    
    // TCP transport with Noise + Yamux
    let noise_config = noise::Config::new(keypair)
        .map_err(|e| TransportError::NoiseConfig(e.to_string()))?;
    
    let tcp_transport = tcp::tokio::Transport::default()
        .upgrade(libp2p::core::upgrade::Version::V1)
        .authenticate(noise_config)
        .multiplex(yamux::Config::default());
    
    // Combine: prefer QUIC, fallback to TCP
    let transport = libp2p::core::transport::OrTransport::new(quic_transport, tcp_transport)
        .map(|either, _| match either {
            futures_util::future::Either::Left((peer_id, muxer)) => (peer_id, muxer),
            futures_util::future::Either::Right((peer_id, muxer)) => {
                (peer_id, libp2p::core::muxing::StreamMuxerBox::new(muxer))
            }
        })
        .boxed();
    
    Ok(transport)
}

/// Transport type selection
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportType {
    /// QUIC only (UDP-based, built-in encryption)
    QuicOnly,
    /// TCP only with Noise + Yamux (traditional)
    TcpOnly,
    /// Dual-stack: QUIC preferred, TCP fallback
    DualStack,
}

impl Default for TransportType {
    fn default() -> Self {
        TransportType::DualStack
    }
}

impl std::str::FromStr for TransportType {
    type Err = String;
    
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "quic" | "quic-only" => Ok(TransportType::QuicOnly),
            "tcp" | "tcp-only" => Ok(TransportType::TcpOnly),
            "dual" | "dual-stack" | "both" => Ok(TransportType::DualStack),
            _ => Err(format!("Unknown transport type: {}. Use 'quic', 'tcp', or 'dual'", s)),
        }
    }
}

/// Create a transport based on the specified type
pub fn create_transport(
    keypair: &Keypair,
    transport_type: TransportType,
) -> Result<libp2p::core::transport::Boxed<(PeerId, libp2p::core::muxing::StreamMuxerBox)>, TransportError> {
    match transport_type {
        TransportType::QuicOnly => create_quic_transport(keypair),
        TransportType::TcpOnly => create_tcp_transport(keypair),
        TransportType::DualStack => create_dual_transport(keypair),
    }
}

/// Get listen address for a transport type
pub fn get_listen_address(transport_type: TransportType, port: u16) -> String {
    match transport_type {
        TransportType::QuicOnly => format!("/ip4/0.0.0.0/udp/{}/quic-v1", port),
        TransportType::TcpOnly => format!("/ip4/0.0.0.0/tcp/{}", port),
        TransportType::DualStack => {
            // For dual-stack, caller should listen on both
            // Return QUIC address as primary
            format!("/ip4/0.0.0.0/udp/{}/quic-v1", port)
        }
    }
}

/// Get both listen addresses for dual-stack transport
pub fn get_dual_listen_addresses(port: u16) -> (String, String) {
    (
        format!("/ip4/0.0.0.0/udp/{}/quic-v1", port),
        format!("/ip4/0.0.0.0/tcp/{}", port),
    )
}

/// Transport statistics for monitoring
#[derive(Debug, Clone, Default)]
pub struct TransportStats {
    pub quic_connections: u64,
    pub tcp_connections: u64,
    pub quic_bytes_sent: u64,
    pub quic_bytes_received: u64,
    pub tcp_bytes_sent: u64,
    pub tcp_bytes_received: u64,
    pub quic_connection_failures: u64,
    pub tcp_connection_failures: u64,
}

impl TransportStats {
    pub fn new() -> Self {
        Self::default()
    }
    
    pub fn total_connections(&self) -> u64 {
        self.quic_connections + self.tcp_connections
    }
    
    pub fn total_bytes_sent(&self) -> u64 {
        self.quic_bytes_sent + self.tcp_bytes_sent
    }
    
    pub fn total_bytes_received(&self) -> u64 {
        self.quic_bytes_received + self.tcp_bytes_received
    }
    
    pub fn quic_ratio(&self) -> f64 {
        let total = self.total_connections();
        if total == 0 {
            0.0
        } else {
            self.quic_connections as f64 / total as f64
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_transport_type_from_str() {
        assert_eq!("quic".parse::<TransportType>().unwrap(), TransportType::QuicOnly);
        assert_eq!("tcp".parse::<TransportType>().unwrap(), TransportType::TcpOnly);
        assert_eq!("dual".parse::<TransportType>().unwrap(), TransportType::DualStack);
        assert_eq!("QUIC".parse::<TransportType>().unwrap(), TransportType::QuicOnly);
        assert!("invalid".parse::<TransportType>().is_err());
    }
    
    #[test]
    fn test_transport_type_default() {
        assert_eq!(TransportType::default(), TransportType::DualStack);
    }
    
    #[test]
    fn test_get_listen_address() {
        assert_eq!(
            get_listen_address(TransportType::QuicOnly, 51820),
            "/ip4/0.0.0.0/udp/51820/quic-v1"
        );
        assert_eq!(
            get_listen_address(TransportType::TcpOnly, 51820),
            "/ip4/0.0.0.0/tcp/51820"
        );
    }
    
    #[test]
    fn test_dual_listen_addresses() {
        let (quic, tcp) = get_dual_listen_addresses(51820);
        assert!(quic.contains("udp"));
        assert!(quic.contains("quic-v1"));
        assert!(tcp.contains("tcp"));
    }
    
    #[test]
    fn test_transport_stats() {
        let mut stats = TransportStats::new();
        stats.quic_connections = 10;
        stats.tcp_connections = 5;
        
        assert_eq!(stats.total_connections(), 15);
        assert!((stats.quic_ratio() - 0.666).abs() < 0.01);
    }
    
    #[tokio::test]
    async fn test_create_quic_transport() {
        let key = Keypair::generate_ed25519();
        let transport = create_quic_transport(&key);
        assert!(transport.is_ok());
    }
    
    #[tokio::test]
    async fn test_create_tcp_transport() {
        let key = Keypair::generate_ed25519();
        let transport = create_tcp_transport(&key);
        assert!(transport.is_ok());
    }
    
    #[tokio::test]
    async fn test_create_dual_transport() {
        let key = Keypair::generate_ed25519();
        let transport = create_dual_transport(&key);
        assert!(transport.is_ok());
    }
    
    #[tokio::test]
    async fn test_create_transport_by_type() {
        let key = Keypair::generate_ed25519();
        
        assert!(create_transport(&key, TransportType::QuicOnly).is_ok());
        assert!(create_transport(&key, TransportType::TcpOnly).is_ok());
        assert!(create_transport(&key, TransportType::DualStack).is_ok());
    }
}

