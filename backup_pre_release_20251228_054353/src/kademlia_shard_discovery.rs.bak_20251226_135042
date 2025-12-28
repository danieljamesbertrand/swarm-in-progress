//! Kademlia-based Shard Discovery for Distributed Llama Inference
//!
//! This module provides decentralized shard discovery using libp2p's Kademlia DHT.
//! Each node announces its shard information to the DHT, and clients can discover
//! all shards in a cluster to build the inference pipeline.
//!
//! ## Key Features
//! - Decentralized peer discovery via Kademlia DHT
//! - Automatic pipeline ordering by shard ID
//! - Capability-based node selection with weighted scoring
//! - Fault tolerance with multiple shard replicas
//! - NAT traversal support via libp2p relay
//!
//! ## Usage
//! ```rust,ignore
//! use punch_simple::kademlia_shard_discovery::{
//!     KademliaShardDiscovery, ShardAnnouncement, dht_keys
//! };
//!
//! // Create discovery instance
//! let mut discovery = KademliaShardDiscovery::new("llama-8b-cluster");
//!
//! // Announce this node's shard (requires swarm setup)
//! let record = discovery.create_announcement(0, shard_info);
//! swarm.behaviour_mut().kademlia.put_record(record, Quorum::One).unwrap();
//!
//! // Discover all shards
//! let pipeline = discovery.get_pipeline();
//! ```

use crate::command_protocol::NodeWeights;
use crate::shard_optimization::{QuantizationType, OptimizationPriority};
use libp2p::kad;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

/// Shard announcement stored in Kademlia DHT
/// Contains all information needed to identify and connect to a shard node
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct ShardAnnouncement {
    /// libp2p PeerId of the node hosting this shard
    pub peer_id: String,
    /// Unique shard identifier (0, 1, 2, ...)
    pub shard_id: u32,
    /// Starting layer index for this shard
    pub layer_start: u32,
    /// Ending layer index for this shard (exclusive)
    pub layer_end: u32,
    /// Total number of layers in this shard
    pub num_layers: u32,
    /// Whether this shard contains the embedding layer
    pub has_embeddings: bool,
    /// Whether this shard contains the output head
    pub has_output: bool,
    /// Multiaddress for connecting to this node
    pub multiaddr: String,
    /// Node capabilities for weighted selection
    pub capabilities: ShardCapabilities,
    /// Model name/identifier
    pub model_name: String,
    /// Total shards in the cluster
    pub total_shards: u32,
    /// Timestamp of this announcement (for freshness)
    pub timestamp: u64,
    /// Version of this announcement format
    pub version: u32,
    /// Quantization type used for this shard (affects size/speed/quality)
    pub quantization: QuantizationType,
    /// Model parameter count in billions (for memory estimation)
    pub model_params_billions: f32,
}

/// Capabilities specific to shard processing
/// Extends NodeCapabilities with shard-specific information
#[derive(Clone, Serialize, Deserialize, Debug, Default)]
pub struct ShardCapabilities {
    /// Number of CPU cores available
    pub cpu_cores: u32,
    /// Current CPU usage (0-100)
    pub cpu_usage: f64,
    /// Total memory in MB
    pub memory_total_mb: u64,
    /// Available memory in MB
    pub memory_available_mb: u64,
    /// GPU memory in MB (0 if no GPU)
    pub gpu_memory_mb: u64,
    /// Number of GPU compute units (CUDA cores / stream processors)
    pub gpu_compute_units: u32,
    /// GPU utilization percentage (0-100)
    pub gpu_usage: f64,
    /// Whether GPU is available for inference
    pub gpu_available: bool,
    /// Network latency to bootstrap node in ms
    pub latency_ms: f64,
    /// Node reputation score (0.0 - 1.0)
    pub reputation: f64,
    /// Whether the shard is currently loaded in memory
    pub shard_loaded: bool,
    /// Current number of active inference requests
    pub active_requests: u32,
    /// Maximum concurrent requests this node can handle
    pub max_concurrent: u32,
}

