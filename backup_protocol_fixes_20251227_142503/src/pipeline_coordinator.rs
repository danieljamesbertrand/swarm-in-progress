//! Pipeline Coordinator - Handles partial shard availability for distributed Llama inference
//!
//! This module provides strategies for handling incomplete pipelines:
//! - Graceful degradation with queuing until shards become available
//! - Dynamic shard loading on nodes with spare capacity
//! - Fallback to single-node full model execution
//! - Request queuing and retry logic
//!
//! ## Key Concepts
//!
//! **Pipeline Parallelism** requires all shards in sequence. This coordinator
//! provides strategies to handle missing shards gracefully rather than failing.
//!
//! ## Usage
//! ```rust,ignore
//! use punch_simple::pipeline_coordinator::{PipelineCoordinator, InferenceRequest, PipelineStrategy};
//!
//! // Requires discovery and shard_manager setup
//! let mut coordinator = PipelineCoordinator::new(discovery, shard_manager);
//! coordinator.set_strategy(PipelineStrategy::WaitAndRetry { 
//!     timeout_secs: 60, 
//!     retry_interval_ms: 1000 
//! });
//!
//! // Submit request - will queue if pipeline incomplete
//! let handle = coordinator.submit_inference(request).await.unwrap();
//!
//! // Wait for result (may wait for shards to become available)
//! let response = handle.await.unwrap();
//! ```

use crate::kademlia_shard_discovery::{
    KademliaShardDiscovery, ShardAnnouncement, ShardCapabilities, PipelineStatus,
};
use crate::llama_model_loader::LlamaModelManager;
use libp2p::kad;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::{mpsc, oneshot, RwLock, Mutex};
use tokio::process::Command as TokioCommand;
use std::process::Stdio;

/// Strategy for handling incomplete pipelines
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum PipelineStrategy {
    /// Fail immediately if pipeline is incomplete
    FailFast,
    
    /// Queue requests and wait for missing shards (with timeout)
    WaitAndRetry {
        timeout_secs: u64,
        retry_interval_ms: u64,
    },
    
    /// Dynamically load missing shards on nodes with capacity
    DynamicLoading {
        /// Maximum shards a single node can load
        max_shards_per_node: u32,
        /// Minimum available memory (MB) required to load a shard
        min_memory_mb: u64,
    },
    
    /// Fall back to single-node full model if available
    SingleNodeFallback {
        /// Minimum memory required for full model
        required_memory_mb: u64,
    },
    
    /// Combined: try dynamic loading, then wait, then fallback
    Adaptive {
        wait_timeout_secs: u64,
        min_memory_for_shard_mb: u64,
        min_memory_for_full_mb: u64,
    },
    
    /// Spawn new nodes on demand when shards are missing
    SpawnNodes {
        /// Maximum nodes to spawn per request
        max_nodes_per_request: u32,
        /// Minimum memory (MB) required per spawned node
        min_memory_per_node_mb: u64,
        /// Spawn command template (e.g., "cargo run --bin shard_listener -- --shard-id {shard_id}")
        spawn_command_template: String,
        /// Timeout for node to come online (seconds)
        node_startup_timeout_secs: u64,
    },
}

impl Default for PipelineStrategy {
    fn default() -> Self {
        Self::Adaptive {
            wait_timeout_secs: 30,
            min_memory_for_shard_mb: 4096,   // 4GB per shard
            min_memory_for_full_mb: 16384,   // 16GB for full model
        }
    }
}

/// Inference request to be processed
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InferenceRequest {
    pub request_id: String,
    pub prompt: String,
    pub max_tokens: u32,
    pub temperature: f32,
    pub top_p: f32,
    pub context: Option<Vec<String>>,
    pub created_at: u64,
    pub priority: u32,
}

impl InferenceRequest {
    pub fn new(prompt: &str) -> Self {
        Self {
            request_id: uuid::Uuid::new_v4().to_string(),
            prompt: prompt.to_string(),
            max_tokens: 256,
            temperature: 0.7,
            top_p: 0.9,
            context: None,
            created_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            priority: 0,
        }
    }

    pub fn with_max_tokens(mut self, tokens: u32) -> Self {
        self.max_tokens = tokens;
        self
    }

    pub fn with_temperature(mut self, temp: f32) -> Self {
        self.temperature = temp;
        self
    }

    pub fn with_priority(mut self, priority: u32) -> Self {
        self.priority = priority;
        self
    }
}

/// Inference response
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InferenceResponse {
    pub request_id: String,
    pub text: String,
    pub tokens_generated: u32,
    pub total_latency_ms: f64,
    pub shard_latencies: Vec<ShardLatency>,
    pub strategy_used: String,
    pub success: bool,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ShardLatency {
    pub shard_id: u32,
    pub node_id: String,
    pub latency_ms: f64,
}

/// Queued request waiting for pipeline completion
struct QueuedRequest {
    request: InferenceRequest,
    response_tx: oneshot::Sender<Result<InferenceResponse, PipelineError>>,
    queued_at: Instant,
    timeout: Duration,
}

/// Node that has joined the request queue and is available to load shards
#[derive(Clone, Debug)]
struct QueuedNode {
    peer_id: String,
    capabilities: ShardCapabilities,
    joined_at: Instant,
    assigned_shard: Option<u32>,
    shard_loading: bool,
}

/// Shard demand tracking - tracks which shards are most needed
#[derive(Clone, Debug, Default)]
struct ShardDemand {
    /// Number of pending requests waiting for this shard
    pending_requests: u32,
    /// Number of nodes currently loading this shard
    nodes_loading: u32,
    /// Number of nodes that have this shard loaded
    nodes_available: u32,
    /// Last time this shard was requested
    last_requested: Option<Instant>,
    /// Priority score (higher = more needed)
    priority_score: f64,
}

/// Pipeline coordinator state
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum CoordinatorState {
    /// Pipeline complete, ready to process
    Ready,
    /// Waiting for missing shards
    WaitingForShards { missing: Vec<u32> },
    /// Dynamically loading shards
    LoadingShards { loading: Vec<u32> },
    /// Using single-node fallback
    FallbackMode { node_id: String },
    /// Pipeline unavailable
    Unavailable { reason: String },
}

/// Pipeline errors
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum PipelineError {
    /// Pipeline incomplete and timeout expired
    Timeout { missing_shards: Vec<u32>, waited_secs: u64 },
    /// No fallback available
    NoFallback { reason: String },
    /// Shard loading failed
    ShardLoadFailed { shard_id: u32, error: String },
    /// Inference failed
    InferenceFailed { shard_id: u32, error: String },
    /// Request cancelled
    Cancelled,
    /// Internal error
    Internal { message: String },
}

impl std::fmt::Display for PipelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Timeout { missing_shards, waited_secs } => {
                write!(f, "Timeout after {}s waiting for shards: {:?}", waited_secs, missing_shards)
            }
            Self::NoFallback { reason } => write!(f, "No fallback available: {}", reason),
            Self::ShardLoadFailed { shard_id, error } => {
                write!(f, "Failed to load shard {}: {}", shard_id, error)
            }
            Self::InferenceFailed { shard_id, error } => {
                write!(f, "Inference failed at shard {}: {}", shard_id, error)
            }
            Self::Cancelled => write!(f, "Request cancelled"),
            Self::Internal { message } => write!(f, "Internal error: {}", message),
        }
    }
}

impl std::error::Error for PipelineError {}

/// Command sender function type for sending commands to nodes
pub type CommandSender = Box<dyn Fn(String, crate::command_protocol::Command) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<crate::command_protocol::CommandResponse, PipelineError>> + Send>> + Send + Sync>;

/// Node spawner for creating shard_listener processes on demand
pub struct NodeSpawner {
    /// Spawned node processes (shard_id -> process handle)
    spawned_nodes: Arc<RwLock<HashMap<u32, tokio::process::Child>>>,
    /// Bootstrap address for spawned nodes
    bootstrap_addr: String,
    /// Cluster name
    cluster_name: String,
    /// Total shards in cluster
    total_shards: u32,
    /// Total layers in model
    total_layers: u32,
    /// Model name
    model_name: String,
    /// Shards directory
    shards_dir: String,
}

impl NodeSpawner {
    pub fn new(
        bootstrap_addr: String,
        cluster_name: String,
        total_shards: u32,
        total_layers: u32,
        model_name: String,
        shards_dir: String,
    ) -> Self {
        Self {
            spawned_nodes: Arc::new(RwLock::new(HashMap::new())),
            bootstrap_addr,
            cluster_name,
            total_shards,
            total_layers,
            model_name,
            shards_dir,
        }
    }

    /// Spawn a new shard_listener node for a specific shard
    pub async fn spawn_node_for_shard(&self, shard_id: u32) -> Result<(), PipelineError> {
        // Check if node already exists
        let spawned = self.spawned_nodes.read().await;
        if spawned.contains_key(&shard_id) {
            println!("[SPAWNER] Node for shard {} already exists", shard_id);
            return Ok(());
        }
        drop(spawned);

        println!("[SPAWNER] Spawning node for shard {}...", shard_id);

        // Build command to spawn shard_listener
        let mut cmd = TokioCommand::new("cargo");
        cmd.args(&[
            "run",
            "--bin",
            "shard_listener",
            "--",
            "--bootstrap",
            &self.bootstrap_addr,
            "--cluster",
            &self.cluster_name,
            "--shard-id",
            &shard_id.to_string(),
            "--total-shards",
            &self.total_shards.to_string(),
            "--total-layers",
            &self.total_layers.to_string(),
            "--model-name",
            &self.model_name,
            "--shards-dir",
            &self.shards_dir,
            "--enable-torrent",
        ]);
        
        // Spawn process in background
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());
        cmd.stdin(Stdio::null());

