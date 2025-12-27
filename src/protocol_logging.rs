//! Protocol Logging Module
//! 
//! Provides structured logging for all connections and transactions across protocols.
//! Logs every connection establishment, command execution, and protocol interaction.

use std::time::{SystemTime, UNIX_EPOCH};
use serde::{Serialize, Deserialize};

/// Connection event types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConnectionEventType {
    Established,
    Closed,
    Failed,
    Rejected,
}

/// Connection log entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionLog {
    pub timestamp: u64,
    pub event_type: ConnectionEventType,
    pub peer_id: String,
    pub direction: String, // "inbound" or "outbound"
    pub protocol: String, // "QUIC", "TCP", etc.
    pub endpoint: Option<String>,
    pub error: Option<String>,
}

/// Transaction event types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TransactionEventType {
    Started,
    Completed,
    Failed,
    Timeout,
}

/// Transaction log entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionLog {
    pub timestamp: u64,
    pub event_type: TransactionEventType,
    pub protocol: String, // "JSON_COMMAND", "TORRENT", "METRICS", etc.
    pub command: Option<String>,
    pub request_id: Option<String>,
    pub from_peer: String,
    pub to_peer: Option<String>,
    pub duration_ms: Option<u64>,
    pub error: Option<String>,
    pub result_size: Option<usize>,
}

/// Log a connection event
pub fn log_connection(
    event_type: ConnectionEventType,
    peer_id: &str,
    direction: &str,
    protocol: &str,
    endpoint: Option<&str>,
    error: Option<&str>,
) {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    
    let log = ConnectionLog {
        timestamp,
        event_type: event_type.clone(),
        peer_id: peer_id.to_string(),
        direction: direction.to_string(),
        protocol: protocol.to_string(),
        endpoint: endpoint.map(|s| s.to_string()),
        error: error.map(|s| s.to_string()),
    };
    
    // Log to stdout with structured format
    match event_type {
        ConnectionEventType::Established => {
            println!("[CONN] ✓ {} {} connection established: {} ({})", 
                direction, protocol, peer_id, 
                endpoint.unwrap_or("unknown"));
        }
        ConnectionEventType::Closed => {
            println!("[CONN] ✗ {} {} connection closed: {}", 
                direction, protocol, peer_id);
        }
        ConnectionEventType::Failed => {
            eprintln!("[CONN] ✗ {} {} connection failed: {} - {}", 
                direction, protocol, peer_id, 
                error.unwrap_or("unknown error"));
        }
        ConnectionEventType::Rejected => {
            eprintln!("[CONN] ✗ {} {} connection rejected: {} - {}", 
                direction, protocol, peer_id, 
                error.unwrap_or("unknown reason"));
        }
    }
    
    // Could also write to file or send to logging service
    // For now, stdout is sufficient
}

/// Log a transaction event
pub fn log_transaction(
    event_type: TransactionEventType,
    protocol: &str,
    command: Option<&str>,
    request_id: Option<&str>,
    from_peer: &str,
    to_peer: Option<&str>,
    duration_ms: Option<u64>,
    error: Option<&str>,
    result_size: Option<usize>,
) {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    
    let log = TransactionLog {
        timestamp,
        event_type: event_type.clone(),
        protocol: protocol.to_string(),
        command: command.map(|s| s.to_string()),
        request_id: request_id.map(|s| s.to_string()),
        from_peer: from_peer.to_string(),
        to_peer: to_peer.map(|s| s.to_string()),
        duration_ms,
        error: error.map(|s| s.to_string()),
        result_size,
    };
    
    // Log to stdout with structured format
    match event_type {
        TransactionEventType::Started => {
            println!("[TX] → {} transaction started: {} (req: {}) from {} to {}", 
                protocol,
                command.unwrap_or("unknown"),
                request_id.unwrap_or("none"),
                from_peer,
                to_peer.unwrap_or("unknown"));
        }
        TransactionEventType::Completed => {
            let duration_str = duration_ms.map(|d| format!("{}ms", d)).unwrap_or_else(|| "unknown".to_string());
            let size_str = result_size.map(|s| format!("{} bytes", s)).unwrap_or_else(|| "unknown".to_string());
            println!("[TX] ✓ {} transaction completed: {} (req: {}) in {} (size: {})", 
                protocol,
                command.unwrap_or("unknown"),
                request_id.unwrap_or("none"),
                duration_str,
                size_str);
        }
        TransactionEventType::Failed => {
            eprintln!("[TX] ✗ {} transaction failed: {} (req: {}) - {}", 
                protocol,
                command.unwrap_or("unknown"),
                request_id.unwrap_or("none"),
                error.unwrap_or("unknown error"));
        }
        TransactionEventType::Timeout => {
            eprintln!("[TX] ⏱ {} transaction timeout: {} (req: {})", 
                protocol,
                command.unwrap_or("unknown"),
                request_id.unwrap_or("none"));
        }
    }
    
    // Could also write to file or send to logging service
}

/// Helper to log connection established
pub fn log_connection_established(peer_id: &str, direction: &str, protocol: &str, endpoint: Option<&str>) {
    log_connection(ConnectionEventType::Established, peer_id, direction, protocol, endpoint, None);
}

/// Helper to log connection closed
pub fn log_connection_closed(peer_id: &str, direction: &str, protocol: &str) {
    log_connection(ConnectionEventType::Closed, peer_id, direction, protocol, None, None);
}

/// Helper to log connection failed
pub fn log_connection_failed(peer_id: &str, direction: &str, protocol: &str, error: &str) {
    log_connection(ConnectionEventType::Failed, peer_id, direction, protocol, None, Some(error));
}

/// Helper to log transaction started
pub fn log_transaction_started(protocol: &str, command: &str, request_id: &str, from_peer: &str, to_peer: Option<&str>) {
    log_transaction(TransactionEventType::Started, protocol, Some(command), Some(request_id), from_peer, to_peer, None, None, None);
}

/// Helper to log transaction completed
pub fn log_transaction_completed(protocol: &str, command: &str, request_id: &str, from_peer: &str, to_peer: Option<&str>, duration_ms: u64, result_size: Option<usize>) {
    log_transaction(TransactionEventType::Completed, protocol, Some(command), Some(request_id), from_peer, to_peer, Some(duration_ms), None, result_size);
}

/// Helper to log transaction failed
pub fn log_transaction_failed(protocol: &str, command: &str, request_id: &str, from_peer: &str, to_peer: Option<&str>, error: &str) {
    log_transaction(TransactionEventType::Failed, protocol, Some(command), Some(request_id), from_peer, to_peer, None, Some(error), None);
}

/// Helper to log transaction timeout
pub fn log_transaction_timeout(protocol: &str, command: &str, request_id: &str, from_peer: &str, to_peer: Option<&str>) {
    log_transaction(TransactionEventType::Timeout, protocol, Some(command), Some(request_id), from_peer, to_peer, None, None, None);
}