impl ShardCapabilities {
    /// Create new capabilities with system detection
    pub fn detect() -> Self {
        // Try to detect GPU from environment or nvidia-smi
        let (gpu_mem, gpu_compute, gpu_avail) = Self::detect_gpu();
        
        Self {
            cpu_cores: num_cpus::get() as u32,
            cpu_usage: 0.0, // Would need system monitoring
            memory_total_mb: 16384, // Would need sysinfo crate
            memory_available_mb: 8192,
            gpu_memory_mb: gpu_mem,
            gpu_compute_units: gpu_compute,
            gpu_usage: 0.0,
            gpu_available: gpu_avail,
            latency_ms: 0.0,
            reputation: 1.0,
            shard_loaded: false,
            active_requests: 0,
            max_concurrent: 4,
        }
    }

    /// Detect GPU information from environment or system
    fn detect_gpu() -> (u64, u32, bool) {
        // Check environment variables first
        if let Ok(gpu_mem) = std::env::var("NODE_GPU_MEMORY_MB") {
            if let Ok(mem) = gpu_mem.parse::<u64>() {
                let compute = std::env::var("NODE_GPU_COMPUTE_UNITS")
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(5000);
                return (mem, compute, true);
            }
        }
        
        // No GPU detected
        (0, 0, false)
    }

    /// Calculate a composite score for node selection
    /// 
    /// Factors considered:
    /// - CPU: cores and available capacity
    /// - Memory: available/total ratio
    /// - GPU: memory, compute units, availability
    /// - Latency: network responsiveness
    /// - Reputation: historical performance
    /// - Load: current request load vs capacity
    pub fn calculate_score(&self, weights: &NodeWeights) -> f64 {
        // CPU score: normalized cores * available capacity
        let cpu_score = (self.cpu_cores as f64 / 16.0).min(1.0) * (1.0 - self.cpu_usage / 100.0);
        
        // Memory score: available/total ratio
        let memory_score = self.memory_available_mb as f64 / self.memory_total_mb.max(1) as f64;
        
        // Load score: how much capacity is available
        let load_score = 1.0 - (self.active_requests as f64 / self.max_concurrent.max(1) as f64);
        
        // Latency score: inverse relationship
        let latency_score = 1.0 / (1.0 + self.latency_ms / 100.0);
        
        // Reputation score: direct 0-1 value
        let reputation_score = self.reputation.clamp(0.0, 1.0);
        
        // GPU score: composite of memory, compute, and availability
        let gpu_score = if self.gpu_available && self.gpu_memory_mb > 0 {
            let memory_factor = (self.gpu_memory_mb as f64 / 24576.0).min(1.0); // Normalized to 24GB
            let compute_factor = (self.gpu_compute_units as f64 / 10000.0).min(1.0); // Normalized to 10k units
            let usage_factor = 1.0 - (self.gpu_usage / 100.0);
            
            // Weighted: memory most important for LLMs
            0.5 * memory_factor + 0.3 * compute_factor + 0.2 * usage_factor
        } else {
            0.0
        };
        
        // Shard loaded bonus: prefer nodes with shard already in memory
        let shard_bonus = if self.shard_loaded { 0.1 } else { 0.0 };

        // Calculate weighted total
        let base_score = weights.cpu * cpu_score
            + weights.memory * memory_score
            + weights.latency * latency_score
            + weights.reputation * reputation_score
            + weights.gpu * gpu_score;
        
        // Add load balancing (10% weight) and shard bonus
        base_score + 0.10 * load_score + shard_bonus
    }

    /// Check if this node is a good candidate for AI inference
    pub fn is_inference_capable(&self) -> bool {
        // Minimum requirements for inference
        self.memory_available_mb >= 4096 && // At least 4GB RAM
        (self.gpu_available || self.cpu_cores >= 4) && // GPU or decent CPU
        self.active_requests < self.max_concurrent // Has capacity
    }
}