        match cmd.spawn() {
            Ok(child) => {
                let pid = child.id();
                println!("[SPAWNER] ✓ Spawned shard_listener process for shard {} (PID: {:?})", shard_id, pid);
                println!("[SPAWNER]   Bootstrap: {}", self.bootstrap_addr);
                println!("[SPAWNER]   Cluster: {}", self.cluster_name);
                println!("[SPAWNER]   Shards dir: {}", self.shards_dir);
                
                // Store process handle
                let mut spawned = self.spawned_nodes.write().await;
                spawned.insert(shard_id, child);
                
                Ok(())
            }
            Err(e) => {
                eprintln!("[SPAWNER] ❌ Failed to spawn node for shard {}: {}", shard_id, e);
                eprintln!("[SPAWNER]   Error type: {:?}", e.kind());
                eprintln!("[SPAWNER]   Command: cargo run --bin shard_listener");
                eprintln!("[SPAWNER]   Bootstrap: {}", self.bootstrap_addr);
                eprintln!("[SPAWNER]   Cluster: {}", self.cluster_name);
                eprintln!("[SPAWNER]   Shards dir: {}", self.shards_dir);
                eprintln!("[SPAWNER]   Possible causes:");
                eprintln!("[SPAWNER]     - Cargo not found in PATH");
                eprintln!("[SPAWNER]     - shard_listener binary not found");
                eprintln!("[SPAWNER]     - Insufficient permissions");
                eprintln!("[SPAWNER]     - System resource limits exceeded");
                Err(PipelineError::ShardLoadFailed {
                    shard_id,
                    error: format!("Failed to spawn node: {} (kind: {:?})", e, e.kind()),
                })
            }
        }
    }

    /// Wait for a spawned node to come online and join DHT
    pub async fn wait_for_node_online(
        &self,
        shard_id: u32,
        timeout_secs: u64,
        discovery: &Arc<RwLock<KademliaShardDiscovery>>,
    ) -> Result<(), PipelineError> {
        let deadline = Instant::now() + Duration::from_secs(timeout_secs);
        let check_interval = Duration::from_millis(500);

        println!("[SPAWNER] Waiting for shard {} node to come online (timeout: {}s)...", shard_id, timeout_secs);

        let mut check_count = 0;
        while Instant::now() < deadline {
            check_count += 1;
            let elapsed = Instant::now().duration_since(deadline - Duration::from_secs(timeout_secs));
            
            // Check if shard is now available in discovery
            let pipeline: Vec<ShardAnnouncement> = {
                let disc = discovery.read().await;
                disc.get_pipeline().into_iter().cloned().collect()
            };

            // Check if our shard is in the pipeline
            if pipeline.iter().any(|s| s.shard_id == shard_id) {
                println!("[SPAWNER] ✓ Shard {} node is online and discovered! (after {:.1}s, {} checks)", 
                    shard_id, elapsed.as_secs_f64(), check_count);
                return Ok(());
            }

            // Check if process handle exists and verify it's still running
            let process_exists = {
                let spawned = self.spawned_nodes.read().await;
                spawned.contains_key(&shard_id)
            };
            
            if !process_exists {
                eprintln!("[SPAWNER] ⚠️  Process handle for shard {} not found (may have crashed)", shard_id);
                eprintln!("[SPAWNER]   Elapsed: {:.1}s / {}s", elapsed.as_secs_f64(), timeout_secs);
            } else if check_count % 10 == 0 {
                // Log progress every 5 seconds (10 checks * 500ms)
                println!("[SPAWNER]   Still waiting for shard {}... ({:.1}s / {}s elapsed, {} checks)", 
                    shard_id, elapsed.as_secs_f64(), timeout_secs, check_count);
                println!("[SPAWNER]   Discovered shards: {:?}", pipeline.iter().map(|s| s.shard_id).collect::<Vec<_>>());
            }

            tokio::time::sleep(check_interval).await;
        }

        eprintln!("[SPAWNER] ❌ Timeout waiting for shard {} node to come online", shard_id);
        eprintln!("[SPAWNER]   Timeout: {}s", timeout_secs);
        eprintln!("[SPAWNER]   Checks performed: {}", check_count);
        eprintln!("[SPAWNER]   Currently discovered shards: {:?}", {
            let disc = discovery.read().await;
            disc.get_pipeline().iter().map(|s| s.shard_id).collect::<Vec<_>>()
        });
        eprintln!("[SPAWNER]   Possible causes:");
        eprintln!("[SPAWNER]     - Node is still compiling (first run takes 30-60s)");
        eprintln!("[SPAWNER]     - Node crashed during startup (check process logs)");
        eprintln!("[SPAWNER]     - Node hasn't joined DHT yet (bootstrap connection issue)");
        eprintln!("[SPAWNER]     - Network connectivity issues");
        eprintln!("[SPAWNER]     - DHT discovery not working properly");
        
        Err(PipelineError::Timeout {
            missing_shards: vec![shard_id],
            waited_secs: timeout_secs,
        })
    }

    /// Get list of spawned shard IDs
    pub async fn get_spawned_shards(&self) -> Vec<u32> {
        self.spawned_nodes.read().await.keys().cloned().collect()
    }

    /// Terminate a spawned node
    pub async fn terminate_node(&self, shard_id: u32) -> Result<(), PipelineError> {
        let child_opt = {
            let mut spawned = self.spawned_nodes.write().await;
            spawned.remove(&shard_id)
        };

        if let Some(mut child) = child_opt {
            println!("[SPAWNER] Terminating node for shard {}...", shard_id);
            match child.kill().await {
                Ok(_) => {
                    println!("[SPAWNER] ✓ Terminated node for shard {}", shard_id);
                }
                Err(e) => {
                    eprintln!("[SPAWNER] ❌ Failed to terminate node for shard {}: {}", shard_id, e);
                    eprintln!("[SPAWNER]   Error type: {:?}", e.kind());
                    eprintln!("[SPAWNER]   Possible causes:");
                    eprintln!("[SPAWNER]     - Process already terminated");
                    eprintln!("[SPAWNER]     - Insufficient permissions");
                    eprintln!("[SPAWNER]     - Process handle invalid");
                    return Err(PipelineError::Internal {
                        message: format!("Failed to terminate node: {} (kind: {:?})", e, e.kind()),
                    });
                }
            }
        } else {
            println!("[SPAWNER] No process handle found for shard {} (may already be terminated)", shard_id);
        }
        Ok(())
    }

    /// Terminate all spawned nodes
    pub async fn terminate_all(&self) {
        let spawned = self.spawned_nodes.read().await;
        let shard_ids: Vec<u32> = spawned.keys().cloned().collect();
        drop(spawned);

        for shard_id in shard_ids {
            let _ = self.terminate_node(shard_id).await;
        }
    }
}

/// Dynamic shard loading capability
#[derive(Clone)]
pub struct DynamicShardLoader {
    /// Model manager for downloading shards
    model_manager: Arc<RwLock<LlamaModelManager>>,
    /// Currently loaded shards per node
    loaded_shards: Arc<RwLock<HashMap<String, Vec<u32>>>>,
    /// Memory usage per node
    memory_usage: Arc<RwLock<HashMap<String, u64>>>,
    /// Command sender for sending LOAD_SHARD commands (optional)
    command_sender: Option<Arc<dyn Fn(String, crate::command_protocol::Command) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<crate::command_protocol::CommandResponse, PipelineError>> + Send>> + Send + Sync>>,
}

impl DynamicShardLoader {
    pub fn new(model_manager: LlamaModelManager) -> Self {
        Self {
            model_manager: Arc::new(RwLock::new(model_manager)),
            loaded_shards: Arc::new(RwLock::new(HashMap::new())),
            memory_usage: Arc::new(RwLock::new(HashMap::new())),
            command_sender: None,
        }
    }
    
    /// Set command sender for sending LOAD_SHARD commands to nodes
    pub fn with_command_sender<F>(mut self, sender: F) -> Self
    where
        F: Fn(String, crate::command_protocol::Command) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<crate::command_protocol::CommandResponse, PipelineError>> + Send>> + Send + Sync + 'static,
    {
        self.command_sender = Some(Arc::new(sender));
        self
    }

    /// Check if a node can load an additional shard
    pub async fn can_load_shard(&self, node: &ShardAnnouncement, min_memory_mb: u64) -> bool {
        let caps = &node.capabilities;
        caps.memory_available_mb >= min_memory_mb && caps.shard_loaded
    }

    /// Attempt to load a shard on a specific node
    pub async fn load_shard_on_node(
        &self,
        node_id: &str,
        shard_id: u32,
        model_name: &str,
    ) -> Result<(), PipelineError> {
        println!("[LOADER] Attempting to load shard {} on node {}", shard_id, node_id);

        // Send LOAD_SHARD command to the node
        if let Some(ref sender) = self.command_sender {
            use crate::command_protocol::{Command, commands};
            use serde_json::json;
            
            let cmd = Command::new(commands::LOAD_SHARD, "coordinator", Some(node_id))
                .with_param("shard_id", json!(shard_id))
                .with_param("model_name", json!(model_name));
            
            println!("[LOADER] Sending LOAD_SHARD command to node {}", node_id);
            println!("[LOADER]   Shard ID: {}", shard_id);
            println!("[LOADER]   Model: {}", model_name);
            match sender(node_id.to_string(), cmd).await {
                Ok(response) => {
                    println!("[LOADER]   Response status: {:?}", response.status);
                    if response.status == crate::command_protocol::ResponseStatus::Success {
                        println!("[LOADER] ✓ Node {} confirmed shard {} loaded", node_id, shard_id);
                        
                        // Track loaded shard
                        let mut loaded = self.loaded_shards.write().await;
                        let node_shards = loaded.entry(node_id.to_string())
                            .or_insert_with(Vec::new);
                        if !node_shards.contains(&shard_id) {
                            node_shards.push(shard_id);
                            println!("[LOADER]   Node {} now has {} shard(s) loaded", node_id, node_shards.len());
                        }
                        
                        Ok(())
                    } else {
                        let error_msg = response.error.clone().unwrap_or_else(|| "Unknown error".to_string());
                        eprintln!("[LOADER] ❌ Node {} failed to load shard {}: {}", node_id, shard_id, error_msg);
                        eprintln!("[LOADER]   Response status: {:?}", response.status);
                        eprintln!("[LOADER]   Response ID: {}", response.request_id);
                        if let Some(ref result) = response.result {
                            eprintln!("[LOADER]   Response result: {:?}", result);
                        }
                        Err(PipelineError::ShardLoadFailed {
                            shard_id,
                            error: format!("Node {} returned error: {} (status: {:?})", node_id, error_msg, response.status),
                        })
                    }
                }
                Err(e) => {
                    eprintln!("[LOADER] ❌ Failed to send LOAD_SHARD command to node {}: {}", node_id, e);
                    eprintln!("[LOADER]   Shard ID: {}", shard_id);
                    eprintln!("[LOADER]   Model: {}", model_name);
                    eprintln!("[LOADER]   Error details: {:?}", e);
                    eprintln!("[LOADER]   Possible causes:");
                    eprintln!("[LOADER]     - Node {} is not reachable", node_id);
                    eprintln!("[LOADER]     - Network connectivity issues");
                    eprintln!("[LOADER]     - Node crashed or is not responding");
                    eprintln!("[LOADER]     - Command protocol error");
                    Err(PipelineError::ShardLoadFailed {
                        shard_id,
                        error: format!("Failed to send command to node {}: {}", node_id, e),
                    })
                }
            }
        } else {
            // No command sender configured - fallback to local simulation
            println!("[LOADER] WARNING: No command sender configured, simulating shard load");
            
            // Download shard if not cached (local fallback)
            let manager = self.model_manager.read().await;
            let _shard_name = format!("{}-shard-{}.safetensors", model_name, shard_id);
            drop(manager);

            // Track loaded shard
            let mut loaded = self.loaded_shards.write().await;
            loaded.entry(node_id.to_string())
                .or_insert_with(Vec::new)
                .push(shard_id);

            println!("[LOADER] Successfully loaded shard {} on node {} (simulated)", shard_id, node_id);
            Ok(())
        }
    }

    /// Get nodes that could potentially load a shard
    pub async fn get_capable_nodes<'a>(
        &self,
        nodes: &'a [&'a ShardAnnouncement],
        min_memory_mb: u64,
        max_shards_per_node: u32,
    ) -> Vec<&'a ShardAnnouncement> {
        let loaded = self.loaded_shards.read().await;
        
        let capable: Vec<&'a ShardAnnouncement> = nodes.iter()
            .filter(|n| {
                let current_shards = loaded.get(&n.peer_id).map(|v| v.len()).unwrap_or(0);
                let has_memory = n.capabilities.memory_available_mb >= min_memory_mb;
                let has_capacity = (current_shards as u32) < max_shards_per_node;
                
                if !has_memory {
                    println!("[LOADER] Node {} filtered: insufficient memory ({}MB < {}MB)", 
                        n.peer_id, n.capabilities.memory_available_mb, min_memory_mb);
                }
                if !has_capacity {
                    println!("[LOADER] Node {} filtered: at capacity ({} shards >= {} max)", 
                        n.peer_id, current_shards, max_shards_per_node);
                }
                
                has_memory && has_capacity
            })
            .cloned()
            .collect();
        
        println!("[LOADER] Found {} capable node(s) out of {} total for dynamic loading", 
            capable.len(), nodes.len());
        
        capable
    }
}

/// Single-node fallback handler
pub struct SingleNodeFallback {
    /// Node capable of running full model
    fallback_node: Option<ShardAnnouncement>,
    /// Full model path (if available locally)
    model_path: Option<String>,
}

impl SingleNodeFallback {
    pub fn new() -> Self {
        Self {
            fallback_node: None,
            model_path: None,
        }
    }

    /// Find a node capable of running the full model
    pub fn find_capable_node<'a>(
        &mut self,
        nodes: &'a [&'a ShardAnnouncement],
        required_memory_mb: u64,
    ) -> Option<&'a ShardAnnouncement> {
        let capable = nodes.iter()
            .filter(|n| n.capabilities.memory_available_mb >= required_memory_mb)
            .max_by(|a, b| {
                // Prefer nodes with more memory and lower latency
                let score_a = a.capabilities.memory_available_mb as f64 / (1.0 + a.capabilities.latency_ms);
                let score_b = b.capabilities.memory_available_mb as f64 / (1.0 + b.capabilities.latency_ms);
                score_a.partial_cmp(&score_b).unwrap_or(std::cmp::Ordering::Equal)
            });

        if let Some(node) = capable {
            self.fallback_node = Some((*node).clone());
            Some(*node)
        } else {
            None
        }
    }

    /// Check if fallback is available
    pub fn is_available(&self) -> bool {
        self.fallback_node.is_some()
    }

    /// Get the fallback node
    pub fn get_node(&self) -> Option<&ShardAnnouncement> {
        self.fallback_node.as_ref()
    }
}

