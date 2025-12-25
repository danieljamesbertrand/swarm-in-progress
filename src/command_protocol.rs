//! JSON Command Protocol - Standardized inter-node communication
//! All nodes communicate via JSON commands with routing and capability-based selection

use serde::{Serialize, Deserialize};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

/// Standard command structure for all inter-node communication
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Command {
    pub command: String,
    pub request_id: String,
    pub from: String,  // PeerId of requester
    pub to: Option<String>,  // PeerId of target (None = find best node)
    pub timestamp: u64,
    pub params: HashMap<String, serde_json::Value>,
}

/// Standard response structure
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct CommandResponse {
    pub command: String,
    pub request_id: String,
    pub from: String,  // PeerId of executor
    pub to: String,    // PeerId of requester
    pub timestamp: u64,
    pub status: ResponseStatus,
    pub result: Option<HashMap<String, serde_json::Value>>,
    pub error: Option<String>,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
#[serde(rename_all = "lowercase")]
pub enum ResponseStatus {
    Success,
    Error,
    Timeout,
}

impl Command {
    pub fn new(command: &str, from: &str, to: Option<&str>) -> Self {
        Self {
            command: command.to_string(),
            request_id: format!("req-{}", SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()),
            from: from.to_string(),
            to: to.map(|s| s.to_string()),
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            params: HashMap::new(),
        }
    }

    pub fn with_param(mut self, key: &str, value: serde_json::Value) -> Self {
        self.params.insert(key.to_string(), value);
        self
    }

    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
}

impl CommandResponse {
    pub fn success(
        command: &str,
        request_id: &str,
        from: &str,
        to: &str,
        result: HashMap<String, serde_json::Value>,
    ) -> Self {
        Self {
            command: command.to_string(),
            request_id: request_id.to_string(),
            from: from.to_string(),
            to: to.to_string(),
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            status: ResponseStatus::Success,
            result: Some(result),
            error: None,
        }
    }

    pub fn error(
        command: &str,
        request_id: &str,
        from: &str,
        to: &str,
        error_msg: &str,
    ) -> Self {
        Self {
            command: command.to_string(),
            request_id: request_id.to_string(),
            from: from.to_string(),
            to: to.to_string(),
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            status: ResponseStatus::Error,
            result: None,
            error: Some(error_msg.to_string()),
        }
    }

    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
}

/// Node capabilities for weighted selection
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct NodeCapabilities {
    pub cpu_cores: u32,
    pub cpu_usage: f64,  // 0-100
    pub cpu_speed_ghz: f64,
    pub memory_total_mb: u64,
    pub memory_available_mb: u64,
    pub disk_total_mb: u64,
    pub disk_available_mb: u64,
    pub latency_ms: f64,
    pub reputation: f64,  // 0.0-1.0
}

impl NodeCapabilities {
    pub fn calculate_score(&self, weights: &NodeWeights) -> f64 {
        let cpu_score = (self.cpu_cores as f64 / 16.0).min(1.0) * (1.0 - self.cpu_usage / 100.0);
        let memory_score = self.memory_available_mb as f64 / self.memory_total_mb as f64;
        let disk_score = self.disk_available_mb as f64 / self.disk_total_mb as f64;
        let latency_score = 1.0 / (1.0 + self.latency_ms / 100.0);
        let reputation_score = self.reputation;

        weights.cpu * cpu_score +
        weights.memory * memory_score +
        weights.disk * disk_score +
        weights.latency * latency_score +
        weights.reputation * reputation_score
    }
}

/// Weights for node selection algorithm
#[derive(Clone, Debug)]
pub struct NodeWeights {
    pub cpu: f64,
    pub memory: f64,
    pub disk: f64,
    pub latency: f64,
    pub reputation: f64,
}

impl Default for NodeWeights {
    fn default() -> Self {
        Self {
            cpu: 0.20,
            memory: 0.15,
            disk: 0.15,
            latency: 0.25,
            reputation: 0.25,
        }
    }
}

/// Reputation tracking
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct ReputationData {
    pub reputation: f64,  // 0.0-1.0
    pub total_requests: u64,
    pub successful_requests: u64,
    pub failed_requests: u64,
    pub average_latency_ms: f64,
    pub last_updated: u64,
}

impl ReputationData {
    pub fn new() -> Self {
        Self {
            reputation: 1.0,
            total_requests: 0,
            successful_requests: 0,
            failed_requests: 0,
            average_latency_ms: 0.0,
            last_updated: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        }
    }

    pub fn update(&mut self, success: bool, latency_ms: f64, quality_score: f64) {
        self.total_requests += 1;
        
        if success {
            self.successful_requests += 1;
            // Update average latency
            let total_latency = self.average_latency_ms * (self.successful_requests - 1) as f64 + latency_ms;
            self.average_latency_ms = total_latency / self.successful_requests as f64;
            
            // Increase reputation
            let increase = 0.01 + (quality_score * 0.02);
            self.reputation = (self.reputation + increase).min(1.0);
        } else {
            self.failed_requests += 1;
            // Decrease reputation
            self.reputation = (self.reputation - 0.05).max(0.0);
        }
        
        self.last_updated = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
    }
}

/// Standard command names
pub mod commands {
    pub const GET_CAPABILITIES: &str = "GET_CAPABILITIES";
    pub const EXECUTE_TASK: &str = "EXECUTE_TASK";
    pub const GET_REPUTATION: &str = "GET_REPUTATION";
    pub const UPDATE_REPUTATION: &str = "UPDATE_REPUTATION";
    pub const FIND_NODES: &str = "FIND_NODES";
    pub const LIST_FILES: &str = "LIST_FILES";
    pub const GET_FILE_METADATA: &str = "GET_FILE_METADATA";
    pub const REQUEST_PIECE: &str = "REQUEST_PIECE";
}