impl ShardAnnouncement {
    /// Create a new shard announcement
    pub fn new(
        peer_id: &str,
        shard_id: u32,
        total_shards: u32,
        total_layers: u32,
        multiaddr: &str,
        model_name: &str,
    ) -> Self {
        let layers_per_shard = total_layers / total_shards;
        let layer_start = shard_id * layers_per_shard;
        let layer_end = if shard_id == total_shards - 1 {
            total_layers
        } else {
            (shard_id + 1) * layers_per_shard
        };

        Self {
            peer_id: peer_id.to_string(),
            shard_id,
            layer_start,
            layer_end,
            num_layers: layer_end - layer_start,
            has_embeddings: shard_id == 0,
            has_output: shard_id == total_shards - 1,
            multiaddr: multiaddr.to_string(),
            capabilities: ShardCapabilities::detect(),
            model_name: model_name.to_string(),
            total_shards,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            version: 2, // Updated version for quantization support
            quantization: QuantizationType::default(), // Default to FP16
            model_params_billions: 7.0, // Default assumption (Llama 7B)
        }
    }
    
    /// Create with specific quantization settings
    pub fn with_quantization(mut self, quantization: QuantizationType, params_billions: f32) -> Self {
        self.quantization = quantization;
        self.model_params_billions = params_billions;
        self
    }

    /// Create from environment variables
    pub fn from_env(peer_id: &str, multiaddr: &str) -> Option<Self> {
        let shard_id: u32 = std::env::var("LLAMA_SHARD_ID").ok()?.parse().ok()?;
        let total_shards: u32 = std::env::var("LLAMA_TOTAL_SHARDS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(4);
        let total_layers: u32 = std::env::var("LLAMA_TOTAL_LAYERS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(32);
        let model_name = std::env::var("LLAMA_MODEL_NAME").unwrap_or_else(|_| "llama-8b".to_string());
        
        // Parse quantization from environment or model filename
        let quantization = std::env::var("LLAMA_QUANTIZATION")
            .ok()
            .and_then(|s| {
                // Try to parse from filename format
                QuantizationType::from_filename(&s)
            })
            .unwrap_or_default();
        
        // Parse model params (in billions)
        let model_params = std::env::var("LLAMA_MODEL_PARAMS_B")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(7.0);

        Some(Self::new(
            peer_id,
            shard_id,
            total_shards,
            total_layers,
            multiaddr,
            &model_name,
        ).with_quantization(quantization, model_params))
    }

    /// Check if this announcement is still fresh (less than TTL seconds old)
    pub fn is_fresh(&self, ttl_seconds: u64) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        now.saturating_sub(self.timestamp) < ttl_seconds
    }

    /// Serialize to bytes for DHT storage
    pub fn to_bytes(&self) -> Result<Vec<u8>, serde_json::Error> {
        serde_json::to_vec(self)
    }

    /// Deserialize from DHT record bytes
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, serde_json::Error> {
        serde_json::from_slice(bytes)
    }
}

/// DHT key patterns for shard discovery
pub mod dht_keys {
    /// Get the key for a specific cluster
    pub fn cluster_key(cluster_name: &str) -> String {
        format!("/llama-cluster/{}", cluster_name)
    }

    /// Get the key for a specific shard in a cluster
    pub fn shard_key(cluster_name: &str, shard_id: u32) -> String {
        format!("/llama-cluster/{}/shard/{}", cluster_name, shard_id)
    }

    /// Get the key for listing all shards in a cluster
    pub fn all_shards_key(cluster_name: &str) -> String {
        format!("/llama-cluster/{}/shards", cluster_name)
    }

    /// Get the key for cluster metadata
    pub fn metadata_key(cluster_name: &str) -> String {
        format!("/llama-cluster/{}/metadata", cluster_name)
    }

