//! QUIC Protocol Diagnostics Module
//! 
//! Comprehensive protocol analyzer for QUIC connections on the rendezvous server.
//! Tracks handshake stages, connection events, errors, and performance metrics.

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use serde::{Serialize, Deserialize};
use libp2p::{PeerId, Multiaddr};

/// QUIC handshake stage
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum QuicHandshakeStage {
    /// Initial packet sent/received
    Initial,
    /// Handshake packet sent/received
    Handshake,
    /// 1-RTT packet (connection established)
    OneRtt,
    /// Handshake completed
    #[default]
    Completed,
    /// Handshake failed
    Failed,
}

/// QUIC connection event type
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QuicEventType {
    /// Connection attempt initiated
    ConnectionAttempt,
    /// Initial packet sent
    InitialSent,
    /// Initial packet received
    InitialReceived,
    /// Handshake packet sent
    HandshakeSent,
    /// Handshake packet received
    HandshakeReceived,
    /// Connection established
    Established,
    /// Connection closed
    Closed,
    /// Connection error
    Error,
    /// Handshake timeout
    HandshakeTimeout,
    /// Connection migration
    Migration,
    /// Stream opened
    StreamOpened,
    /// Stream closed
    StreamClosed,
}

/// Detailed QUIC connection event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuicConnectionEvent {
    pub timestamp: u64,
    pub event_type: QuicEventType,
    pub peer_id: Option<String>,
    pub remote_addr: Option<String>,
    pub local_addr: Option<String>,
    pub handshake_stage: Option<QuicHandshakeStage>,
    pub error: Option<String>,
    pub details: HashMap<String, String>,
}

/// QUIC connection statistics
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct QuicConnectionStats {
    pub peer_id: String,
    pub remote_addr: String,
    pub local_addr: String,
    pub established_at: Option<u64>,
    pub closed_at: Option<u64>,
    pub duration_ms: Option<u64>,
    pub handshake_duration_ms: Option<u64>,
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub packets_sent: u64,
    pub packets_received: u64,
    pub streams_opened: u64,
    pub streams_closed: u64,
    pub retransmissions: u64,
    pub handshake_stage: QuicHandshakeStage,
    pub error_count: u64,
    pub last_error: Option<String>,
    pub events: Vec<QuicConnectionEvent>,
}

/// QUIC diagnostic state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuicDiagnostics {
    pub total_connections: u64,
    pub active_connections: u64,
    pub failed_connections: u64,
    pub handshake_timeouts: u64,
    pub total_bytes_sent: u64,
    pub total_bytes_received: u64,
    pub total_packets_sent: u64,
    pub total_packets_received: u64,
    pub average_handshake_duration_ms: f64,
    pub connections: HashMap<String, QuicConnectionStats>,
    pub recent_events: VecDeque<QuicConnectionEvent>,
    pub error_log: VecDeque<String>,
}

impl Default for QuicDiagnostics {
    fn default() -> Self {
        Self {
            total_connections: 0,
            active_connections: 0,
            failed_connections: 0,
            handshake_timeouts: 0,
            total_bytes_sent: 0,
            total_bytes_received: 0,
            total_packets_sent: 0,
            total_packets_received: 0,
            average_handshake_duration_ms: 0.0,
            connections: HashMap::new(),
            recent_events: VecDeque::with_capacity(1000),
            error_log: VecDeque::with_capacity(500),
        }
    }
}

/// QUIC Diagnostics Manager
pub struct QuicDiagnosticsManager {
    state: Arc<RwLock<QuicDiagnostics>>,
    max_events: usize,
    max_errors: usize,
}

impl QuicDiagnosticsManager {
    pub fn new() -> Self {
        Self {
            state: Arc::new(RwLock::new(QuicDiagnostics::default())),
            max_events: 1000,
            max_errors: 500,
        }
    }

    pub fn state(&self) -> Arc<RwLock<QuicDiagnostics>> {
        self.state.clone()
    }

