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
use crate::llama_model_loader::{LlamaModelManager, RsyncConfig};
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
            Ok(mut child) => {
                println!("[SPAWNER] ✓ Spawned shard_listener process for shard {} (PID: {:?})", shard_id, child.id());
                
                // Store process handle
                let mut spawned = self.spawned_nodes.write().await;
                spawned.insert(shard_id, child);
                
                Ok(())
            }
            Err(e) => {
                eprintln!("[SPAWNER] Failed to spawn node for shard {}: {}", shard_id, e);
                Err(PipelineError::ShardLoadFailed {
                    shard_id,
                    error: format!("Failed to spawn node: {}", e),
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

        while Instant::now() < deadline {
            // Check if shard is now available in discovery
            let pipeline: Vec<ShardAnnouncement> = {
                let disc = discovery.read().await;
                disc.get_pipeline().into_iter().cloned().collect()
            };

            // Check if our shard is in the pipeline
            if pipeline.iter().any(|s| s.shard_id == shard_id) {
                println!("[SPAWNER] ✓ Shard {} node is online and discovered!", shard_id);
                return Ok(());
            }

            // Check if process handle exists (we can't check status without mutable access)
            // The process will be cleaned up when we try to wait on it later
            let _process_exists = {
                let spawned = self.spawned_nodes.read().await;
                spawned.contains_key(&shard_id)
            };

            tokio::time::sleep(check_interval).await;
        }

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
            if let Err(e) = child.kill().await {
                eprintln!("[SPAWNER] Failed to terminate node for shard {}: {}", shard_id, e);
                return Err(PipelineError::Internal {
                    message: format!("Failed to terminate node: {}", e),
                });
            }
            println!("[SPAWNER] ✓ Terminated node for shard {}", shard_id);
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
            match sender(node_id.to_string(), cmd).await {
                Ok(response) => {
                    if response.status == crate::command_protocol::ResponseStatus::Success {
                        println!("[LOADER] ✓ Node {} confirmed shard {} loaded", node_id, shard_id);
                        
                        // Track loaded shard
                        let mut loaded = self.loaded_shards.write().await;
                        loaded.entry(node_id.to_string())
                            .or_insert_with(Vec::new)
                            .push(shard_id);
                        
                        Ok(())
                    } else {
                        let error_msg = response.error.unwrap_or_else(|| "Unknown error".to_string());
                        Err(PipelineError::ShardLoadFailed {
                            shard_id,
                            error: format!("Node {} returned error: {}", node_id, error_msg),
                        })
                    }
                }
                Err(e) => {
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
            let shard_name = format!("{}-shard-{}.safetensors", model_name, shard_id);
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
        
        nodes.iter()
            .filter(|n| {
                let current_shards = loaded.get(&n.peer_id).map(|v| v.len()).unwrap_or(0);
                n.capabilities.memory_available_mb >= min_memory_mb 
                    && (current_shards as u32) < max_shards_per_node
            })
            .cloned()
            .collect()
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
pub struct PipelineCoordinator {
    /// Shard discovery service
    discovery: Arc<RwLock<KademliaShardDiscovery>>,
    /// Current strategy
    strategy: PipelineStrategy,
    /// Current state
    state: Arc<RwLock<CoordinatorState>>,
    /// Request queue
    request_queue: Arc<Mutex<VecDeque<QueuedRequest>>>,
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
            shard_loader: None,
            node_spawner: None,
            fallback: Arc::new(RwLock::new(SingleNodeFallback::new())),
            stats: Arc::new(RwLock::new(CoordinatorStats::default())),
            shutdown_tx: None,
        }
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

        // Spawn nodes for each missing shard
        for shard_id in &missing_shards {
            println!("[COORDINATOR] Spawning node for shard {}...", shard_id);
            if let Err(e) = spawner.spawn_node_for_shard(*shard_id).await {
                eprintln!("[COORDINATOR] Failed to spawn node for shard {}: {}", shard_id, e);
                // Continue with other shards
            } else {
                println!("[COORDINATOR] ✓ Spawned node for shard {}", shard_id);
            }
        }

        // Wait for nodes to come online
        println!("[COORDINATOR] Waiting for spawned nodes to come online...");
        for shard_id in &missing_shards {
            println!("[COORDINATOR] Waiting for shard {} node to come online...", shard_id);
            if let Err(e) = spawner.wait_for_node_online(*shard_id, 30, &self.discovery).await {
                eprintln!("[COORDINATOR] ⚠️  Shard {} node did not come online in time: {}", shard_id, e);
                // Continue - node might still be starting
            } else {
                println!("[COORDINATOR] ✓ Shard {} node is online", shard_id);
            }
        }

        // Final check
        let discovery = self.discovery.read().await;
        let status = discovery.status();
        drop(discovery);

        if status.is_complete {
            println!("[COORDINATOR] ✓ All nodes are online and pipeline is complete!");
            Ok(())
        } else {
            let still_missing = status.missing_shards;
            println!("[COORDINATOR] ⚠️  Pipeline still incomplete. Missing: {:?}", still_missing);
            println!("[COORDINATOR] Nodes may still be starting up. They will be used when ready.");
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
        let discovery = self.discovery.read().await;
        let status = discovery.status();
        let pipeline = discovery.get_pipeline();
        let online_nodes = pipeline.len() as u32;
        let missing_shards = discovery.get_missing_shards();
        let is_complete = status.is_complete;
        drop(discovery);
        (online_nodes, 4, missing_shards, is_complete) // 4 = total expected shards
    }

    /// Update discovery with new shard information
    pub async fn update_discovery(&self, announcement: ShardAnnouncement) {
        let mut discovery = self.discovery.write().await;
        discovery.add_shard(announcement);
        drop(discovery);
        
        // Check if this completes the pipeline
        self.update_state().await;
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
        drop(discovery);

        if status.is_complete {
            // Pipeline ready - process immediately
            return self.process_inference(request, start).await;
        }

        // Pipeline incomplete - apply strategy
        match &self.strategy {
            PipelineStrategy::FailFast => {
                self.record_failure().await;
                Err(PipelineError::NoFallback {
                    reason: format!("Pipeline incomplete, missing shards: {:?}", status.missing_shards),
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
        drop(discovery);

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
            self.record_failure().await;
            return Err(PipelineError::NoFallback {
                reason: "No nodes with sufficient capacity for dynamic loading".to_string(),
            });
        }

        // Try to load each missing shard
        for shard_id in missing_shards {
            // Round-robin across capable nodes
            let node_idx = (*shard_id as usize) % capable_nodes.len();
            let node = capable_nodes[node_idx];

            if let Err(e) = loader.load_shard_on_node(&node.peer_id, *shard_id, &node.model_name).await {
                self.record_failure().await;
                return Err(e);
            }

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
            self.record_failure().await;
            Err(PipelineError::NoFallback {
                reason: format!("No node with {}MB+ memory available", required_memory_mb),
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
                Ok(response) => return Ok(response),
                Err(e) => println!("[COORDINATOR] Dynamic loading failed: {}", e),
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
            Ok(response) => return Ok(response),
            Err(e) => println!("[COORDINATOR] Wait failed: {}", e),
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
                Ok(response) => return Ok(response),
                Err(e) => println!("[COORDINATOR] Node spawning failed: {}", e),
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
            if let Err(e) = spawner.spawn_node_for_shard(*shard_id).await {
                eprintln!("[COORDINATOR] Failed to spawn node for shard {}: {}", shard_id, e);
                self.record_failure().await;
                return Err(e);
            }
        }

        // Wait for nodes to come online
        for shard_id in &shards_to_spawn {
            if let Err(e) = spawner.wait_for_node_online(
                *shard_id,
                node_startup_timeout_secs,
                &self.discovery,
            ).await {
                eprintln!("[COORDINATOR] Node for shard {} failed to come online: {}", shard_id, e);
                self.record_failure().await;
                return Err(e);
            }
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
        println!("[INFERENCE] Processing request: {}", request.request_id);

        let discovery = self.discovery.read().await;
        let pipeline = discovery.get_pipeline();
        
        if pipeline.is_empty() {
            drop(discovery);
            self.record_failure().await;
            return Err(PipelineError::Internal {
                message: "Pipeline is empty".to_string(),
            });
        }

        let mut shard_latencies = Vec::new();
        let mut current_activations = request.prompt.clone();

        // Process through each shard in sequence
        for shard in &pipeline {
            let shard_start = Instant::now();
            
            println!("[INFERENCE] Processing shard {} (layers {}-{}) on {}",
                shard.shard_id,
                shard.layer_start,
                shard.layer_end,
                shard.peer_id
            );

            // Simulate shard processing
            // In production, this would send the activations to the shard node
            tokio::time::sleep(Duration::from_millis(50)).await;
            
            current_activations = format!("{} [processed by shard {}]", current_activations, shard.shard_id);

            shard_latencies.push(ShardLatency {
                shard_id: shard.shard_id,
                node_id: shard.peer_id.clone(),
                latency_ms: shard_start.elapsed().as_millis() as f64,
            });
        }

        drop(discovery);

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
            text: format!("Generated response for: {} | Pipeline: {} shards", 
                request.prompt, shard_latencies.len()),
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