    /// Parse shard ID from a shard key
    pub fn parse_shard_id(key: &str) -> Option<u32> {
        key.rsplit('/').next()?.parse().ok()
    }
}

/// Cluster metadata stored in DHT
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct ClusterMetadata {
    pub cluster_name: String,
    pub model_name: String,
    pub total_shards: u32,
    pub total_layers: u32,
    pub created_at: u64,
    pub last_updated: u64,
}

/// Kademlia-based shard discoverer
/// Maintains a local cache of discovered shards and provides pipeline ordering
pub struct KademliaShardDiscovery {
    /// Cluster name for DHT key prefixing
    cluster_name: String,
    /// Known shards indexed by shard_id
    known_shards: HashMap<u32, Vec<ShardAnnouncement>>,
    /// Sorted pipeline order (shard IDs)
    pipeline_order: Vec<u32>,
    /// Expected total shards (from metadata or config)
    expected_shards: Option<u32>,
    /// Cluster metadata
    metadata: Option<ClusterMetadata>,
    /// Announcement TTL in seconds
    ttl_seconds: u64,
    /// Node selection weights
    weights: NodeWeights,
}

impl KademliaShardDiscovery {
    /// Create a new shard discovery instance
    pub fn new(cluster_name: &str) -> Self {
        Self {
            cluster_name: cluster_name.to_string(),
            known_shards: HashMap::new(),
            pipeline_order: Vec::new(),
            expected_shards: None,
            metadata: None,
            ttl_seconds: 300, // 5 minute TTL
            weights: NodeWeights::default(),
        }
    }

    /// Create with expected shard count
    pub fn with_expected_shards(cluster_name: &str, expected_shards: u32) -> Self {
        let mut discovery = Self::new(cluster_name);
        discovery.expected_shards = Some(expected_shards);
        discovery
    }

    /// Set custom node selection weights
    pub fn set_weights(&mut self, weights: NodeWeights) {
        self.weights = weights;
    }

    /// Set announcement TTL
    pub fn set_ttl(&mut self, ttl_seconds: u64) {
        self.ttl_seconds = ttl_seconds;
    }

    /// Get the cluster name
    pub fn cluster_name(&self) -> &str {
        &self.cluster_name
    }

    /// Create a Kademlia record for announcing a shard
    pub fn create_announcement_record(&self, announcement: &ShardAnnouncement) -> kad::Record {
        let key = kad::RecordKey::new(&dht_keys::shard_key(&self.cluster_name, announcement.shard_id));
        let value = announcement.to_bytes().unwrap_or_default();
        kad::Record::new(key, value)
    }

    /// Create Kademlia record key for querying a shard
    pub fn shard_record_key(&self, shard_id: u32) -> kad::RecordKey {
        kad::RecordKey::new(&dht_keys::shard_key(&self.cluster_name, shard_id))
    }

    /// Process a discovered shard record from Kademlia
    pub fn process_shard_record(&mut self, record: &kad::Record) -> Option<ShardAnnouncement> {
        let announcement = ShardAnnouncement::from_bytes(&record.value).ok()?;

        // Validate freshness
        if !announcement.is_fresh(self.ttl_seconds) {
            println!(
                "[DISCOVERY] Ignoring stale announcement for shard {} from {}",
                announcement.shard_id, announcement.peer_id
            );
            return None;
        }

        // Store in known shards (support multiple replicas per shard)
        let replicas = self.known_shards.entry(announcement.shard_id).or_insert_with(Vec::new);

        // Update or add this replica
        if let Some(existing) = replicas.iter_mut().find(|r| r.peer_id == announcement.peer_id) {
            *existing = announcement.clone();
        } else {
            replicas.push(announcement.clone());
        }

        // Update pipeline order
        self.rebuild_pipeline();

        // Update expected shards from announcement
        if self.expected_shards.is_none() {
            self.expected_shards = Some(announcement.total_shards);
        }

        println!(
            "[DISCOVERY] Processed shard {} from {} (layers {}-{})",
            announcement.shard_id, announcement.peer_id, announcement.layer_start, announcement.layer_end
        );

        Some(announcement)
    }