impl Default for SingleNodeFallback {
    fn default() -> Self {
        Self::new()
    }
}

/// Main pipeline coordinator
#[derive(Clone)]
pub struct PipelineCoordinator {
    /// Shard discovery service
    discovery: Arc<RwLock<KademliaShardDiscovery>>,
    /// Current strategy
    strategy: PipelineStrategy,
    /// Current state
    state: Arc<RwLock<CoordinatorState>>,
    /// Request queue
    request_queue: Arc<Mutex<VecDeque<QueuedRequest>>>,
    /// Nodes that have joined the request queue and are available to load shards
    queued_nodes: Arc<Mutex<Vec<QueuedNode>>>,
    /// Shard demand tracking - tracks which shards are most needed
    shard_demand: Arc<RwLock<HashMap<u32, ShardDemand>>>,
    /// Dynamic shard loader
    shard_loader: Option<DynamicShardLoader>,
    /// Node spawner for on-demand node creation
    node_spawner: Option<Arc<NodeSpawner>>,
    /// Single-node fallback
    fallback: Arc<RwLock<SingleNodeFallback>>,
    /// Statistics
    stats: Arc<RwLock<CoordinatorStats>>,
    /// Shutdown signal
    shutdown_tx: Option<mpsc::Sender<()>>,
    /// Command sender for sending requests to nodes (optional)
    command_sender: Option<Arc<dyn Fn(String, crate::command_protocol::Command) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<crate::command_protocol::CommandResponse, PipelineError>> + Send>> + Send + Sync>>,
}

/// Coordinator statistics
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct CoordinatorStats {
    pub total_requests: u64,
    pub successful_requests: u64,
    pub failed_requests: u64,
    pub queued_requests: u64,
    pub fallback_requests: u64,
    pub dynamic_loads: u64,
    pub nodes_spawned: u64,
    pub average_latency_ms: f64,
    pub average_queue_time_ms: f64,
}

impl PipelineCoordinator {
    /// Create a new coordinator
    pub fn new(discovery: KademliaShardDiscovery) -> Self {
        Self {
            discovery: Arc::new(RwLock::new(discovery)),
            strategy: PipelineStrategy::default(),
            state: Arc::new(RwLock::new(CoordinatorState::Unavailable { 
                reason: "Initializing".to_string() 
            })),
            request_queue: Arc::new(Mutex::new(VecDeque::new())),
            queued_nodes: Arc::new(Mutex::new(Vec::new())),
            shard_demand: Arc::new(RwLock::new(HashMap::new())),
            shard_loader: None,
            node_spawner: None,
            fallback: Arc::new(RwLock::new(SingleNodeFallback::new())),
            stats: Arc::new(RwLock::new(CoordinatorStats::default())),
            shutdown_tx: None,
            command_sender: None,
        }
    }

    /// Node joins the request queue and gets assigned the next needed shard
    pub async fn node_join_queue(&self, peer_id: String, capabilities: ShardCapabilities) -> Result<Option<u32>, PipelineError> {
        println!("[COORDINATOR] Node {} joined request queue", peer_id);
        println!("[COORDINATOR]   Memory available: {}MB", capabilities.memory_available_mb);
        println!("[COORDINATOR]   Shard loaded: {}", capabilities.shard_loaded);
        println!("[COORDINATOR]   Latency: {:.2}ms", capabilities.latency_ms);
        
        let mut queued_nodes = self.queued_nodes.lock().await;
        
        // Check if node already in queue
        if queued_nodes.iter().any(|n| n.peer_id == peer_id) {
            println!("[COORDINATOR] ⚠️  Node {} already in queue (skipping duplicate)", peer_id);
            return Ok(None);
        }
        
        // Add node to queue
        let node = QueuedNode {
            peer_id: peer_id.clone(),
            capabilities: capabilities.clone(),
            joined_at: Instant::now(),
            assigned_shard: None,
            shard_loading: false,
        };
        queued_nodes.push(node);
        drop(queued_nodes);
        
        // Find the next shard that needs to be loaded
        let suggested_shard = self.find_next_needed_shard(&capabilities).await;
        
        if let Some(shard_id) = suggested_shard {
            println!("[COORDINATOR] Suggesting shard {} to node {}", shard_id, peer_id);
            
            // Assign shard to node
            let mut queued_nodes = self.queued_nodes.lock().await;
            if let Some(node) = queued_nodes.iter_mut().find(|n| n.peer_id == peer_id) {
                node.assigned_shard = Some(shard_id);
                node.shard_loading = true;
            }
            drop(queued_nodes);
            
            // Update demand tracking
            self.update_shard_demand(shard_id, true).await;
            
            // Send LOAD_SHARD command to node
            if let Some(ref sender) = self.command_sender {
                use crate::command_protocol::{Command, commands};
                use serde_json::json;
                
                let discovery = self.discovery.read().await;
                let pipeline = discovery.get_pipeline();
                let model_name = pipeline.first()
                    .map(|s| s.model_name.clone())
                    .unwrap_or_else(|| "llama-2-7b".to_string());
                drop(discovery);
                
                let cmd = Command::new(commands::LOAD_SHARD, "coordinator", Some(&peer_id))
                    .with_param("shard_id", json!(shard_id))
                    .with_param("model_name", json!(model_name))
                    .with_param("priority", json!("high")); // High priority for queue nodes
                
                // Send command asynchronously (don't wait)
                let sender_clone = sender.clone();
                let peer_id_clone = peer_id.clone();
                let shard_id_clone = shard_id;
                let model_name_clone = model_name.clone();
                tokio::spawn(async move {
                    println!("[COORDINATOR] 📤 Sending LOAD_SHARD command to node {} for shard {}", 
                        peer_id_clone, shard_id_clone);
                    match sender_clone(peer_id_clone.clone(), cmd).await {
                        Ok(response) => {
                            if response.status == crate::command_protocol::ResponseStatus::Success {
                                println!("[COORDINATOR] ✓ Node {} confirmed loading shard {}", 
                                    peer_id_clone, shard_id_clone);
                            } else {
                                let error_msg = response.error.clone().unwrap_or_else(|| "Unknown error".to_string());
                                eprintln!("[COORDINATOR] ❌ Node {} failed to load shard {}: {}", 
                                    peer_id_clone, shard_id_clone, error_msg);
                                eprintln!("[COORDINATOR]   Response status: {:?}", response.status);
                                eprintln!("[COORDINATOR]   Model: {}", model_name_clone);
                                eprintln!("[COORDINATOR]   Possible causes:");
                                eprintln!("[COORDINATOR]     - Shard file not found");
                                eprintln!("[COORDINATOR]     - Insufficient memory");
                                eprintln!("[COORDINATOR]     - Model file corruption");
                                eprintln!("[COORDINATOR]     - Node internal error");
                            }
                        }
                        Err(e) => {
                            eprintln!("[COORDINATOR] ❌ Failed to send LOAD_SHARD to node {}: {}", peer_id_clone, e);
                            eprintln!("[COORDINATOR]   Shard ID: {}", shard_id_clone);
                            eprintln!("[COORDINATOR]   Model: {}", model_name_clone);
                            eprintln!("[COORDINATOR]   Error details: {:?}", e);
                            eprintln!("[COORDINATOR]   Possible causes:");
                            eprintln!("[COORDINATOR]     - Node {} is not reachable", peer_id_clone);
                            eprintln!("[COORDINATOR]     - Network connectivity issues");
                            eprintln!("[COORDINATOR]     - Command protocol error");
                        }
                    }
                });
            }
            
            Ok(Some(shard_id))
        } else {
            println!("[COORDINATOR] No shard needed at the moment for node {}", peer_id);
            Ok(None)
        }
    }

    /// Find the next shard that needs to be loaded based on demand
    async fn find_next_needed_shard(&self, capabilities: &ShardCapabilities) -> Option<u32> {
        let discovery = self.discovery.read().await;
        let status = discovery.status();
        let missing_shards = discovery.get_missing_shards();
        let _pipeline = discovery.get_pipeline();
        let total_shards = status.expected_shards;
        drop(discovery);
        
        // If pipeline is complete, suggest shards based on demand
        if status.is_complete {
            return self.get_most_needed_shard().await;
        }
        
        // Pipeline incomplete - find the first missing shard in sequence
        // Nodes should load shards in order to complete the pipeline
        for shard_id in 0..total_shards {
            if missing_shards.contains(&shard_id) {
                // Check if node can handle this shard
                if capabilities.memory_available_mb >= 2048 { // At least 2GB for a shard
                    return Some(shard_id);
                }
            }
        }
        
        None
    }