    /// Record a connection attempt
    pub async fn record_connection_attempt(
        &self,
        peer_id: Option<PeerId>,
        remote_addr: Option<&Multiaddr>,
        local_addr: Option<&Multiaddr>,
    ) {
        let mut state = self.state.write().await;
        state.total_connections += 1;
        state.active_connections += 1;

        let peer_id_str = peer_id.map(|p| p.to_string());
        let remote_addr_str = remote_addr.map(|a| a.to_string());
        let local_addr_str = local_addr.map(|a| a.to_string());

        let connection_id = format!(
            "{}_{}",
            peer_id_str.as_deref().unwrap_or("unknown"),
            remote_addr_str.as_deref().unwrap_or("unknown")
        );

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let event = QuicConnectionEvent {
            timestamp: now,
            event_type: QuicEventType::ConnectionAttempt,
            peer_id: peer_id_str.clone(),
            remote_addr: remote_addr_str.clone(),
            local_addr: local_addr_str.clone(),
            handshake_stage: Some(QuicHandshakeStage::Initial),
            error: None,
            details: HashMap::new(),
        };

        state.recent_events.push_back(event.clone());
        if state.recent_events.len() > self.max_events {
            state.recent_events.pop_front();
        }

        let stats = QuicConnectionStats {
            peer_id: peer_id_str.unwrap_or_else(|| "unknown".to_string()),
            remote_addr: remote_addr_str.unwrap_or_else(|| "unknown".to_string()),
            local_addr: local_addr_str.unwrap_or_else(|| "unknown".to_string()),
            established_at: None,
            closed_at: None,
            duration_ms: None,
            handshake_duration_ms: None,
            bytes_sent: 0,
            bytes_received: 0,
            packets_sent: 0,
            packets_received: 0,
            streams_opened: 0,
            streams_closed: 0,
            retransmissions: 0,
            handshake_stage: QuicHandshakeStage::Initial,
            error_count: 0,
            last_error: None,
            events: vec![event],
        };

        state.connections.insert(connection_id, stats);
    }

    /// Record connection established
    pub async fn record_connection_established(
        &self,
        peer_id: PeerId,
        remote_addr: &Multiaddr,
        local_addr: Option<&Multiaddr>,
        handshake_duration_ms: Option<u64>,
    ) {
        let mut state = self.state.write().await;
        let peer_id_str = peer_id.to_string();
        let remote_addr_str = remote_addr.to_string();
        let local_addr_str = local_addr.map(|a| a.to_string());

        let connection_id = format!("{}_{}", peer_id_str, remote_addr_str);

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let event = QuicConnectionEvent {
            timestamp: now,
            event_type: QuicEventType::Established,
            peer_id: Some(peer_id_str.clone()),
            remote_addr: Some(remote_addr_str.clone()),
            local_addr: local_addr_str.clone(),
            handshake_stage: Some(QuicHandshakeStage::Completed),
            error: None,
            details: HashMap::new(),
        };

        state.recent_events.push_back(event.clone());
        if state.recent_events.len() > self.max_events {
            state.recent_events.pop_front();
        }

        if let Some(stats) = state.connections.get_mut(&connection_id) {
            stats.established_at = Some(now);
            stats.handshake_stage = QuicHandshakeStage::Completed;
            stats.handshake_duration_ms = handshake_duration_ms;
            stats.events.push(event);

            // Update average handshake duration
            if let Some(duration) = handshake_duration_ms {
                let total = state.total_connections as f64;
                let current_avg = state.average_handshake_duration_ms;
                state.average_handshake_duration_ms =
                    (current_avg * (total - 1.0) + duration as f64) / total;
            }
        } else {
            // Create new stats if not found
            let stats = QuicConnectionStats {
                peer_id: peer_id_str,
                remote_addr: remote_addr_str,
                local_addr: local_addr_str.unwrap_or_else(|| "unknown".to_string()),
                established_at: Some(now),
                closed_at: None,
                duration_ms: None,
                handshake_duration_ms,
                bytes_sent: 0,
                bytes_received: 0,
                packets_sent: 0,
                packets_received: 0,
                streams_opened: 0,
                streams_closed: 0,
                retransmissions: 0,
                handshake_stage: QuicHandshakeStage::Completed,
                error_count: 0,
                last_error: None,
                events: vec![event],
            };
            state.connections.insert(connection_id, stats);
        }
    }

    /// Record connection error
    pub async fn record_connection_error(
        &self,
        peer_id: Option<PeerId>,
        remote_addr: Option<&Multiaddr>,
        error: &str,
        handshake_stage: Option<QuicHandshakeStage>,
    ) {
        let mut state = self.state.write().await;
        state.failed_connections += 1;
        state.active_connections = state.active_connections.saturating_sub(1);

        let peer_id_str = peer_id.map(|p| p.to_string());
        let remote_addr_str = remote_addr.map(|a| a.to_string());

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let event = QuicConnectionEvent {
            timestamp: now,
            event_type: QuicEventType::Error,
            peer_id: peer_id_str.clone(),
            remote_addr: remote_addr_str.clone(),
            local_addr: None,
            handshake_stage,
            error: Some(error.to_string()),
            details: HashMap::new(),
        };

        state.recent_events.push_back(event.clone());
        if state.recent_events.len() > self.max_events {
            state.recent_events.pop_front();
        }

        // Add to error log
        let error_msg = format!(
            "[{}] {}: {}",
            now,
            peer_id_str.as_deref().unwrap_or("unknown"),
            error
        );
        state.error_log.push_back(error_msg.clone());
        if state.error_log.len() > self.max_errors {
            state.error_log.pop_front();
        }

        // Update connection stats if exists
        if let (Some(pid), Some(addr)) = (peer_id_str.as_ref(), remote_addr_str.as_ref()) {
            let connection_id = format!("{}_{}", pid, addr);
            if let Some(stats) = state.connections.get_mut(&connection_id) {
                stats.error_count += 1;
                stats.last_error = Some(error.to_string());
                stats.handshake_stage = handshake_stage.unwrap_or(QuicHandshakeStage::Failed);
                stats.events.push(event);
            }
        }

        // Check for handshake timeout
        if error.contains("HandshakeTimedOut") || error.contains("timeout") {
            state.handshake_timeouts += 1;
        }
    }