    /// Manually add a shard announcement
    pub fn add_shard(&mut self, announcement: ShardAnnouncement) {
        let replicas = self
            .known_shards
            .entry(announcement.shard_id)
            .or_insert_with(Vec::new);

        if let Some(existing) = replicas.iter_mut().find(|r| r.peer_id == announcement.peer_id) {
            *existing = announcement;
        } else {
            replicas.push(announcement);
        }

        self.rebuild_pipeline();
    }

    /// Remove stale announcements
    pub fn cleanup_stale(&mut self) {
        for replicas in self.known_shards.values_mut() {
            replicas.retain(|r| r.is_fresh(self.ttl_seconds));
        }

        // Remove empty shard entries
        self.known_shards.retain(|_, v| !v.is_empty());

        self.rebuild_pipeline();
    }

    /// Rebuild the sorted pipeline from discovered shards
    fn rebuild_pipeline(&mut self) {
        self.pipeline_order = self.known_shards.keys().cloned().collect();
        self.pipeline_order.sort();
    }

    /// Get the complete sorted pipeline with best node per shard
    pub fn get_pipeline(&self) -> Vec<&ShardAnnouncement> {
        self.pipeline_order
            .iter()
            .filter_map(|id| self.get_best_node_for_shard(*id))
            .collect()
    }

    /// Get all replicas for a specific shard
    pub fn get_shard_replicas(&self, shard_id: u32) -> Option<&Vec<ShardAnnouncement>> {
        self.known_shards.get(&shard_id)
    }

    /// Get the best node for a specific shard based on capabilities
    pub fn get_best_node_for_shard(&self, shard_id: u32) -> Option<&ShardAnnouncement> {
        self.get_best_node_for_shard_with_priority(shard_id, OptimizationPriority::Balanced)
    }
    
    /// Get the best node for a specific shard based on optimization priority
    /// 
    /// # Arguments
    /// * `shard_id` - The shard to find
    /// * `priority` - Optimization priority (Speed, Quality, Balanced, Memory)
    /// 
    /// # Returns
    /// The best node announcement based on the priority, or None if no nodes available
    pub fn get_best_node_for_shard_with_priority(
        &self,
        shard_id: u32,
        priority: OptimizationPriority,
    ) -> Option<&ShardAnnouncement> {
        self.known_shards.get(&shard_id).and_then(|replicas| {
            replicas
                .iter()
                .filter(|r| r.is_fresh(self.ttl_seconds))
                .max_by(|a, b| {
                    let score_a = self.calculate_priority_score(a, priority);
                    let score_b = self.calculate_priority_score(b, priority);
                    score_a.partial_cmp(&score_b).unwrap_or(std::cmp::Ordering::Equal)
                })
        })
    }
    
    /// Calculate a composite score for a shard announcement based on priority
    fn calculate_priority_score(&self, announcement: &ShardAnnouncement, priority: OptimizationPriority) -> f64 {
        let base_score = announcement.capabilities.calculate_score(&self.weights);
        let quant = &announcement.quantization;
        
        match priority {
            OptimizationPriority::Speed => {
                // Prioritize speed factor: faster quantization = higher score
                let speed_bonus = quant.speed_factor() as f64 / 10.0; // Normalize to ~0-1.5
                base_score * 0.5 + speed_bonus * 0.5
            }
            OptimizationPriority::Quality => {
                // Prioritize quality factor: higher quality quantization = higher score
                let quality_bonus = quant.quality_factor() as f64;
                base_score * 0.5 + quality_bonus * 0.5
            }
            OptimizationPriority::Balanced => {
                // Balance between base capabilities, speed, and quality
                let speed_factor = quant.speed_factor() as f64 / 10.0;
                let quality_factor = quant.quality_factor() as f64;
                base_score * 0.4 + speed_factor * 0.3 + quality_factor * 0.3
            }
            OptimizationPriority::Memory => {
                // Prioritize low memory usage
                let memory_factor = 1.0 - quant.size_factor() as f64; // Lower size = higher score
                let available_memory = announcement.capabilities.memory_available_mb as f64
                    / announcement.capabilities.memory_total_mb.max(1) as f64;
                base_score * 0.3 + memory_factor * 0.4 + available_memory * 0.3
            }
        }
    }
    
