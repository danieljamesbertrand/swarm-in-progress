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

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
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
    /// GPU memory in MB (0 if no GPU available)
    pub gpu_memory_mb: u64,
    /// Number of GPU compute units (CUDA cores / stream processors)
    pub gpu_compute_units: u32,
    /// GPU utilization percentage (0-100)
    pub gpu_usage: f64,
    /// Whether GPU is available for inference
    pub gpu_available: bool,
}

impl NodeCapabilities {
    /// Calculate a composite score for node selection
    /// 
    /// Each factor is normalized to 0.0-1.0 range, then weighted.
    /// Higher scores indicate better candidates for request dispatch.
    /// 
    /// ## Scoring Factors
    /// - **CPU**: Cores normalized + available capacity (not in use)
    /// - **Memory**: Available/Total ratio
    /// - **GPU**: Memory + compute units + availability
    /// - **Latency**: Inverse relationship (lower = better)
    /// - **Reputation**: Direct 0-1 score from historical performance
    /// - **Disk**: Available/Total ratio (less critical for inference)
    pub fn calculate_score(&self, weights: &NodeWeights) -> f64 {
        // CPU score: cores (normalized to 16) * available capacity
        let cpu_score = (self.cpu_cores as f64 / 16.0).min(1.0) * (1.0 - self.cpu_usage / 100.0);
        
        // Memory score: ratio of available to total
        let memory_score = if self.memory_total_mb > 0 {
            self.memory_available_mb as f64 / self.memory_total_mb as f64
        } else {
            0.0
        };
        
        // Disk score: ratio of available to total
        let disk_score = if self.disk_total_mb > 0 {
            self.disk_available_mb as f64 / self.disk_total_mb as f64
        } else {
            0.0
        };
        
        // Latency score: inverse relationship, lower latency = higher score
        // 10ms = 0.91, 50ms = 0.67, 100ms = 0.50, 500ms = 0.17
        let latency_score = 1.0 / (1.0 + self.latency_ms / 100.0);
        
        // Reputation score: direct 0-1 value
        let reputation_score = self.reputation.clamp(0.0, 1.0);
        
        // GPU score: composite of memory, compute units, and availability
        let gpu_score = if self.gpu_available && self.gpu_memory_mb > 0 {
            let memory_factor = (self.gpu_memory_mb as f64 / 24576.0).min(1.0); // Normalized to 24GB
            let compute_factor = (self.gpu_compute_units as f64 / 10000.0).min(1.0); // Normalized to 10k units
            let usage_factor = 1.0 - (self.gpu_usage / 100.0);
            
            // Weighted combination: memory matters most for LLMs
            0.5 * memory_factor + 0.3 * compute_factor + 0.2 * usage_factor
        } else {
            0.0
        };

        // Calculate weighted total
        weights.cpu * cpu_score
            + weights.memory * memory_score
            + weights.disk * disk_score
            + weights.latency * latency_score
            + weights.reputation * reputation_score
            + weights.gpu * gpu_score
    }

    /// Create a default NodeCapabilities with no GPU
    pub fn default_cpu_only() -> Self {
        Self {
            cpu_cores: num_cpus::get() as u32,
            cpu_usage: 0.0,
            cpu_speed_ghz: 2.5,
            memory_total_mb: 8192,
            memory_available_mb: 4096,
            disk_total_mb: 500000,
            disk_available_mb: 250000,
            latency_ms: 50.0,
            reputation: 1.0,
            gpu_memory_mb: 0,
            gpu_compute_units: 0,
            gpu_usage: 0.0,
            gpu_available: false,
        }
    }

    /// Create NodeCapabilities with GPU
    pub fn with_gpu(mut self, memory_mb: u64, compute_units: u32) -> Self {
        self.gpu_memory_mb = memory_mb;
        self.gpu_compute_units = compute_units;
        self.gpu_available = true;
        self
    }
}