    /// Record connection closed
    pub async fn record_connection_closed(
        &self,
        peer_id: PeerId,
        remote_addr: &Multiaddr,
        cause: Option<&str>,
    ) {
        let mut state = self.state.write().await;
        state.active_connections = state.active_connections.saturating_sub(1);

        let peer_id_str = peer_id.to_string();
        let remote_addr_str = remote_addr.to_string();
        let connection_id = format!("{}_{}", peer_id_str, remote_addr_str);

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let event = QuicConnectionEvent {
            timestamp: now,
            event_type: QuicEventType::Closed,
            peer_id: Some(peer_id_str.clone()),
            remote_addr: Some(remote_addr_str.clone()),
            local_addr: None,
            handshake_stage: None,
            error: cause.map(|s| s.to_string()),
            details: HashMap::new(),
        };

        state.recent_events.push_back(event.clone());
        if state.recent_events.len() > self.max_events {
            state.recent_events.pop_front();
        }

        if let Some(stats) = state.connections.get_mut(&connection_id) {
            stats.closed_at = Some(now);
            if let Some(established) = stats.established_at {
                stats.duration_ms = Some(now.saturating_sub(established));
            }
            stats.events.push(event);
        }
    }

    /// Record handshake stage progression
    pub async fn record_handshake_stage(
        &self,
        peer_id: Option<PeerId>,
        remote_addr: Option<&Multiaddr>,
        stage: QuicHandshakeStage,
    ) {
        let mut state = self.state.write().await;
        let peer_id_str = peer_id.map(|p| p.to_string());
        let remote_addr_str = remote_addr.map(|a| a.to_string());

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let event_type = match stage {
            QuicHandshakeStage::Initial => QuicEventType::InitialReceived,
            QuicHandshakeStage::Handshake => QuicEventType::HandshakeReceived,
            QuicHandshakeStage::OneRtt => QuicEventType::Established,
            _ => QuicEventType::ConnectionAttempt,
        };

        let event = QuicConnectionEvent {
            timestamp: now,
            event_type,
            peer_id: peer_id_str.clone(),
            remote_addr: remote_addr_str.clone(),
            local_addr: None,
            handshake_stage: Some(stage),
            error: None,
            details: HashMap::new(),
        };

        state.recent_events.push_back(event.clone());
        if state.recent_events.len() > self.max_events {
            state.recent_events.pop_front();
        }

        if let (Some(pid), Some(addr)) = (peer_id_str.as_ref(), remote_addr_str.as_ref()) {
            let connection_id = format!("{}_{}", pid, addr);
            if let Some(stats) = state.connections.get_mut(&connection_id) {
                stats.handshake_stage = stage;
                stats.events.push(event);
            }
        }
    }

    /// Get current diagnostics snapshot
    pub async fn get_diagnostics(&self) -> QuicDiagnostics {
        self.state.read().await.clone()
    }

    /// Get connection stats for a specific peer
    pub async fn get_connection_stats(&self, peer_id: &str, remote_addr: &str) -> Option<QuicConnectionStats> {
        let state = self.state.read().await;
        let connection_id = format!("{}_{}", peer_id, remote_addr);
        state.connections.get(&connection_id).cloned()
    }

    /// Get recent events (last N events)
    pub async fn get_recent_events(&self, limit: usize) -> Vec<QuicConnectionEvent> {
        let state = self.state.read().await;
        state.recent_events
            .iter()
            .rev()
            .take(limit)
            .cloned()
            .collect()
    }

    /// Get error log
    pub async fn get_error_log(&self, limit: usize) -> Vec<String> {
        let state = self.state.read().await;
        state.error_log
            .iter()
            .rev()
            .take(limit)
            .cloned()
            .collect()
    }
}

impl Default for QuicDiagnosticsManager {
    fn default() -> Self {
        Self::new()
    }
}