    /// Get the complete pipeline optimized for a specific priority
    pub fn get_pipeline_with_priority(&self, priority: OptimizationPriority) -> Vec<&ShardAnnouncement> {
        self.pipeline_order
            .iter()
            .filter_map(|id| self.get_best_node_for_shard_with_priority(*id, priority))
            .collect()
    }

    /// Get entry node (shard 0 with embeddings)
    pub fn entry_node(&self) -> Option<&ShardAnnouncement> {
        self.get_best_node_for_shard(0).filter(|s| s.has_embeddings)
    }

    /// Get exit node (last shard with output head)
    pub fn exit_node(&self) -> Option<&ShardAnnouncement> {
        if let Some(expected) = self.expected_shards {
            self.get_best_node_for_shard(expected - 1)
                .filter(|s| s.has_output)
        } else {
            // Find highest shard with output
            self.known_shards
                .values()
                .flatten()
                .filter(|s| s.has_output && s.is_fresh(self.ttl_seconds))
                .max_by_key(|s| s.shard_id)
        }
    }

    /// Get next shard in pipeline after current
    pub fn next_shard(&self, current_shard_id: u32) -> Option<&ShardAnnouncement> {
        self.get_best_node_for_shard(current_shard_id + 1)
    }

    /// Get previous shard in pipeline
    pub fn previous_shard(&self, current_shard_id: u32) -> Option<&ShardAnnouncement> {
        if current_shard_id > 0 {
            self.get_best_node_for_shard(current_shard_id - 1)
        } else {
            None
        }
    }

    /// Check if the pipeline is complete (all shards discovered)
    pub fn is_pipeline_complete(&self) -> bool {
        let Some(expected) = self.expected_shards else {
            return false;
        };

        if self.known_shards.len() < expected as usize {
            return false;
        }

        // Check we have all shards 0 to N-1 with at least one fresh replica
        for i in 0..expected {
            if self.get_best_node_for_shard(i).is_none() {
                return false;
            }
        }

        // Verify entry and exit nodes exist
        self.entry_node().is_some() && self.exit_node().is_some()
    }

    /// Get missing shard IDs
    pub fn get_missing_shards(&self) -> Vec<u32> {
        let Some(expected) = self.expected_shards else {
            return vec![];
        };

        (0..expected)
            .filter(|id| self.get_best_node_for_shard(*id).is_none())
            .collect()
    }

    /// Get pipeline status summary
    pub fn status(&self) -> PipelineStatus {
        let discovered = self.known_shards.len() as u32;
        let expected = self.expected_shards.unwrap_or(discovered);
        let total_replicas: usize = self.known_shards.values().map(|v| v.len()).sum();

        PipelineStatus {
            cluster_name: self.cluster_name.clone(),
            discovered_shards: discovered,
            expected_shards: expected,
            total_replicas: total_replicas as u32,
            is_complete: self.is_pipeline_complete(),
            has_entry: self.entry_node().is_some(),
            has_exit: self.exit_node().is_some(),
            missing_shards: self.get_missing_shards(),
        }
    }

    /// Get number of discovered shards
    pub fn shard_count(&self) -> usize {
        self.known_shards.len()
    }