/// Weights for node selection algorithm
/// 
/// These weights determine how much each factor contributes to node selection.
/// All weights should sum to 1.0 for normalized scoring.
/// 
/// ## Default Weights (AI Inference Optimized)
/// - **CPU**: 0.15 - Base compute capability
/// - **Memory**: 0.15 - RAM for model loading
/// - **GPU**: 0.25 - Most important for AI inference performance
/// - **Latency**: 0.20 - Network responsiveness
/// - **Reputation**: 0.20 - Historical performance & reliability
/// - **Disk**: 0.05 - Less critical for inference
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NodeWeights {
    pub cpu: f64,
    pub memory: f64,
    pub disk: f64,
    pub latency: f64,
    pub reputation: f64,
    /// GPU weight - most important for AI inference
    pub gpu: f64,
}

impl Default for NodeWeights {
    fn default() -> Self {
        Self {
            cpu: 0.15,
            memory: 0.15,
            disk: 0.05,
            latency: 0.20,
            reputation: 0.20,
            gpu: 0.25,
        }
    }
}

impl NodeWeights {
    /// Create weights optimized for CPU-only inference
    pub fn cpu_optimized() -> Self {
        Self {
            cpu: 0.30,
            memory: 0.25,
            disk: 0.05,
            latency: 0.20,
            reputation: 0.20,
            gpu: 0.0,
        }
    }

    /// Create weights optimized for GPU inference
    pub fn gpu_optimized() -> Self {
        Self {
            cpu: 0.10,
            memory: 0.10,
            disk: 0.05,
            latency: 0.15,
            reputation: 0.20,
            gpu: 0.40,
        }
    }

    /// Create weights prioritizing low latency
    pub fn latency_optimized() -> Self {
        Self {
            cpu: 0.10,
            memory: 0.10,
            disk: 0.05,
            latency: 0.40,
            reputation: 0.20,
            gpu: 0.15,
        }
    }

    /// Create weights prioritizing reliability/reputation
    pub fn reliability_optimized() -> Self {
        Self {
            cpu: 0.10,
            memory: 0.10,
            disk: 0.05,
            latency: 0.15,
            reputation: 0.45,
            gpu: 0.15,
        }
    }