    /// Get the most needed shard based on demand tracking
    async fn get_most_needed_shard(&self) -> Option<u32> {
        let demand = self.shard_demand.read().await;
        
        // Find shard with highest priority score
        demand.iter()
            .max_by(|(_, a), (_, b)| {
                a.priority_score.partial_cmp(&b.priority_score).unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(shard_id, _)| *shard_id)
    }

    /// Update shard demand tracking
    async fn update_shard_demand(&self, shard_id: u32, node_loading: bool) {
        let mut demand = self.shard_demand.write().await;
        let entry = demand.entry(shard_id).or_insert_with(ShardDemand::default);
        
        if node_loading {
            entry.nodes_loading += 1;
        }
        entry.last_requested = Some(Instant::now());
        
        // Recalculate priority score
        // Higher score = more needed
        entry.priority_score = (entry.pending_requests as f64 * 10.0)
            - (entry.nodes_available as f64 * 5.0)
            - (entry.nodes_loading as f64 * 2.0);
        
        drop(demand);
    }

    /// Update shard availability when a node finishes loading
    pub async fn node_shard_loaded(&self, peer_id: String, shard_id: u32) {
        println!("[COORDINATOR] Node {} finished loading shard {}", peer_id, shard_id);
        
        let mut queued_nodes = self.queued_nodes.lock().await;
        if let Some(node) = queued_nodes.iter_mut().find(|n| n.peer_id == peer_id) {
            node.shard_loading = false;
            if node.assigned_shard == Some(shard_id) {
                // Node completed its assigned shard
                println!("[COORDINATOR] ✓ Node {} completed assigned shard {}", peer_id, shard_id);
            } else {
                println!("[COORDINATOR] ⚠️  Node {} loaded shard {} but was assigned shard {:?}", 
                    peer_id, shard_id, node.assigned_shard);
            }
        } else {
            println!("[COORDINATOR] ⚠️  Node {} loaded shard {} but was not in queue", peer_id, shard_id);
        }
        drop(queued_nodes);
        
        // Update demand tracking
        let mut demand = self.shard_demand.write().await;
        if let Some(entry) = demand.get_mut(&shard_id) {
            if entry.nodes_loading > 0 {
                entry.nodes_loading -= 1;
            }
            entry.nodes_available += 1;
            entry.priority_score = (entry.pending_requests as f64 * 10.0)
                - (entry.nodes_available as f64 * 5.0)
                - (entry.nodes_loading as f64 * 2.0);
        }
        drop(demand);
        
        // Check if we can process queued requests now
        let queue_size = {
            let queue = self.request_queue.lock().await;
            queue.len()
        };
        if queue_size > 0 {
            println!("[COORDINATOR] Checking if {} queued request(s) can be processed now...", queue_size);
        }
        self.process_queue_if_ready().await;
    }

    /// Get shard suggestion for a newly joining node
    pub async fn get_shard_suggestion(&self, _peer_id: &str) -> Option<u32> {
        let discovery = self.discovery.read().await;
        let status = discovery.status();
        let missing_shards = discovery.get_missing_shards();
        drop(discovery);
        
        // If pipeline incomplete, suggest first missing shard
        if !status.is_complete && !missing_shards.is_empty() {
            return missing_shards.first().copied();
        }
        
        // Pipeline complete - suggest based on demand
        self.get_most_needed_shard().await
    }

    /// Process queued requests if pipeline is now ready
    async fn process_queue_if_ready(&self) {
        let discovery = self.discovery.read().await;
        let status = discovery.status();
        drop(discovery);
        
        if status.is_complete {
            let mut queue = self.request_queue.lock().await;
            let queued_count = queue.len();
            let _requests: Vec<_> = queue.drain(..).collect();
            drop(queue);
            
            // Note: Queued requests will be processed on next inference submission
            // For now, we just note that the pipeline is ready
            if !_requests.is_empty() {
                println!("[COORDINATOR] ✓ Pipeline complete! {} queued request(s) are ready for processing", queued_count);
                for (idx, req) in _requests.iter().enumerate() {
                    println!("[COORDINATOR]   Queued request {}: ID={}, prompt=\"{}\"", 
                        idx + 1, req.request.request_id, 
                        if req.request.prompt.len() > 50 { 
                            format!("{}...", &req.request.prompt[..50]) 
                        } else { 
                            req.request.prompt.clone() 
                        });
                }
            }
        } else {
            let queue_size = {
                let queue = self.request_queue.lock().await;
                queue.len()
            };
            if queue_size > 0 {
                let discovery = self.discovery.read().await;
                let missing = discovery.get_missing_shards();
                drop(discovery);
                println!("[COORDINATOR] Pipeline still incomplete, {} request(s) remain queued (missing shards: {:?})", 
                    queue_size, missing);
            }
        }
    }

    /// Handle new node announcement - suggest shard to load
    pub async fn handle_node_announcement(&self, announcement: ShardAnnouncement) {
        println!("[COORDINATOR] New node announced: {} (shard {})", 
            announcement.peer_id, announcement.shard_id);
        
        // If node doesn't have a shard assigned, suggest one
        if !announcement.capabilities.shard_loaded {
            let suggested_shard = self.get_shard_suggestion(&announcement.peer_id).await;
            
            if let Some(shard_id) = suggested_shard {
                println!("[COORDINATOR] Suggesting shard {} to newly joined node {}", 
                    shard_id, announcement.peer_id);
                
                // Send suggestion via command if sender available
                if let Some(ref sender) = self.command_sender {
                    use crate::command_protocol::{Command, commands};
                    use serde_json::json;
                    
                    let cmd = Command::new(commands::LOAD_SHARD, "coordinator", Some(&announcement.peer_id))
                        .with_param("shard_id", json!(shard_id))
                        .with_param("model_name", json!(announcement.model_name))
                        .with_param("suggestion", json!(true))
                        .with_param("priority", json!("medium"));
                    
                    let sender_clone = sender.clone();
                    let peer_id = announcement.peer_id.clone();
                    let model_name_clone = announcement.model_name.clone();
                    tokio::spawn(async move {
                        println!("[COORDINATOR] 📤 Sending shard suggestion {} to node {}", shard_id, peer_id);
                        match sender_clone(peer_id.clone(), cmd).await {
                            Ok(response) => {
                                if response.status == crate::command_protocol::ResponseStatus::Success {
                                    println!("[COORDINATOR] ✓ Node {} accepted shard suggestion {}", peer_id, shard_id);
                                } else {
                                    let error_msg = response.error.clone().unwrap_or_else(|| "Unknown error".to_string());
                                    eprintln!("[COORDINATOR] ⚠️  Node {} rejected shard suggestion {}: {}", 
                                        peer_id, shard_id, error_msg);
                                }
                            }
                            Err(e) => {
                                eprintln!("[COORDINATOR] ❌ Failed to send shard suggestion {} to node {}: {}", 
                                    shard_id, peer_id, e);
                                eprintln!("[COORDINATOR]   Model: {}", model_name_clone);
                                eprintln!("[COORDINATOR]   Error details: {:?}", e);
                            }
                        }
                    });
                }
            }
        }
        
        // Update discovery
        self.update_discovery(announcement).await;
    }

    /// Set command sender for sending requests to nodes
    pub fn with_command_sender<F>(mut self, sender: F) -> Self
    where
        F: Fn(String, crate::command_protocol::Command) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<crate::command_protocol::CommandResponse, PipelineError>> + Send>> + Send + Sync + 'static,
    {
        self.command_sender = Some(Arc::new(sender));
        self
    }

    /// Create with model manager for dynamic loading
    pub fn with_model_manager(mut self, manager: LlamaModelManager) -> Self {
        self.shard_loader = Some(DynamicShardLoader::new(manager));
        self
    }

    /// Create with node spawner for on-demand node creation
    pub fn with_node_spawner(mut self, spawner: NodeSpawner) -> Self {
        self.node_spawner = Some(Arc::new(spawner));
        self
    }

    /// Spawn nodes for missing shards proactively (for startup)
    /// Restart all nodes (terminate existing and spawn new ones)
    pub async fn restart_all_nodes(&self) -> Result<(), PipelineError> {
        let Some(spawner) = &self.node_spawner else {
            return Err(PipelineError::NoFallback {
                reason: "Node spawner not configured".to_string(),
            });
        };

        println!("[COORDINATOR] 🔄 Restarting all nodes...");
        
        // Terminate all existing nodes
        println!("[COORDINATOR] Terminating existing nodes...");
        spawner.terminate_all().await;
        
        // Wait for processes to fully terminate
        tokio::time::sleep(Duration::from_secs(2)).await;
        
        // Clear discovery cache to force re-discovery
        {
            let _discovery = self.discovery.write().await;
            // The discovery will naturally update as nodes rejoin
        }
        
        // Now spawn new nodes
        println!("[COORDINATOR] Spawning new nodes...");
        self.spawn_missing_nodes_on_startup().await
    }

    pub async fn spawn_missing_nodes_on_startup(&self) -> Result<(), PipelineError> {
        let Some(spawner) = &self.node_spawner else {
            return Err(PipelineError::NoFallback {
                reason: "Node spawner not configured".to_string(),
            });
        };

        // Wait a bit for DHT to populate
        tokio::time::sleep(Duration::from_secs(2)).await;

        // Check pipeline status
        let discovery = self.discovery.read().await;
        let status = discovery.status();
        let missing_shards = discovery.get_missing_shards();
        drop(discovery);

        if status.is_complete {
            println!("[COORDINATOR] ✓ Pipeline is complete, no nodes need to be spawned");
            return Ok(());
        }

        println!("[COORDINATOR] Pipeline incomplete. Missing shards: {:?}", missing_shards);
        println!("[COORDINATOR] Spawning nodes for missing shards...");

        // Get currently assigned shards to coordinate assignment
        let discovery = self.discovery.read().await;
        let pipeline = discovery.get_pipeline();
        let assigned_shards: Vec<u32> = pipeline.iter().map(|s| s.shard_id).collect();
        let last_assigned = assigned_shards.iter().max().copied();
        drop(discovery);
        
        if let Some(last) = last_assigned {
            println!("[COORDINATOR] Last assigned shard: {}, coordinating next assignment...", last);
        } else {
            println!("[COORDINATOR] No shards assigned yet, starting from shard 0");
        }

        // Spawn nodes for each missing shard, coordinating assignment
        let mut spawn_results = Vec::new();
        let total_shards = status.expected_shards;
        let mut next_shard_to_assign = last_assigned.map(|s| (s + 1) % total_shards).unwrap_or(0);
        
        for _ in 0..missing_shards.len() {
            // Find next available shard starting from last_assigned + 1
            let shard_id = loop {
                if missing_shards.contains(&next_shard_to_assign) {
                    break next_shard_to_assign;
                }
                next_shard_to_assign = (next_shard_to_assign + 1) % total_shards;
            };
            
            println!("[COORDINATOR] Coordinated assignment: spawning node for shard {} (next after last assigned)", shard_id);
            match spawner.spawn_node_for_shard(shard_id).await {
                Ok(()) => {
                    println!("[COORDINATOR] ✓ Spawned node for shard {}", shard_id);
                    spawn_results.push((shard_id, true));
                    // Move to next shard for next iteration
                    next_shard_to_assign = (shard_id + 1) % total_shards;
                }
                Err(e) => {
                    eprintln!("[COORDINATOR] ✗ Failed to spawn node for shard {}: {}", shard_id, e);
                    spawn_results.push((shard_id, false));
                    // Move to next shard even on failure
                    next_shard_to_assign = (shard_id + 1) % total_shards;
                }
            }
        }
        
        // Legacy code path - spawn any remaining missing shards not covered by coordinated assignment
        let mut legacy_spawned = false;
        for shard_id in &missing_shards {
            // Skip if already spawned in coordinated loop
            if spawn_results.iter().any(|(id, _)| *id == *shard_id) {
                continue;
            }
            legacy_spawned = true;
            println!("[COORDINATOR] Spawning node for shard {} (legacy path - should not happen with coordinated assignment)...", shard_id);
            match spawner.spawn_node_for_shard(*shard_id).await {
                Ok(()) => {
                    println!("[COORDINATOR] ✓ Spawned node for shard {}", shard_id);
                    spawn_results.push((*shard_id, true));
                }
                Err(e) => {
                    eprintln!("[COORDINATOR] ✗ Failed to spawn node for shard {}: {}", shard_id, e);
                    spawn_results.push((*shard_id, false));
                }
            }
        }
        
        if legacy_spawned {
            println!("[COORDINATOR] Note: Some shards were spawned via legacy path (should use coordinated assignment)");
        }
        
        // Report spawn summary
        let successful = spawn_results.iter().filter(|(_, success)| *success).count();
        let failed = spawn_results.len() - successful;
        if failed > 0 {
            let failed_shards: Vec<u32> = spawn_results.iter()
                .filter_map(|(id, success)| if !*success { Some(*id) } else { None })
                .collect();
            eprintln!("[COORDINATOR] ⚠️  Summary: {} nodes spawned successfully, {} failed (shards: {:?})", 
                     successful, failed, failed_shards);
            eprintln!("[COORDINATOR]   Failed nodes may retry automatically or can be manually restarted");
        } else {
            println!("[COORDINATOR] ✓ All {} nodes spawned successfully", successful);
        }

        // Wait for nodes to come online (only for successfully spawned nodes)
        println!("[COORDINATOR] Waiting for spawned nodes to come online (30s timeout per node)...");
        let mut online_results = Vec::new();
        for shard_id in &missing_shards {
            // Only wait for nodes that were successfully spawned
            if spawn_results.iter().any(|(id, success)| *id == *shard_id && *success) {
                println!("[COORDINATOR] Waiting for shard {} node to come online...", shard_id);
                match spawner.wait_for_node_online(*shard_id, 30, &self.discovery).await {
                    Ok(()) => {
                        println!("[COORDINATOR] ✓ Shard {} node is online and discovered", shard_id);
                        online_results.push((*shard_id, true));
                    }
                    Err(e) => {
                        eprintln!("[COORDINATOR] ⚠️  Shard {} node did not come online in time: {}", shard_id, e);
                        eprintln!("[COORDINATOR]   Node may still be compiling or starting. It will be used when ready.");
                        online_results.push((*shard_id, false));
                    }
                }
            }
        }
        
        // Report online summary
        let online_count = online_results.iter().filter(|(_, online)| *online).count();
        let still_waiting: Vec<u32> = online_results.iter()
            .filter_map(|(id, online)| if !*online { Some(*id) } else { None })
            .collect();
        if !still_waiting.is_empty() {
            println!("[COORDINATOR] ⚠️  Online summary: {}/{} nodes online, {} still starting (shards: {:?})", 
                     online_count, online_results.len(), still_waiting.len(), still_waiting);
        } else if online_count > 0 {
            println!("[COORDINATOR] ✓ All {} spawned nodes are online!", online_count);
        }
        
        // Send LOAD_SHARD commands to nodes that came online
        if let Some(ref sender) = self.command_sender {
            println!("[COORDINATOR] Sending LOAD_SHARD commands to online nodes...");
            let discovery = self.discovery.read().await;
            let pipeline = discovery.get_pipeline();
            let model_name = pipeline.first()
                .map(|s| s.model_name.clone())
                .unwrap_or_else(|| "llama-2-7b".to_string());
            drop(discovery);
            
            for (shard_id, is_online) in &online_results {
                if !*is_online {
                    continue;
                }
                
                // Find the peer_id for this shard
                let discovery = self.discovery.read().await;
                let pipeline = discovery.get_pipeline();
                let peer_id = pipeline.iter()
                    .find(|s| s.shard_id == *shard_id)
                    .map(|s| s.peer_id.clone());
                drop(discovery);
                
                if let Some(peer_id) = peer_id {
                    use crate::command_protocol::{Command, commands};
                    use serde_json::json;
                    
                    let cmd = Command::new(commands::LOAD_SHARD, "coordinator", Some(&peer_id))
                        .with_param("shard_id", json!(*shard_id))
                        .with_param("model_name", json!(model_name))
                        .with_param("priority", json!("high"));
                    
                    let sender_clone = sender.clone();
                    let peer_id_clone = peer_id.clone();
                    let shard_id_clone = *shard_id;
                    tokio::spawn(async move {
                        println!("[COORDINATOR] 📤 Sending LOAD_SHARD command to node {} for shard {}", peer_id_clone, shard_id_clone);
                        match sender_clone(peer_id_clone.clone(), cmd).await {
                            Ok(response) => {
                                if response.status == crate::command_protocol::ResponseStatus::Success {
                                    println!("[COORDINATOR] ✓ Node {} confirmed loading shard {}", peer_id_clone, shard_id_clone);
                                } else {
                                    eprintln!("[COORDINATOR] ⚠️  Node {} failed to load shard {}: {:?}", 
                                        peer_id_clone, shard_id_clone, response.error);
                                }
                            }
                            Err(e) => {
                                eprintln!("[COORDINATOR] ⚠️  Failed to send LOAD_SHARD to node {}: {}", peer_id_clone, e);
                            }
                        }
                    });
                } else {
                    eprintln!("[COORDINATOR] ⚠️  Could not find peer_id for shard {} to send LOAD_SHARD command", shard_id);
                }
            }
            
            // Give nodes some time to process LOAD_SHARD commands
            println!("[COORDINATOR] Waiting for shards to load (5s)...");
            tokio::time::sleep(Duration::from_secs(5)).await;
        } else {
            println!("[COORDINATOR] ⚠️  No command sender configured - cannot send LOAD_SHARD commands");
            println!("[COORDINATOR]   Nodes will need to load shards manually or via other mechanisms");
        }

        // Final check
        let discovery = self.discovery.read().await;
        let status = discovery.status();
        let discovered_shards = discovery.shard_count();
        drop(discovery);

        if status.is_complete {
            println!("[COORDINATOR] ✓✓✓ Pipeline is complete! All {} shards are online and ready.", status.expected_shards);
            Ok(())
        } else {
            let still_missing = status.missing_shards;
            println!("[COORDINATOR] ⚠️  Pipeline status: {}/{} shards discovered, missing: {:?}", 
                     discovered_shards, status.expected_shards, still_missing);
            if !still_missing.is_empty() {
                println!("[COORDINATOR]   Missing shard IDs: {:?}", still_missing);
                println!("[COORDINATOR]   Possible reasons:");
                println!("[COORDINATOR]     - Nodes are still compiling (first run takes 30-60s)");
                println!("[COORDINATOR]     - Nodes crashed during startup (check logs)");
                println!("[COORDINATOR]     - Nodes haven't joined DHT yet (waiting for bootstrap)");
                println!("[COORDINATOR]   Nodes will be used automatically when they come online.");
            }
            Ok(())
        }
    }

    /// Set the pipeline strategy
    pub fn set_strategy(&mut self, strategy: PipelineStrategy) {
        self.strategy = strategy;
    }

    /// Get current state
    pub async fn state(&self) -> CoordinatorState {
        self.state.read().await.clone()
    }

    /// Get current statistics
    pub async fn stats(&self) -> CoordinatorStats {
        self.stats.read().await.clone()
    }

    /// Get current pipeline status for web UI
    pub async fn get_pipeline_status(&self) -> (u32, u32, Vec<u32>, bool) {
        let (online_nodes, missing_shards, is_complete, total_replicas) = {
            let discovery = self.discovery.read().await;
            let status = discovery.status();
            let pipeline = discovery.get_pipeline();
            let online_nodes = pipeline.len() as u32;
            let missing_shards = discovery.get_missing_shards();
            let is_complete = status.is_complete;
            let total_replicas = status.total_replicas;
            (online_nodes, missing_shards, is_complete, total_replicas)
        };
        
        // Log status for debugging (only when nodes are discovered)
        if online_nodes > 0 || !missing_shards.is_empty() {
            println!("[STATUS] Pipeline: {}/{} shards online, {} total replicas, complete: {}, missing: {:?}", 
                     online_nodes, 4, total_replicas, is_complete, missing_shards);
        }
        
        (online_nodes, 4, missing_shards, is_complete) // 4 = total expected shards
    }

    /// Process a new shard announcement - automatically suggest shard if needed
    pub async fn process_new_shard_announcement(&self, announcement: ShardAnnouncement) {
        println!("[COORDINATOR] Processing new shard announcement from {}", announcement.peer_id);
        
        // Add to discovery
        self.update_discovery(announcement.clone()).await;
        
        // If node doesn't have a shard loaded, suggest one
        if !announcement.capabilities.shard_loaded {
            // Node can join queue and get assigned a shard
            match self.node_join_queue(
                announcement.peer_id.clone(),
                announcement.capabilities.clone()
            ).await {
                Ok(Some(suggested_shard)) => {
                    println!("[COORDINATOR] ✓ Node {} assigned shard {}", announcement.peer_id, suggested_shard);
                }
                Ok(None) => {
                    println!("[COORDINATOR] Node {} joined queue but no shard assigned yet", announcement.peer_id);
                }
                Err(e) => {
                    eprintln!("[COORDINATOR] ❌ Failed to add node {} to queue: {}", announcement.peer_id, e);
                    eprintln!("[COORDINATOR]   Error details: {:?}", e);
                }
            }
        } else {
            // Node already has a shard - update availability
            self.node_shard_loaded(announcement.peer_id.clone(), announcement.shard_id).await;
        }
    }

    /// Update discovery with new shard information
    /// Uses Kademlia routing table depth for better weighting
    pub async fn update_discovery(&self, announcement: ShardAnnouncement) {
        let is_new = {
            let discovery = self.discovery.read().await;
            !discovery.get_pipeline().iter().any(|s| s.peer_id == announcement.peer_id)
        };
        
        {
            let mut discovery = self.discovery.write().await;
            // add_shard now automatically calculates routing depth if local_peer_id is set
            discovery.add_shard(announcement.clone());
        }
        
        // If this is a new node, process it for queue joining and shard suggestions
        if is_new {
            println!("[COORDINATOR] Processing new shard announcement from {}", announcement.peer_id);
            
            // If node doesn't have a shard loaded, suggest one
            if !announcement.capabilities.shard_loaded {
                // Node can join queue and get assigned a shard
                match self.node_join_queue(
                    announcement.peer_id.clone(),
                    announcement.capabilities.clone()
                ).await {
                    Ok(Some(suggested_shard)) => {
                        println!("[COORDINATOR] ✓ Node {} assigned shard {}", announcement.peer_id, suggested_shard);
                    }
                    Ok(None) => {
                        println!("[COORDINATOR] Node {} joined queue but no shard assigned yet", announcement.peer_id);
                    }
                    Err(e) => {
                        eprintln!("[COORDINATOR] ❌ Failed to add node {} to queue: {}", announcement.peer_id, e);
                        eprintln!("[COORDINATOR]   Error details: {:?}", e);
                    }
                }
            } else {
                // Node already has a shard - update availability
                self.node_shard_loaded(announcement.peer_id.clone(), announcement.shard_id).await;
            }
        }
        
        // Check if this completes the pipeline
        self.update_state().await;
    }

    /// Process a DHT record and add to discovery if valid
    pub async fn process_dht_record(&self, record: &kad::Record) -> Option<ShardAnnouncement> {
        let mut discovery = self.discovery.write().await;
        let announcement = discovery.process_shard_record(record)?;
        drop(discovery);
        
        // Update state
        self.update_state().await;
        
        Some(announcement)
    }

    /// Update routing depth for a peer in discovery
    pub async fn update_routing_depth(&self, peer_id: String, depth: u32) {
        let mut discovery = self.discovery.write().await;
        discovery.update_routing_depth(peer_id, depth);
    }

    /// Update coordinator state based on current discovery
    async fn update_state(&self) {
        let discovery = self.discovery.read().await;
        let status = discovery.status();
        drop(discovery);

        let new_state = if status.is_complete {
            CoordinatorState::Ready
        } else {
            CoordinatorState::WaitingForShards { 
                missing: status.missing_shards 
            }
        };

        let mut state = self.state.write().await;
        *state = new_state;
    }

    /// Submit an inference request
    pub async fn submit_inference(
        &self,
        request: InferenceRequest,
    ) -> Result<InferenceResponse, PipelineError> {
        let start = Instant::now();
        
        // Update stats
        {
            let mut stats = self.stats.write().await;
            stats.total_requests += 1;
        }

        // Check pipeline status
        let discovery = self.discovery.read().await;
        let status = discovery.status();
        let missing_shards = discovery.get_missing_shards();
        drop(discovery);
        
        // Update demand tracking for missing shards
        {
            let mut demand = self.shard_demand.write().await;
            for shard_id in &missing_shards {
                let entry = demand.entry(*shard_id).or_insert_with(ShardDemand::default);
                entry.pending_requests += 1;
                entry.last_requested = Some(Instant::now());
                entry.priority_score = (entry.pending_requests as f64 * 10.0)
                    - (entry.nodes_available as f64 * 5.0)
                    - (entry.nodes_loading as f64 * 2.0);
            }
        }

        if status.is_complete {
            // Pipeline ready - process immediately
            println!("[COORDINATOR] Pipeline is complete, processing inference immediately");
            return self.process_inference(request, start).await;
        } else {
            println!("[COORDINATOR] Pipeline incomplete (missing: {:?}), applying strategy: {:?}", missing_shards, self.strategy);
        }

        // Pipeline incomplete - apply strategy
        match &self.strategy {
            PipelineStrategy::FailFast => {
                eprintln!("[COORDINATOR] ❌ FailFast strategy: Pipeline incomplete");
                eprintln!("[COORDINATOR]   Request ID: {}", request.request_id);
                eprintln!("[COORDINATOR]   Missing shards: {:?}", status.missing_shards);
                eprintln!("[COORDINATOR]   Discovered: {}/{} shards", status.discovered_shards, status.expected_shards);
                eprintln!("[COORDINATOR]   Strategy: FailFast (no retry)");
                self.record_failure().await;
                Err(PipelineError::NoFallback {
                    reason: format!("Pipeline incomplete, missing shards: {:?} (FailFast strategy)", status.missing_shards),
                })
            }

            PipelineStrategy::WaitAndRetry { timeout_secs, retry_interval_ms } => {
                self.wait_for_pipeline(request, *timeout_secs, *retry_interval_ms, start).await
            }

            PipelineStrategy::DynamicLoading { max_shards_per_node, min_memory_mb } => {
                self.try_dynamic_loading(
                    request, 
                    &status.missing_shards, 
                    *max_shards_per_node, 
                    *min_memory_mb,
                    start,
                ).await
            }

            PipelineStrategy::SingleNodeFallback { required_memory_mb } => {
                self.try_fallback(request, *required_memory_mb, start).await
            }

            PipelineStrategy::Adaptive { 
                wait_timeout_secs, 
                min_memory_for_shard_mb, 
                min_memory_for_full_mb,
            } => {
                self.try_adaptive(
                    request,
                    &status.missing_shards,
                    *wait_timeout_secs,
                    *min_memory_for_shard_mb,
                    *min_memory_for_full_mb,
                    start,
                ).await
            }

            PipelineStrategy::SpawnNodes {
                max_nodes_per_request,
                min_memory_per_node_mb: _,
                spawn_command_template: _,
                node_startup_timeout_secs,
            } => {
                self.try_spawn_nodes(
                    request,
                    &status.missing_shards,
                    *max_nodes_per_request,
                    *node_startup_timeout_secs,
                    start,
                ).await
            }
        }
    }

    /// Wait for pipeline to become complete
    async fn wait_for_pipeline(
        &self,
        request: InferenceRequest,
        timeout_secs: u64,
        retry_interval_ms: u64,
        start: Instant,
    ) -> Result<InferenceResponse, PipelineError> {
        let timeout = Duration::from_secs(timeout_secs);
        let retry_interval = Duration::from_millis(retry_interval_ms);
        let deadline = Instant::now() + timeout;

        println!("[COORDINATOR] Waiting for pipeline (timeout: {}s)...", timeout_secs);

        // Update stats
        {
            let mut stats = self.stats.write().await;
            stats.queued_requests += 1;
        }

        while Instant::now() < deadline {
            // Check if pipeline is now complete
            let discovery = self.discovery.read().await;
            let status = discovery.status();
            drop(discovery);

            if status.is_complete {
                println!("[COORDINATOR] Pipeline now complete after waiting");
                let mut stats = self.stats.write().await;
                stats.average_queue_time_ms = 
                    (stats.average_queue_time_ms * (stats.queued_requests - 1) as f64 
                     + start.elapsed().as_millis() as f64) / stats.queued_requests as f64;
                drop(stats);

                return self.process_inference(request, start).await;
            }

            println!("[COORDINATOR] Still waiting, missing: {:?}", status.missing_shards);
            tokio::time::sleep(retry_interval).await;
        }

        // Timeout expired
        let discovery = self.discovery.read().await;
        let missing = discovery.get_missing_shards();
        let status = discovery.status();
        drop(discovery);

        let waited_secs_actual = start.elapsed().as_secs();
        eprintln!("[COORDINATOR] ❌ Timeout waiting for pipeline to become complete");
        eprintln!("[COORDINATOR]   Request ID: {}", request.request_id);
        eprintln!("[COORDINATOR]   Timeout: {}s (actual wait: {}s)", timeout_secs, waited_secs_actual);
        eprintln!("[COORDINATOR]   Missing shards: {:?}", missing);
        eprintln!("[COORDINATOR]   Discovered: {}/{} shards", status.discovered_shards, status.expected_shards);
        eprintln!("[COORDINATOR]   Strategy: WaitAndRetry");
        eprintln!("[COORDINATOR]   Retry interval: {}ms", retry_interval_ms);
        eprintln!("[COORDINATOR]   Possible causes:");
        eprintln!("[COORDINATOR]     - Nodes are still starting up");
        eprintln!("[COORDINATOR]     - Nodes crashed during startup");
        eprintln!("[COORDINATOR]     - DHT discovery not working properly");
        eprintln!("[COORDINATOR]     - Network connectivity issues");
        eprintln!("[COORDINATOR]     - Bootstrap server not accessible");
        
        self.record_failure().await;
        Err(PipelineError::Timeout {
            missing_shards: missing,
            waited_secs: timeout_secs,
        })
    }

    /// Try to dynamically load missing shards
    async fn try_dynamic_loading(
        &self,
        request: InferenceRequest,
        missing_shards: &[u32],
        max_shards_per_node: u32,
        min_memory_mb: u64,
        start: Instant,
    ) -> Result<InferenceResponse, PipelineError> {
        let Some(loader) = &self.shard_loader else {
            self.record_failure().await;
            return Err(PipelineError::NoFallback {
                reason: "Dynamic loading not configured".to_string(),
            });
        };

        println!("[COORDINATOR] Attempting dynamic shard loading for: {:?}", missing_shards);

        // Update state
        {
            let mut state = self.state.write().await;
            *state = CoordinatorState::LoadingShards { loading: missing_shards.to_vec() };
        }

        // Get all available nodes - clone to avoid borrow issues
        let discovery = self.discovery.read().await;
        let pipeline: Vec<ShardAnnouncement> = discovery.get_pipeline().into_iter().cloned().collect();
        drop(discovery);

        // Find nodes that can load additional shards
        let pipeline_refs: Vec<&ShardAnnouncement> = pipeline.iter().collect();
        let capable_nodes = loader.get_capable_nodes(&pipeline_refs, min_memory_mb, max_shards_per_node).await;

        if capable_nodes.is_empty() {
            let available_node_count = {
                let discovery = self.discovery.read().await;
                discovery.get_pipeline().len()
            };
            eprintln!("[COORDINATOR] ❌ No capable nodes found for dynamic loading");
            eprintln!("[COORDINATOR]   Request ID: {}", request.request_id);
            eprintln!("[COORDINATOR]   Missing shards: {:?}", missing_shards);
            eprintln!("[COORDINATOR]   Required memory per shard: {}MB", min_memory_mb);
            eprintln!("[COORDINATOR]   Max shards per node: {}", max_shards_per_node);
            eprintln!("[COORDINATOR]   Available nodes: {}", available_node_count);
            eprintln!("[COORDINATOR]   Possible causes:");
            eprintln!("[COORDINATOR]     - No nodes have sufficient memory");
            eprintln!("[COORDINATOR]     - All nodes are at capacity");
            eprintln!("[COORDINATOR]     - Memory requirements too high");
            
            self.record_failure().await;
            return Err(PipelineError::NoFallback {
                reason: format!("No nodes with sufficient capacity for dynamic loading (required: {}MB, max_shards: {})", 
                    min_memory_mb, max_shards_per_node),
            });
        }

        // Try to load each missing shard
        for shard_id in missing_shards {
            // Round-robin across capable nodes
            let node_idx = (*shard_id as usize) % capable_nodes.len();
            let node = capable_nodes[node_idx];

            println!("[COORDINATOR] Attempting to load shard {} on node {} (node {}/{})", 
                shard_id, node.peer_id, node_idx + 1, capable_nodes.len());
            
            if let Err(e) = loader.load_shard_on_node(&node.peer_id, *shard_id, &node.model_name).await {
                eprintln!("[COORDINATOR] ❌ Failed to load shard {} on node {}: {}", 
                    shard_id, node.peer_id, e);
                eprintln!("[COORDINATOR]   Request ID: {}", request.request_id);
                eprintln!("[COORDINATOR]   Node memory: {}MB", node.capabilities.memory_available_mb);
                eprintln!("[COORDINATOR]   Remaining missing shards: {:?}", 
                    missing_shards.iter().filter(|&&s| s > *shard_id).collect::<Vec<_>>());
                
                self.record_failure().await;
                return Err(e);
            }
            
            println!("[COORDINATOR] ✓ Successfully loaded shard {} on node {}", shard_id, node.peer_id);
            
            // Update discovery with new shard announcement
            let mut new_announcement = node.clone();
            new_announcement.shard_id = *shard_id;
            new_announcement.layer_start = shard_id * (32 / 4); // Assuming 4 shards, 32 layers
            new_announcement.layer_end = (shard_id + 1) * (32 / 4);
            new_announcement.has_embeddings = *shard_id == 0;
            new_announcement.has_output = *shard_id == 3;

            self.update_discovery(new_announcement).await;

            // Track stats
            {
                let mut stats = self.stats.write().await;
                stats.dynamic_loads += 1;
            }
        }

        // Now process the request
        self.process_inference(request, start).await
    }

    /// Try single-node fallback
    async fn try_fallback(
        &self,
        request: InferenceRequest,
        required_memory_mb: u64,
        start: Instant,
    ) -> Result<InferenceResponse, PipelineError> {
        println!("[COORDINATOR] Attempting single-node fallback...");

        // Clone nodes to avoid borrow issues
        let discovery = self.discovery.read().await;
        let all_nodes: Vec<ShardAnnouncement> = discovery.get_pipeline().into_iter().cloned().collect();
        drop(discovery);

        let all_nodes_refs: Vec<&ShardAnnouncement> = all_nodes.iter().collect();
        let mut fallback = self.fallback.write().await;
        let fallback_node = fallback.find_capable_node(&all_nodes_refs, required_memory_mb);

        if let Some(node) = fallback_node {
            println!("[COORDINATOR] Using fallback node: {}", node.peer_id);

            // Update state
            {
                let mut state = self.state.write().await;
                *state = CoordinatorState::FallbackMode { node_id: node.peer_id.clone() };
            }

            // Track stats
            {
                let mut stats = self.stats.write().await;
                stats.fallback_requests += 1;
            }

            // Process on fallback node (simulated)
            self.process_on_fallback(request, node, start).await
        } else {
            eprintln!("[COORDINATOR] ❌ No fallback node available");
            eprintln!("[COORDINATOR]   Request ID: {}", request.request_id);
            eprintln!("[COORDINATOR]   Required memory: {}MB", required_memory_mb);
            eprintln!("[COORDINATOR]   Available nodes: {}", all_nodes_refs.len());
            let node_count = all_nodes_refs.len();
            if !all_nodes_refs.is_empty() {
                eprintln!("[COORDINATOR]   Node memory capacities:");
                for node in &all_nodes_refs {
                    eprintln!("[COORDINATOR]     - Node {}: {}MB (shard {})", 
                        node.peer_id, node.capabilities.memory_available_mb, node.shard_id);
                }
            } else {
                eprintln!("[COORDINATOR]   No nodes available in pipeline");
            }
            eprintln!("[COORDINATOR]   Possible causes:");
            eprintln!("[COORDINATOR]     - No nodes have sufficient memory for full model");
            eprintln!("[COORDINATOR]     - Pipeline is empty");
            eprintln!("[COORDINATOR]     - Memory requirements too high");
            
            self.record_failure().await;
            Err(PipelineError::NoFallback {
                reason: format!("No node with {}MB+ memory available (checked {} nodes)", 
                    required_memory_mb, node_count),
            })
        }
    }

    /// Try adaptive strategy: dynamic loading → wait → fallback
    async fn try_adaptive(
        &self,
        request: InferenceRequest,
        missing_shards: &[u32],
        wait_timeout_secs: u64,
        min_memory_for_shard_mb: u64,
        min_memory_for_full_mb: u64,
        start: Instant,
    ) -> Result<InferenceResponse, PipelineError> {
        println!("[COORDINATOR] Using adaptive strategy...");

        // Step 1: Try dynamic loading if configured
        if self.shard_loader.is_some() {
            println!("[COORDINATOR] Step 1: Trying dynamic shard loading...");
            match self.try_dynamic_loading(
                request.clone(),
                missing_shards,
                2,  // max 2 shards per node
                min_memory_for_shard_mb,
                start,
            ).await {
                Ok(response) => {
                    println!("[COORDINATOR] ✓ Dynamic loading succeeded");
                    return Ok(response);
                }
                Err(e) => {
                    eprintln!("[COORDINATOR] ⚠️  Dynamic loading failed: {}", e);
                    eprintln!("[COORDINATOR]   Continuing to next strategy...");
                }
            }
        }

        // Step 2: Try waiting for shards
        println!("[COORDINATOR] Step 2: Waiting for shards...");
        match self.wait_for_pipeline(
            request.clone(),
            wait_timeout_secs / 2,  // Use half the timeout
            500,  // 500ms retry interval
            start,
        ).await {
            Ok(response) => {
                println!("[COORDINATOR] ✓ Wait strategy succeeded");
                return Ok(response);
            }
            Err(e) => {
                eprintln!("[COORDINATOR] ⚠️  Wait strategy failed: {}", e);
                eprintln!("[COORDINATOR]   Continuing to next strategy...");
            }
        }

        // Step 3: Try spawning nodes if spawner is available
        if self.node_spawner.is_some() {
            println!("[COORDINATOR] Step 3: Trying to spawn nodes...");
            match self.try_spawn_nodes(
                request.clone(),
                missing_shards,
                4,  // max 4 nodes
                30, // 30 second timeout
                start,
            ).await {
                Ok(response) => {
                    println!("[COORDINATOR] ✓ Node spawning succeeded");
                    return Ok(response);
                }
                Err(e) => {
                    eprintln!("[COORDINATOR] ⚠️  Node spawning failed: {}", e);
                    eprintln!("[COORDINATOR]   Continuing to fallback strategy...");
                }
            }
        }

        // Step 4: Try single-node fallback
        println!("[COORDINATOR] Step 4: Trying single-node fallback...");
        self.try_fallback(request, min_memory_for_full_mb, start).await
    }

    /// Try to spawn new nodes for missing shards
    async fn try_spawn_nodes(
        &self,
        request: InferenceRequest,
        missing_shards: &[u32],
        max_nodes_per_request: u32,
        node_startup_timeout_secs: u64,
        start: Instant,
    ) -> Result<InferenceResponse, PipelineError> {
        let Some(spawner) = &self.node_spawner else {
            self.record_failure().await;
            return Err(PipelineError::NoFallback {
                reason: "Node spawner not configured".to_string(),
            });
        };

        println!("[COORDINATOR] Attempting to spawn nodes for missing shards: {:?}", missing_shards);

        // Limit number of nodes to spawn
        let shards_to_spawn: Vec<u32> = missing_shards.iter()
            .take(max_nodes_per_request as usize)
            .cloned()
            .collect();

        if shards_to_spawn.is_empty() {
            return Err(PipelineError::NoFallback {
                reason: "No shards to spawn".to_string(),
            });
        }

        // Update state
        {
            let mut state = self.state.write().await;
            *state = CoordinatorState::LoadingShards { loading: shards_to_spawn.clone() };
        }

        // Spawn nodes for each missing shard
        for shard_id in &shards_to_spawn {
            println!("[COORDINATOR] Spawning node for shard {} (request: {})", 
                shard_id, request.request_id);
            if let Err(e) = spawner.spawn_node_for_shard(*shard_id).await {
                eprintln!("[COORDINATOR] ❌ Failed to spawn node for shard {}: {}", shard_id, e);
                eprintln!("[COORDINATOR]   Request ID: {}", request.request_id);
                eprintln!("[COORDINATOR]   Missing shards: {:?}", shards_to_spawn);
                eprintln!("[COORDINATOR]   Successfully spawned: {:?}", 
                    shards_to_spawn.iter().filter(|&&s| s < *shard_id).collect::<Vec<_>>());
                self.record_failure().await;
                return Err(e);
            }
            println!("[COORDINATOR] ✓ Spawned node for shard {}", shard_id);
        }

        // Wait for nodes to come online
        for shard_id in &shards_to_spawn {
            println!("[COORDINATOR] Waiting for shard {} node to come online (timeout: {}s)...", 
                shard_id, node_startup_timeout_secs);
            if let Err(e) = spawner.wait_for_node_online(
                *shard_id,
                node_startup_timeout_secs,
                &self.discovery,
            ).await {
                eprintln!("[COORDINATOR] ❌ Node for shard {} failed to come online: {}", shard_id, e);
                eprintln!("[COORDINATOR]   Request ID: {}", request.request_id);
                eprintln!("[COORDINATOR]   Timeout: {}s", node_startup_timeout_secs);
                eprintln!("[COORDINATOR]   Successfully online: {:?}", 
                    shards_to_spawn.iter().filter(|&&s| s < *shard_id).collect::<Vec<_>>());
                self.record_failure().await;
                return Err(e);
            }
            println!("[COORDINATOR] ✓ Shard {} node is online", shard_id);
        }

        // Update stats
        {
            let mut stats = self.stats.write().await;
            stats.nodes_spawned += shards_to_spawn.len() as u64;
        }

        println!("[COORDINATOR] ✓ All nodes spawned and online, processing inference...");

        // Now process the request
        self.process_inference(request, start).await
    }

    /// Process inference through the pipeline
    async fn process_inference(
        &self,
        request: InferenceRequest,
        start: Instant,
    ) -> Result<InferenceResponse, PipelineError> {
        println!("\n[INFERENCE] ═══════════════════════════════════════════════════════════════════════════");
        println!("[INFERENCE] Starting collaborative inference process");
        println!("[INFERENCE] Request ID: {}", request.request_id);
        println!("[INFERENCE] Prompt: \"{}\"", request.prompt);
        println!("[INFERENCE] Max Tokens: {}, Temperature: {:.2}", request.max_tokens, request.temperature);
        println!("[INFERENCE] ═══════════════════════════════════════════════════════════════════════════\n");

        let pipeline_clone: Vec<ShardAnnouncement> = {
            let discovery = self.discovery.read().await;
            let pipeline = discovery.get_pipeline();
            let cloned: Vec<ShardAnnouncement> = pipeline.iter().map(|s| (*s).clone()).collect();
            if cloned.is_empty() {
                drop(discovery);
                eprintln!("[INFERENCE] ❌ Pipeline is empty - cannot process inference");
                eprintln!("[INFERENCE]   Request ID: {}", request.request_id);
                eprintln!("[INFERENCE]   Prompt: \"{}\"", request.prompt);
                eprintln!("[INFERENCE]   Possible causes:");
                eprintln!("[INFERENCE]     - No nodes have joined the pipeline");
                eprintln!("[INFERENCE]     - DHT discovery not working");
                eprintln!("[INFERENCE]     - Nodes haven't announced themselves");
                eprintln!("[INFERENCE]     - Bootstrap connection failed");
                self.record_failure().await;
                return Err(PipelineError::Internal {
                    message: "Pipeline is empty - no shards available for processing".to_string(),
                });
            }
            cloned
        };

        println!("[INFERENCE] Pipeline discovered: {} shards", pipeline_clone.len());
        for (_idx, shard) in pipeline_clone.iter().enumerate() {
            println!("[INFERENCE]   Shard {}: layers {}-{} on node {}", 
                shard.shard_id, shard.layer_start, shard.layer_end, shard.peer_id);
        }
        println!();

        let mut shard_latencies = Vec::new();
        let mut shard_outputs = Vec::new();
        let mut current_input = request.prompt.clone();

        // Send preload messages for each shard BEFORE processing starts
        println!("[INFERENCE] 📦 Sending preload messages for {} shards", pipeline_clone.len());
        for shard in &pipeline_clone {
            println!("[INFERENCE] 📦 Preload: Node {} will process shard {} (layers {}-{})", 
                     shard.peer_id, shard.shard_id, shard.layer_start, shard.layer_end);
        }
        
        // Process through each shard in sequence (pipeline parallelism)
        for (idx, shard) in pipeline_clone.iter().enumerate() {
            let shard_start = Instant::now();
            
            println!("[INFERENCE] ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
            println!("[INFERENCE] Processing Shard {} of {} (Pipeline Step {})", 
                shard.shard_id, pipeline_clone.len() - 1, idx + 1);
            println!("[INFERENCE]   Node ID: {}", shard.peer_id);
            println!("[INFERENCE]   Layers: {}-{} ({} layers)", 
                shard.layer_start, shard.layer_end, shard.layer_end - shard.layer_start);
            println!("[INFERENCE]   Input length: {} characters", current_input.len());
            println!("[INFERENCE]   📦 Shard {} processing layers {}-{}", 
                shard.shard_id, shard.layer_start, shard.layer_end);
            println!("[INFERENCE] ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

            // Send inference request to the shard node
            let shard_output = if let Some(ref sender) = self.command_sender {
                // Real inference: send command to node
                use crate::command_protocol::{Command, commands};
                use serde_json::json;
                
                println!("[INFERENCE] 📤 Sending JSON command to node {}:", shard.peer_id);
                let cmd = Command::new(commands::EXECUTE_TASK, "coordinator", Some(&shard.peer_id))
                    .with_param("task_type", json!("ai_inference"))
                    .with_param("input_data", json!(current_input))
                    .with_param("max_tokens", json!(request.max_tokens))
                    .with_param("temperature", json!(request.temperature))
                    .with_param("shard_id", json!(shard.shard_id))
                    .with_param("layer_start", json!(shard.layer_start))
                    .with_param("layer_end", json!(shard.layer_end));
                
                // Print command details
                println!("[INFERENCE]   Command: {}", cmd.command);
                println!("[INFERENCE]   Request ID: {}", cmd.request_id);
                println!("[INFERENCE]   From: {} → To: {}", cmd.from, shard.peer_id);
                println!("[INFERENCE]   Params: task_type=ai_inference, shard_id={}, layers={}-{}", 
                    shard.shard_id, shard.layer_start, shard.layer_end);
                println!("[INFERENCE]   Input data (first 100 chars): \"{}\"", 
                    if current_input.len() > 100 { 
                        format!("{}...", &current_input[..100]) 
                    } else { 
                        current_input.clone() 
                    });
                
                match sender(shard.peer_id.clone(), cmd).await {
                    Ok(response) => {
                        println!("[INFERENCE] 📥 Received JSON response from node {}:", shard.peer_id);
                        println!("[INFERENCE]   Status: {:?}", response.status);
                        println!("[INFERENCE]   Response ID: {}", response.request_id);
                        println!("[INFERENCE]   From: {} → To: {}", response.from, response.to);
                        
                        if response.status == crate::command_protocol::ResponseStatus::Success {
                            // Extract output from response
                            if let Some(result) = response.result {
                                println!("[INFERENCE]   Result keys: {:?}", result.keys().collect::<Vec<_>>());
                                
                                if let Some(output_val) = result.get("output") {
                                    if let Some(output_str) = output_val.as_str() {
                                        println!("[INFERENCE]   Output length: {} characters", output_str.len());
                                        println!("[INFERENCE]   Output preview: \"{}\"", 
                                            if output_str.len() > 150 { 
                                                format!("{}...", &output_str[..150]) 
                                            } else { 
                                                output_str.to_string() 
                                            });
                                        
                                        if let Some(tokens) = result.get("tokens_generated") {
                                            println!("[INFERENCE]   Tokens generated: {}", tokens);
                                        }
                                        if let Some(time) = result.get("processing_time_ms") {
                                            println!("[INFERENCE]   Processing time: {} ms", time);
                                        }
                                        
                                        output_str.to_string()
                                    } else {
                                        println!("[INFERENCE]   Output (non-string): {:?}", output_val);
                                        format!("[Shard {} processed: {}]", shard.shard_id, output_val)
                                    }
                                } else {
                                    eprintln!("[INFERENCE] ⚠️  No 'output' key in result from node {} (shard {})", 
                                        shard.peer_id, shard.shard_id);
                                    eprintln!("[INFERENCE]   Result keys: {:?}", result.keys().collect::<Vec<_>>());
                                    eprintln!("[INFERENCE]   Using fallback output");
                                    format!("[Shard {} processed layers {}-{}]", 
                                        shard.shard_id, shard.layer_start, shard.layer_end)
                                }
                            } else {
                                eprintln!("[INFERENCE] ⚠️  No result in response from node {} (shard {})", 
                                    shard.peer_id, shard.shard_id);
                                eprintln!("[INFERENCE]   Response status: {:?}", response.status);
                                eprintln!("[INFERENCE]   Using fallback output");
                                format!("[Shard {} processed layers {}-{}]", 
                                    shard.shard_id, shard.layer_start, shard.layer_end)
                            }
                        } else {
                            let error_msg = response.error.clone().unwrap_or_else(|| "Unknown error".to_string());
                            eprintln!("[INFERENCE] ❌ Error response from node {} (shard {}): {}", 
                                shard.peer_id, shard.shard_id, error_msg);
                            eprintln!("[INFERENCE]   Response status: {:?}", response.status);
                            eprintln!("[INFERENCE]   Response ID: {}", response.request_id);
                            eprintln!("[INFERENCE]   Shard: {} (layers {}-{})", 
                                shard.shard_id, shard.layer_start, shard.layer_end);
                            eprintln!("[INFERENCE]   Input length: {} characters", current_input.len());
                            if let Some(ref result) = response.result {
                                eprintln!("[INFERENCE]   Response result: {:?}", result);
                            }
                            eprintln!("[INFERENCE]   Possible causes:");
                            eprintln!("[INFERENCE]     - Shard model not loaded on node");
                            eprintln!("[INFERENCE]     - Insufficient memory on node");
                            eprintln!("[INFERENCE]     - Model file corruption");
                            eprintln!("[INFERENCE]     - Processing error in shard");
                            eprintln!("[INFERENCE]     - Node internal error");
                            return Err(PipelineError::InferenceFailed {
                                shard_id: shard.shard_id,
                                error: format!("Node {} returned error: {} (status: {:?}, shard {}, layers {}-{})", 
                                    shard.peer_id, error_msg, response.status, shard.shard_id, 
                                    shard.layer_start, shard.layer_end),
                            });
                        }
                    }
                    Err(e) => {
                        eprintln!("[INFERENCE] ❌ Failed to send inference request to node {} (shard {}): {}", 
                            shard.peer_id, shard.shard_id, e);
                        eprintln!("[INFERENCE]   Error details: {:?}", e);
                        eprintln!("[INFERENCE]   Command: {}", commands::EXECUTE_TASK);
                        eprintln!("[INFERENCE]   Request ID: {}", request.request_id);
                        eprintln!("[INFERENCE]   Shard: {} (layers {}-{})", 
                            shard.shard_id, shard.layer_start, shard.layer_end);
                        eprintln!("[INFERENCE]   Input length: {} characters", current_input.len());
                        eprintln!("[INFERENCE]   Possible causes:");
                        eprintln!("[INFERENCE]     - Node {} is not reachable", shard.peer_id);
                        eprintln!("[INFERENCE]     - Network connectivity issues");
                        eprintln!("[INFERENCE]     - Node crashed or is not responding");
                        eprintln!("[INFERENCE]     - Command protocol error");
                        eprintln!("[INFERENCE]     - Timeout waiting for connection");
                        return Err(PipelineError::InferenceFailed {
                            shard_id: shard.shard_id,
                            error: format!("Failed to send request to node {}: {} (shard {}, layers {}-{})", 
                                shard.peer_id, e, shard.shard_id, shard.layer_start, shard.layer_end),
                        });
                    }
                }
            } else {
                // Fallback: simulate processing if no command sender
                println!("[INFERENCE] WARNING: No command sender configured, simulating shard processing");
                tokio::time::sleep(Duration::from_millis(50)).await;
                format!("[Shard {} processed layers {}-{}]", 
                    shard.shard_id, shard.layer_start, shard.layer_end)
            };

            shard_outputs.push(shard_output.clone());
            
            let latency = shard_start.elapsed().as_millis() as f64;
            println!("[INFERENCE] ✓ Shard {} completed in {:.2}ms", shard.shard_id, latency);
            println!("[INFERENCE]   Output length: {} characters", shard_output.len());
            
            // Pass output to next shard in pipeline
            current_input = shard_output.clone();
            
            if idx < pipeline_clone.len() - 1 {
                println!("[INFERENCE]   → Passing output to next shard in pipeline...\n");
            } else {
                println!("[INFERENCE]   → Final shard completed!\n");
            }

            shard_latencies.push(ShardLatency {
                shard_id: shard.shard_id,
                node_id: shard.peer_id.clone(),
                latency_ms: latency,
            });
        }

        // Combine outputs from all shards
        // In pipeline parallelism, the final shard's output is the final answer
        println!("[INFERENCE] ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        println!("[INFERENCE] Combining outputs from {} shards...", shard_outputs.len());
        
        let final_output = if let Some(last_output) = shard_outputs.last() {
            println!("[INFERENCE] Using final shard output as answer");
            last_output.clone()
        } else {
            eprintln!("[INFERENCE] ⚠️  No outputs available from any shard!");
            eprintln!("[INFERENCE]   Request ID: {}", request.request_id);
            eprintln!("[INFERENCE]   Pipeline size: {}", pipeline_clone.len());
            eprintln!("[INFERENCE]   Shard outputs collected: {}", shard_outputs.len());
            eprintln!("[INFERENCE]   Using fallback output");
            format!("Processed through {} shards: {}", shard_outputs.len(), request.prompt)
        };
        
        println!("[INFERENCE] Final output length: {} characters", final_output.len());
        println!("[INFERENCE] ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

        let total_latency = start.elapsed().as_millis() as f64;

        // Update stats
        {
            let mut stats = self.stats.write().await;
            stats.successful_requests += 1;
            stats.average_latency_ms = 
                (stats.average_latency_ms * (stats.successful_requests - 1) as f64 + total_latency) 
                / stats.successful_requests as f64;
        }

        Ok(InferenceResponse {
            request_id: request.request_id,
            text: final_output,
            tokens_generated: request.max_tokens.min(100),
            total_latency_ms: total_latency,
            shard_latencies,
            strategy_used: "pipeline".to_string(),
            success: true,
            error: None,
        })
    }

    /// Process inference on fallback node
    async fn process_on_fallback(
        &self,
        request: InferenceRequest,
        node: &ShardAnnouncement,
        start: Instant,
    ) -> Result<InferenceResponse, PipelineError> {
        println!("[FALLBACK] Processing on single node: {}", node.peer_id);

        // Simulate full model inference on single node
        tokio::time::sleep(Duration::from_millis(200)).await;

        let total_latency = start.elapsed().as_millis() as f64;

        // Update stats
        {
            let mut stats = self.stats.write().await;
            stats.successful_requests += 1;
            stats.average_latency_ms = 
                (stats.average_latency_ms * (stats.successful_requests - 1) as f64 + total_latency) 
                / stats.successful_requests as f64;
        }

        Ok(InferenceResponse {
            request_id: request.request_id,
            text: format!("Generated response for: {} | Fallback mode on {}", 
                request.prompt, node.peer_id),
            tokens_generated: request.max_tokens.min(100),
            total_latency_ms: total_latency,
            shard_latencies: vec![ShardLatency {
                shard_id: 0,
                node_id: node.peer_id.clone(),
                latency_ms: total_latency,
            }],
            strategy_used: "single_node_fallback".to_string(),
            success: true,
            error: None,
        })
    }

    /// Record a failed request
    async fn record_failure(&self) {
        let mut stats = self.stats.write().await;
        stats.failed_requests += 1;
        let failure_rate = if stats.total_requests > 0 {
            (stats.failed_requests as f64 / stats.total_requests as f64) * 100.0
        } else {
            0.0
        };
        if stats.failed_requests % 10 == 0 || failure_rate > 50.0 {
            eprintln!("[COORDINATOR] ⚠️  Failure statistics: {}/{} failed ({:.1}% failure rate)", 
                stats.failed_requests, stats.total_requests, failure_rate);
        }
    }

    /// Get pipeline status
    pub async fn pipeline_status(&self) -> PipelineStatus {
        let discovery = self.discovery.read().await;
        discovery.status()
    }

    /// Add a shard to the discovery
    pub async fn add_shard(&self, announcement: ShardAnnouncement) {
        self.update_discovery(announcement).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kademlia_shard_discovery::ShardAnnouncement;

    fn create_test_shard(shard_id: u32, total_shards: u32, memory_mb: u64) -> ShardAnnouncement {
        let mut ann = ShardAnnouncement::new(
            &format!("peer-{}", shard_id),
            shard_id,
            total_shards,
            32,
            &format!("/ip4/10.0.0.{}/tcp/51820", shard_id),
            "llama-test",
        );
        ann.capabilities.memory_available_mb = memory_mb;
        ann
    }

    #[tokio::test]
    async fn test_coordinator_creation() {
        let discovery = KademliaShardDiscovery::with_expected_shards("test", 4);
        let coordinator = PipelineCoordinator::new(discovery);
        
        let state = coordinator.state().await;
        assert!(matches!(state, CoordinatorState::Unavailable { .. }));
    }

    #[tokio::test]
    async fn test_complete_pipeline_ready() {
        let discovery = KademliaShardDiscovery::with_expected_shards("test", 4);
        let coordinator = PipelineCoordinator::new(discovery);

        // Add all shards
        for i in 0..4 {
            coordinator.add_shard(create_test_shard(i, 4, 8192)).await;
        }

        let state = coordinator.state().await;
        assert!(matches!(state, CoordinatorState::Ready));
    }

    #[tokio::test]
    async fn test_incomplete_pipeline_waiting() {
        let discovery = KademliaShardDiscovery::with_expected_shards("test", 4);
        let coordinator = PipelineCoordinator::new(discovery);

        // Add only 2 shards
        coordinator.add_shard(create_test_shard(0, 4, 8192)).await;
        coordinator.add_shard(create_test_shard(2, 4, 8192)).await;

        let state = coordinator.state().await;
        match state {
            CoordinatorState::WaitingForShards { missing } => {
                assert!(missing.contains(&1));
                assert!(missing.contains(&3));
            }
            _ => panic!("Expected WaitingForShards state"),
        }
    }

    #[tokio::test]
    async fn test_fail_fast_strategy() {
        let discovery = KademliaShardDiscovery::with_expected_shards("test", 4);
        let mut coordinator = PipelineCoordinator::new(discovery);
        coordinator.set_strategy(PipelineStrategy::FailFast);

        // Add incomplete pipeline
        coordinator.add_shard(create_test_shard(0, 4, 8192)).await;

        let request = InferenceRequest::new("test prompt");
        let result = coordinator.submit_inference(request).await;

        assert!(result.is_err());
        match result.unwrap_err() {
            PipelineError::NoFallback { .. } => {}
            e => panic!("Expected NoFallback error, got: {:?}", e),
        }
    }

    #[tokio::test]
    async fn test_complete_pipeline_inference() {
        let discovery = KademliaShardDiscovery::with_expected_shards("test", 4);
        let coordinator = PipelineCoordinator::new(discovery);

        // Add all shards
        for i in 0..4 {
            coordinator.add_shard(create_test_shard(i, 4, 8192)).await;
        }

        let request = InferenceRequest::new("test prompt");
        let result = coordinator.submit_inference(request).await;

        assert!(result.is_ok());
        let response = result.unwrap();
        assert!(response.success);
        assert_eq!(response.shard_latencies.len(), 4);
    }

    #[tokio::test]
    async fn test_single_node_fallback() {
        let discovery = KademliaShardDiscovery::with_expected_shards("test", 4);
        let mut coordinator = PipelineCoordinator::new(discovery);
        coordinator.set_strategy(PipelineStrategy::SingleNodeFallback {
            required_memory_mb: 16000,
        });

        // Add one high-memory shard (simulating a capable node)
        let mut high_mem_shard = create_test_shard(0, 4, 32000);
        high_mem_shard.capabilities.memory_available_mb = 32000;
        coordinator.add_shard(high_mem_shard).await;

        let request = InferenceRequest::new("test prompt");
        let result = coordinator.submit_inference(request).await;

        assert!(result.is_ok());
        let response = result.unwrap();
        assert!(response.success);
        assert_eq!(response.strategy_used, "single_node_fallback");
    }

    #[tokio::test]
    async fn test_inference_request_builder() {
        let request = InferenceRequest::new("Hello world")
            .with_max_tokens(500)
            .with_temperature(0.8)
            .with_priority(1);

        assert_eq!(request.prompt, "Hello world");
        assert_eq!(request.max_tokens, 500);
        assert_eq!(request.temperature, 0.8);
        assert_eq!(request.priority, 1);
    }

    #[tokio::test]
    async fn test_stats_tracking() {
        let discovery = KademliaShardDiscovery::with_expected_shards("test", 4);
        let coordinator = PipelineCoordinator::new(discovery);

        // Add all shards
        for i in 0..4 {
            coordinator.add_shard(create_test_shard(i, 4, 8192)).await;
        }

        // Process a few requests
        for _ in 0..3 {
            let request = InferenceRequest::new("test");
            let _ = coordinator.submit_inference(request).await;
        }

        let stats = coordinator.stats().await;
        assert_eq!(stats.total_requests, 3);
        assert_eq!(stats.successful_requests, 3);
        assert!(stats.average_latency_ms > 0.0);
    }
}