    /// Get total replica count across all shards
    pub fn replica_count(&self) -> usize {
        self.known_shards.values().map(|v| v.len()).sum()
    }
}

/// Pipeline status summary
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PipelineStatus {
    pub cluster_name: String,
    pub discovered_shards: u32,
    pub expected_shards: u32,
    pub total_replicas: u32,
    pub is_complete: bool,
    pub has_entry: bool,
    pub has_exit: bool,
    pub missing_shards: Vec<u32>,
}

impl std::fmt::Display for PipelineStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Pipeline '{}': {}/{} shards, {} replicas, complete: {}, entry: {}, exit: {}",
            self.cluster_name,
            self.discovered_shards,
            self.expected_shards,
            self.total_replicas,
            self.is_complete,
            self.has_entry,
            self.has_exit
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shard_announcement_creation() {
        let announcement = ShardAnnouncement::new(
            "12D3KooWTest",
            0,
            4,
            32,
            "/ip4/192.168.1.100/tcp/51820",
            "llama-8b",
        );

        assert_eq!(announcement.shard_id, 0);
        assert_eq!(announcement.layer_start, 0);
        assert_eq!(announcement.layer_end, 8);
        assert!(announcement.has_embeddings);
        assert!(!announcement.has_output);
        assert_eq!(announcement.total_shards, 4);
    }

    #[test]
    fn test_shard_announcement_last_shard() {
        let announcement = ShardAnnouncement::new(
            "12D3KooWTest",
            3,
            4,
            32,
            "/ip4/192.168.1.103/tcp/51820",
            "llama-8b",
        );

        assert_eq!(announcement.shard_id, 3);
        assert_eq!(announcement.layer_start, 24);
        assert_eq!(announcement.layer_end, 32);
        assert!(!announcement.has_embeddings);
        assert!(announcement.has_output);
    }

    #[test]
    fn test_shard_announcement_serialization() {
        let announcement = ShardAnnouncement::new(
            "12D3KooWTest",
            1,
            4,
            32,
            "/ip4/192.168.1.101/tcp/51820",
            "llama-8b",
        );

        let bytes = announcement.to_bytes().unwrap();
        let deserialized = ShardAnnouncement::from_bytes(&bytes).unwrap();

        assert_eq!(deserialized.shard_id, announcement.shard_id);
        assert_eq!(deserialized.peer_id, announcement.peer_id);
        assert_eq!(deserialized.layer_start, announcement.layer_start);
    }

    #[test]
    fn test_dht_keys() {
        assert_eq!(
            dht_keys::cluster_key("llama-8b"),
            "/llama-cluster/llama-8b"
        );
        assert_eq!(
            dht_keys::shard_key("llama-8b", 2),
            "/llama-cluster/llama-8b/shard/2"
        );
        assert_eq!(dht_keys::parse_shard_id("/llama-cluster/test/shard/3"), Some(3));
    }

    #[test]
    fn test_discovery_pipeline_building() {
        let mut discovery = KademliaShardDiscovery::with_expected_shards("test-cluster", 4);

        // Add shards out of order
        discovery.add_shard(ShardAnnouncement::new(
            "peer2", 2, 4, 32, "/ip4/10.0.0.2/tcp/51820", "llama",
        ));
        discovery.add_shard(ShardAnnouncement::new(
            "peer0", 0, 4, 32, "/ip4/10.0.0.0/tcp/51820", "llama",
        ));
        discovery.add_shard(ShardAnnouncement::new(
            "peer3", 3, 4, 32, "/ip4/10.0.0.3/tcp/51820", "llama",
        ));
        discovery.add_shard(ShardAnnouncement::new(
            "peer1", 1, 4, 32, "/ip4/10.0.0.1/tcp/51820", "llama",
        ));

        // Pipeline should be sorted
        let pipeline = discovery.get_pipeline();
        assert_eq!(pipeline.len(), 4);
        assert_eq!(pipeline[0].shard_id, 0);
        assert_eq!(pipeline[1].shard_id, 1);
        assert_eq!(pipeline[2].shard_id, 2);
        assert_eq!(pipeline[3].shard_id, 3);

        // Check entry/exit
        assert!(discovery.entry_node().unwrap().has_embeddings);
        assert!(discovery.exit_node().unwrap().has_output);

        // Check completeness
        assert!(discovery.is_pipeline_complete());
    }