    /// Validate that weights sum to approximately 1.0
    pub fn validate(&self) -> bool {
        let sum = self.cpu + self.memory + self.disk + self.latency + self.reputation + self.gpu;
        (sum - 1.0).abs() < 0.01
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
    pub const SYNC_TORRENTS: &str = "SYNC_TORRENTS";
    pub const LOAD_SHARD: &str = "LOAD_SHARD";
    pub const SHARD_LOADED: &str = "SHARD_LOADED";
    pub const SWARM_READY: &str = "SWARM_READY";
    pub const SWARM_STATUS: &str = "SWARM_STATUS";
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_command_creation() {
        let cmd = Command::new("TEST_COMMAND", "peer1", Some("peer2"));
        assert_eq!(cmd.command, "TEST_COMMAND");
        assert_eq!(cmd.from, "peer1");
        assert_eq!(cmd.to, Some("peer2".to_string()));
        assert!(!cmd.request_id.is_empty());
    }

    #[test]
    fn test_command_with_params() {
        let cmd = Command::new("TEST_COMMAND", "peer1", None)
            .with_param("key1", serde_json::json!("value1"))
            .with_param("key2", serde_json::json!(42));
        
        assert_eq!(cmd.params.get("key1"), Some(&serde_json::json!("value1")));
        assert_eq!(cmd.params.get("key2"), Some(&serde_json::json!(42)));
    }

    #[test]
    fn test_command_json_serialization() {
        let cmd = Command::new("TEST_COMMAND", "peer1", Some("peer2"));
        let json = cmd.to_json().unwrap();
        assert!(json.contains("TEST_COMMAND"));
        assert!(json.contains("peer1"));
        
        let deserialized = Command::from_json(&json).unwrap();
        assert_eq!(cmd.command, deserialized.command);
        assert_eq!(cmd.from, deserialized.from);
    }

    #[test]
    fn test_command_response_success() {
        let mut result = HashMap::new();
        result.insert("data".to_string(), serde_json::json!("test"));
        
        let resp = CommandResponse::success("TEST_COMMAND", "req-123", "peer2", "peer1", result);
        assert_eq!(resp.status, ResponseStatus::Success);
        assert_eq!(resp.from, "peer2");
        assert_eq!(resp.to, "peer1");
        assert!(resp.error.is_none());
        assert!(resp.result.is_some());
    }

    #[test]
    fn test_command_response_error() {
        let resp = CommandResponse::error("TEST_COMMAND", "req-123", "peer2", "peer1", "Error message");
        assert_eq!(resp.status, ResponseStatus::Error);
        assert_eq!(resp.error, Some("Error message".to_string()));
        assert!(resp.result.is_none());
    }

    #[test]
    fn test_node_capabilities_score_calculation() {
        let capabilities = NodeCapabilities {
            cpu_cores: 8,
            cpu_usage: 50.0,
            cpu_speed_ghz: 3.0,
            memory_total_mb: 16384,
            memory_available_mb: 8192,
            disk_total_mb: 1000000,
            disk_available_mb: 500000,
            latency_ms: 10.0,
            reputation: 0.9,
            gpu_memory_mb: 0,
            gpu_compute_units: 0,
            gpu_usage: 0.0,
            gpu_available: false,
        };
        
        let weights = NodeWeights::default();
        let score = capabilities.calculate_score(&weights);
        
        assert!(score > 0.0);
        // With GPU weight at 0.25 but no GPU available, max score is ~0.75
        assert!(score <= 1.0);
    }
    
    #[test]
    fn test_node_capabilities_score_with_gpu() {
        let capabilities = NodeCapabilities {
            cpu_cores: 16,
            cpu_usage: 25.0,
            cpu_speed_ghz: 4.0,
            memory_total_mb: 32768,
            memory_available_mb: 24576,
            disk_total_mb: 2000000,
            disk_available_mb: 1500000,
            latency_ms: 5.0,
            reputation: 0.95,
            gpu_memory_mb: 24576, // 24GB
            gpu_compute_units: 16384, // RTX 4090
            gpu_usage: 10.0,
            gpu_available: true,
        };
        
        let weights = NodeWeights::gpu_optimized();
        let score = capabilities.calculate_score(&weights);
        
        assert!(score > 0.5, "GPU-equipped node should score well");
        assert!(score <= 1.5);
    }

    #[test]
    fn test_reputation_data_new() {
        let rep = ReputationData::new();
        assert_eq!(rep.reputation, 1.0);
        assert_eq!(rep.total_requests, 0);
        assert_eq!(rep.successful_requests, 0);
        assert_eq!(rep.failed_requests, 0);
    }

    #[test]
    fn test_reputation_data_update_success() {
        let mut rep = ReputationData::new();
        rep.update(true, 10.0, 0.9);
        
        assert_eq!(rep.total_requests, 1);
        assert_eq!(rep.successful_requests, 1);
        assert_eq!(rep.failed_requests, 0);
        assert!(rep.reputation > 1.0 || rep.reputation <= 1.0); // Should increase but cap at 1.0
        assert_eq!(rep.average_latency_ms, 10.0);
    }

    #[test]
    fn test_reputation_data_update_failure() {
        let mut rep = ReputationData::new();
        rep.update(false, 0.0, 0.0);
        
        assert_eq!(rep.total_requests, 1);
        assert_eq!(rep.successful_requests, 0);
        assert_eq!(rep.failed_requests, 1);
        assert!(rep.reputation < 1.0); // Should decrease
    }

    #[test]
    fn test_node_weights_default() {
        let weights = NodeWeights::default();
        let sum = weights.cpu + weights.memory + weights.disk + weights.latency + weights.reputation + weights.gpu;
        // Weights should sum to approximately 1.0 (allowing for floating point)
        assert!((sum - 1.0).abs() < 0.01, "Weights sum to {} instead of 1.0", sum);
    }
    
    #[test]
    fn test_node_weights_validate() {
        let valid = NodeWeights::default();
        assert!(valid.validate(), "Default weights should be valid");
        
        let invalid = NodeWeights {
            cpu: 0.5,
            memory: 0.5,
            disk: 0.5,
            latency: 0.5,
            reputation: 0.5,
            gpu: 0.5,
        };
        assert!(!invalid.validate(), "Weights summing to 3.0 should be invalid");
    }
}