    #[test]
    fn test_discovery_incomplete_pipeline() {
        let mut discovery = KademliaShardDiscovery::with_expected_shards("test-cluster", 4);

        discovery.add_shard(ShardAnnouncement::new(
            "peer0", 0, 4, 32, "/ip4/10.0.0.0/tcp/51820", "llama",
        ));
        discovery.add_shard(ShardAnnouncement::new(
            "peer2", 2, 4, 32, "/ip4/10.0.0.2/tcp/51820", "llama",
        ));

        assert!(!discovery.is_pipeline_complete());
        assert_eq!(discovery.get_missing_shards(), vec![1, 3]);
    }

    #[test]
    fn test_discovery_multiple_replicas() {
        let mut discovery = KademliaShardDiscovery::new("test-cluster");

        // Add two replicas for shard 0
        let mut replica1 = ShardAnnouncement::new(
            "peer0a", 0, 4, 32, "/ip4/10.0.0.1/tcp/51820", "llama",
        );
        replica1.capabilities.cpu_cores = 8;
        replica1.capabilities.reputation = 0.9;

        let mut replica2 = ShardAnnouncement::new(
            "peer0b", 0, 4, 32, "/ip4/10.0.0.2/tcp/51820", "llama",
        );
        replica2.capabilities.cpu_cores = 16;
        replica2.capabilities.reputation = 0.95;

        discovery.add_shard(replica1);
        discovery.add_shard(replica2);

        assert_eq!(discovery.shard_count(), 1);
        assert_eq!(discovery.replica_count(), 2);

        // Best node should be replica2 (better capabilities)
        let best = discovery.get_best_node_for_shard(0).unwrap();
        assert_eq!(best.peer_id, "peer0b");
    }

    #[test]
    fn test_next_shard() {
        let mut discovery = KademliaShardDiscovery::new("test-cluster");

        for i in 0..4 {
            discovery.add_shard(ShardAnnouncement::new(
                &format!("peer{}", i),
                i,
                4,
                32,
                &format!("/ip4/10.0.0.{}/tcp/51820", i),
                "llama",
            ));
        }

        assert_eq!(discovery.next_shard(0).unwrap().shard_id, 1);
        assert_eq!(discovery.next_shard(1).unwrap().shard_id, 2);
        assert_eq!(discovery.next_shard(2).unwrap().shard_id, 3);
        assert!(discovery.next_shard(3).is_none());
    }

    #[test]
    fn test_pipeline_status() {
        let mut discovery = KademliaShardDiscovery::with_expected_shards("test-cluster", 4);

        discovery.add_shard(ShardAnnouncement::new(
            "peer0", 0, 4, 32, "/ip4/10.0.0.0/tcp/51820", "llama",
        ));

        let status = discovery.status();
        assert_eq!(status.discovered_shards, 1);
        assert_eq!(status.expected_shards, 4);
        assert!(!status.is_complete);
        assert!(status.has_entry);
        assert!(!status.has_exit);
        assert_eq!(status.missing_shards, vec![1, 2, 3]);
    }

    #[test]
    fn test_capabilities_score() {
        let mut caps = ShardCapabilities::default();
        caps.cpu_cores = 16;
        caps.memory_total_mb = 32768;
        caps.memory_available_mb = 16384;
        caps.reputation = 0.95;
        caps.latency_ms = 10.0;

        let weights = NodeWeights::default();
        let score = caps.calculate_score(&weights);

        assert!(score > 0.0);
        assert!(score <= 1.5); // With load score bonus
    }
}

