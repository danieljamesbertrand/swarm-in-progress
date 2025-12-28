
//! Promethos-AI Web Server
//! 
//! WebSocket server that connects the web console to the Llama inference engine.
//! 
//! Run with: cargo run --bin web_server
//! Then open: http://localhost:8080

use std::net::SocketAddr;
use std::sync::Arc;
use std::collections::HashMap;
use tokio::sync::{RwLock, Mutex, oneshot};
use tokio::net::{TcpListener, TcpStream};
use tokio::time::{Duration, Instant};
use tokio_tungstenite::{accept_async, tungstenite::Message};
use futures_util::{StreamExt, SinkExt};
use serde::{Deserialize, Serialize};
use punch_simple::pipeline_coordinator::{PipelineCoordinator, InferenceRequest, PipelineStrategy, NodeSpawner};
use punch_simple::kademlia_shard_discovery::{KademliaShardDiscovery, dht_keys};
use punch_simple::message::{JsonCodec, JsonMessage};
use punch_simple::command_validation::validate_command;
use libp2p::{
    identity,
    tcp,
    noise,
    yamux,
    kad,
    ping,
    request_response::{self, ProtocolSupport},
    swarm::{Swarm, SwarmEvent},
    core::transport::Transport,
    PeerId, Multiaddr, StreamProtocol,
};
use libp2p::swarm::Config as SwarmConfig;

/// Query request from web client
#[derive(Deserialize)]
struct QueryRequest {
    query: String,
    #[serde(default)]
    request_id: Option<String>,
}

/// Node control request from web client
#[derive(Deserialize, Clone)]
struct NodeControlRequest {
    #[serde(rename = "type")]
    message_type: String,
    node_id: String,
    command: String,
    params: Option<HashMap<String, serde_json::Value>>,
    #[serde(default)]
    request_id: Option<String>,
}

/// Node control response to web client
#[derive(Serialize)]
struct NodeControlResponse {
    #[serde(rename = "type")]
    message_type: String,
    node_id: String,
    command: String,
    success: bool,
    result: Option<serde_json::Value>,
    error: Option<String>,
    #[serde(default)]
    request_id: Option<String>,
}

/// Response to web client
#[derive(Serialize)]
struct QueryResponse {
    response: String,
    tokens: usize,
    latency_ms: u64,
    shards_used: Vec<ShardInfo>,
    success: bool,
    request_id: Option<String>,
}

/// Pipeline status update
#[derive(Serialize)]
struct PipelineUpdate {
    stage: String,
    status: String, // "waiting", "processing", "complete", "error"
    shard_id: Option<u32>,
    latency_ms: Option<u64>,
}

/// Node inference request message (for scrolling log)
#[derive(Serialize, Clone)]
struct NodeInferenceRequestMessage {
    #[serde(rename = "type")]
    message_type: String,
    node_id: String,
    shard_id: u32,
    request_id: String,
    timestamp: u64,
    input_preview: String, // First 100 chars of input
    layers: String, // "0-7" format
}

/// Node communication event message
#[derive(Serialize, Clone)]
struct NodeEventMessage {
    #[serde(rename = "type")]
    message_type: String,
    event: String, // "command_sent", "command_received", "command_error", "node_joined", "shard_loaded"
    node_id: String,
    shard_id: Option<u32>,
    command: Option<String>,
    status: Option<String>,
    latency_ms: Option<u64>,
    timestamp: u64,
    details: Option<String>,
}

/// Node status update from node to web server
#[derive(Serialize, Deserialize, Clone, Debug)]
struct NodeStatusUpdate {
    #[serde(rename = "type")]
    message_type: String,
    node_id: String,
    timestamp: u64,
    status: NodeStatus,
    operation_mode: OperationMode,
    capabilities: Option<NodeCapabilitiesInfo>,
    shard_id: Option<u32>,
    active_requests: u32,
    total_requests: u64,
    last_error: Option<String>,
}

/// Node status enum
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum NodeStatus {
    Online,
    Offline,
    Busy,
    Idle,
    Error,
    Loading,
}

/// Operation mode for nodes
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum OperationMode {
    Normal,      // Normal inference processing
    Maintenance, // Maintenance mode - no new requests
    Standby,     // Standby - ready but not processing
    Shutdown,    // Shutting down
}

/// Node capabilities info (simplified for status updates)
#[derive(Serialize, Deserialize, Clone, Debug)]
struct NodeCapabilitiesInfo {
    cpu_usage: f64,
    memory_usage_mb: u64,
    memory_available_mb: u64,
    gpu_available: bool,
    gpu_usage: Option<f64>,
    latency_ms: f64,
}

/// Control command from web server to node
#[derive(Serialize, Deserialize, Clone, Debug)]
struct NodeControlCommand {
    #[serde(rename = "type")]
    message_type: String,
    command: ControlCommandType,
    node_id: String,
    request_id: String,
    timestamp: u64,
    params: HashMap<String, serde_json::Value>,
}

/// Control command types
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum ControlCommandType {
    SetOperationMode,
    GetStatus,
    GetCapabilities,
    Pause,
    Resume,
    Restart,
    Shutdown,
    UpdateConfig,
}

/// Per-node message queue structure
struct NodeQueue {
    node_id: String,
    // Current node state
    current_status: Arc<RwLock<NodeStatusUpdate>>,
    // Last seen timestamp
    last_seen: Arc<RwLock<u64>>,
    // Pending commands waiting for response
    pending_commands: Arc<Mutex<HashMap<String, tokio::sync::oneshot::Sender<serde_json::Value>>>>,
}

/// Node queue manager - tracks all nodes and their message queues
struct NodeQueueManager {
    // Map of node_id -> NodeQueue
    nodes: Arc<RwLock<HashMap<String, NodeQueue>>>,
    // Broadcast channel for status updates to all WebSocket clients
    status_broadcast_tx: tokio::sync::broadcast::Sender<NodeStatusUpdate>,
}

/// Pipeline status message sent to web client
#[derive(Serialize)]
struct PipelineStatusMessage {
    #[serde(rename = "type")]
    message_type: String,
    total_nodes: u32,
    online_nodes: u32,
    missing_shards: Vec<u32>,
    is_complete: bool,
}

/// Ready status message - sent when pipeline is ready for processing
#[derive(Serialize)]
struct ReadyStatusMessage {
    #[serde(rename = "type")]
    message_type: String,
    ready: bool,
    online_nodes: u32,
    total_nodes: u32,
}

/// Metrics message sent to web client
#[derive(Serialize)]
struct MetricsMessage {
    #[serde(rename = "type")]
    message_type: String,
    metrics: SystemMetrics,
}

/// Shard info for response
#[derive(Serialize, Clone)]
struct ShardInfo {
    shard_id: u32,
    layer_start: u32,
    layer_end: u32,
    latency_ms: u64,
}

/// Simulated shard node
struct ShardNode {
    shard_id: u32,
    layer_start: u32,
    layer_end: u32,
    has_embeddings: bool,
    has_output: bool,
}

impl ShardNode {
    fn new(shard_id: u32, total_shards: u32, total_layers: u32) -> Self {
        let layers_per_shard = total_layers / total_shards;
        let layer_start = shard_id * layers_per_shard;
        let layer_end = if shard_id == total_shards - 1 {
            total_layers
        } else {
            (shard_id + 1) * layers_per_shard
        };
        
        Self {
            shard_id,
            layer_start,
            layer_end,
            has_embeddings: shard_id == 0,
            has_output: shard_id == total_shards - 1,
        }
    }
}

/// Metrics for tracking node events and communications
#[derive(Clone, Serialize, Deserialize, Debug, Default)]
struct SystemMetrics {
    // Node metrics
    total_nodes_joined: u64,
    nodes_online: u32,
    nodes_offline: u32,
    node_join_events: Vec<NodeJoinEvent>,
    
    // Shard metrics
    total_shards_loaded: u64,
    shards_loading: u32,
    shards_available: u32,
    shard_load_events: Vec<ShardLoadEvent>,
    
    // Communication metrics
    commands_sent: u64,
    commands_received: u64,
    command_errors: u64,
    avg_command_latency_ms: f64,
    command_latency_samples: Vec<f64>,
    bytes_sent: u64,
    bytes_received: u64,
    
    // Inference metrics
    inference_requests: u64,
    inference_successes: u64,
    inference_failures: u64,
    avg_inference_latency_ms: f64,
    
    // Timestamp
    last_updated: u64,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
struct NodeJoinEvent {
    timestamp: u64,
    peer_id: String,
    shard_id: Option<u32>,
    multiaddr: Option<String>,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
struct ShardLoadEvent {
    timestamp: u64,
    peer_id: String,
    shard_id: u32,
    status: String, // "loading", "loaded", "failed"
    duration_ms: Option<u64>,
}

// Define DiscoveryBehaviour outside so it can be used in struct
#[derive(libp2p::swarm::NetworkBehaviour)]
struct DiscoveryBehaviour {
    kademlia: kad::Behaviour<kad::store::MemoryStore>,
    identify: libp2p::identify::Behaviour,
    ping: ping::Behaviour,
    request_response: request_response::Behaviour<JsonCodec>,
}

impl NodeQueueManager {
    fn new() -> (Self, tokio::sync::broadcast::Receiver<NodeStatusUpdate>) {
        let (status_broadcast_tx, status_broadcast_rx) = tokio::sync::broadcast::channel(128);
        (
            Self {
                nodes: Arc::new(RwLock::new(HashMap::new())),
                status_broadcast_tx,
            },
            status_broadcast_rx,
        )
    }

    /// Register a new node and create its message queues
    async fn register_node(&self, node_id: String, shard_id: Option<u32>) -> Result<(), String> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        let initial_status = NodeStatusUpdate {
            message_type: "node_status".to_string(),
            node_id: node_id.clone(),
            timestamp: now,
            status: NodeStatus::Online,
            operation_mode: OperationMode::Normal,
            capabilities: None,
            shard_id,
            active_requests: 0,
            total_requests: 0,
            last_error: None,
        };
        
        let node_queue = NodeQueue {
            node_id: node_id.clone(),
            current_status: Arc::new(RwLock::new(initial_status.clone())),
            last_seen: Arc::new(RwLock::new(now)),
            pending_commands: Arc::new(Mutex::new(HashMap::new())),
        };
        
        let mut nodes = self.nodes.write().await;
        nodes.insert(node_id.clone(), node_queue);
        drop(nodes);
        
        // Broadcast initial status
        let _ = self.status_broadcast_tx.send(initial_status);
        
        println!("[QUEUE] Registered node {} with dedicated message queues", node_id);
        Ok(())
    }

    /// Send a control command to a specific node (returns command for P2P sending)
    fn create_control_command(&self, node_id: &str, command: ControlCommandType, params: HashMap<String, serde_json::Value>) -> Result<NodeControlCommand, String> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        Ok(NodeControlCommand {
            message_type: "node_control".to_string(),
            command,
            node_id: node_id.to_string(),
            request_id: format!("cmd-{}", now),
            timestamp: now,
            params,
        })
    }

    /// Get current status of a node
    async fn get_node_status(&self, node_id: &str) -> Option<NodeStatusUpdate> {
        let nodes = self.nodes.read().await;
        nodes.get(node_id).map(|nq| {
            // Clone the current status
            nq.current_status.blocking_read().clone()
        })
    }

    /// Get all registered nodes
    async fn list_nodes(&self) -> Vec<String> {
        let nodes = self.nodes.read().await;
        nodes.keys().cloned().collect()
    }

    /// Update node status (called when receiving status from node via P2P)
    async fn update_node_status(&self, status: NodeStatusUpdate) {
        let node_id = status.node_id.clone();
        
        // Update stored status
        {
            let nodes = self.nodes.read().await;
            if let Some(node_queue) = nodes.get(&node_id) {
                *node_queue.current_status.write().await = status.clone();
                *node_queue.last_seen.write().await = status.timestamp;
            } else {
                // Auto-register node if not found
                drop(nodes);
                let _ = self.register_node(node_id.clone(), status.shard_id).await;
                let nodes = self.nodes.read().await;
                if let Some(node_queue) = nodes.get(&node_id) {
                    *node_queue.current_status.write().await = status.clone();
                    *node_queue.last_seen.write().await = status.timestamp;
                }
            }
        }
        
        // Broadcast to all WebSocket clients
        let _ = self.status_broadcast_tx.send(status);
    }

    /// Check if node is registered
    async fn is_node_registered(&self, node_id: &str) -> bool {
        let nodes = self.nodes.read().await;
        nodes.contains_key(node_id)
    }

    /// Remove a node (when it disconnects)
    async fn unregister_node(&self, node_id: &str) {
        let mut nodes = self.nodes.write().await;
        if nodes.remove(node_id).is_some() {
            println!("[QUEUE] Unregistered node {}", node_id);
        }
    }

    /// Get broadcast sender for status updates
    fn status_broadcast_sender(&self) -> tokio::sync::broadcast::Sender<NodeStatusUpdate> {
        self.status_broadcast_tx.clone()
    }
}

/// The inference engine - uses real distributed pipeline
struct InferenceEngine {
    coordinator: Arc<PipelineCoordinator>,
    peer_id: PeerId,
    swarm: Arc<Mutex<Swarm<DiscoveryBehaviour>>>,
    // Store pending responses - using command request_id string as key for reliable matching
    pending_responses: Arc<Mutex<HashMap<String, oneshot::Sender<punch_simple::command_protocol::CommandResponse>>>>,
    discovery_task: Arc<tokio::task::JoinHandle<()>>,
    metrics: Arc<RwLock<SystemMetrics>>,
    // Channel for broadcasting node events to all WebSocket connections
    node_event_tx: Option<tokio::sync::broadcast::Sender<NodeEventMessage>>,
    // Node queue manager for dedicated per-node communication
    node_queue_manager: Arc<NodeQueueManager>,
}

impl InferenceEngine {
    async fn new(
        bootstrap: &str, 
        node_request_tx: Option<tokio::sync::broadcast::Sender<NodeInferenceRequestMessage>>,
        node_event_tx: Option<tokio::sync::broadcast::Sender<NodeEventMessage>>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        // Generate peer identity
        let key = identity::Keypair::generate_ed25519();
        let peer_id = PeerId::from(key.public());
        
        // Create metrics first
        let metrics = Arc::new(RwLock::new(SystemMetrics::default()));
        
        // Create discovery
        let mut discovery = KademliaShardDiscovery::with_expected_shards("llama-cluster", 4);
        // Set local peer ID for distance calculations
        discovery.set_local_peer_id(peer_id.to_string());
        
        // Create node spawner for on-demand node creation
        let spawner = NodeSpawner::new(
            bootstrap.to_string(),
            "llama-cluster".to_string(),
            4,  // total_shards
            32, // total_layers
            "llama-8b".to_string(),
            "models_cache/shards".to_string(),
        );

        // Create pipeline coordinator with spawner and strategy
        let mut coordinator = PipelineCoordinator::new(discovery)
            .with_node_spawner(spawner);
        coordinator.set_strategy(PipelineStrategy::Adaptive {
            wait_timeout_secs: 30,
            min_memory_for_shard_mb: 4096,
            min_memory_for_full_mb: 16384,
        });
        
        // Create P2P swarm for command sending and discovery
        let transport = tcp::tokio::Transport::default()
            .upgrade(libp2p::core::upgrade::Version::V1)
            .authenticate(noise::Config::new(&key).unwrap())
            .multiplex(yamux::Config::default())
            .boxed();

        // Kademlia DHT - Large timeout for reliable discovery
        let store = kad::store::MemoryStore::new(peer_id);
        let mut kademlia_config = kad::Config::default();
        kademlia_config.set_query_timeout(Duration::from_secs(120)); // Large timeout for reliable DHT operations
        let kademlia = kad::Behaviour::with_config(peer_id, store, kademlia_config);

        // Identify
        let identify = libp2p::identify::Behaviour::new(
            libp2p::identify::Config::new("web-server/1.0".to_string(), key.public())
        );

        // Ping protocol for connection keepalive (sends pings every 25 seconds)
        let ping = ping::Behaviour::new(
            ping::Config::new()
                .with_interval(Duration::from_secs(25)) // Ping every 25 seconds
                .with_timeout(Duration::from_secs(10)), // 10 second timeout
        );

        // Request-Response
        let request_response = request_response::Behaviour::with_codec(
            JsonCodec,
            [(StreamProtocol::new("/json-message/1.0"), ProtocolSupport::Full)],
            request_response::Config::default(),
        );

        let behaviour = DiscoveryBehaviour {
            kademlia,
            identify,
            ping,
            request_response,
        };

        // Swarm - Increased idle timeout since ping keeps connections alive
        let swarm_config = SwarmConfig::with_tokio_executor()
            .with_idle_connection_timeout(Duration::from_secs(90));
        let swarm = Swarm::new(transport, behaviour, peer_id, swarm_config);
        
        // Listen on ephemeral port
        let swarm_arc = Arc::new(Mutex::new(swarm));
        {
            let mut swarm = swarm_arc.lock().await;
            if let Err(e) = swarm.listen_on("/ip4/0.0.0.0/tcp/0".parse().unwrap()) {
                eprintln!("[SERVER] Failed to listen: {}", e);
            }
        }
        
        // Create pending responses map - using command request_id string as key
        let pending_responses: Arc<Mutex<HashMap<String, oneshot::Sender<punch_simple::command_protocol::CommandResponse>>>> = Arc::new(Mutex::new(HashMap::new()));
        
        // Create P2P command sender with real P2P communication
        let metrics_for_sender = Arc::clone(&metrics);
        let swarm_for_sender = Arc::clone(&swarm_arc);
        let pending_for_sender = Arc::clone(&pending_responses);
        let sender_peer_id = peer_id;
        let node_request_tx_for_sender = node_request_tx.clone();
        let node_event_tx_for_sender = node_event_tx.clone();
        let command_sender = move |peer_id_str: String, cmd: punch_simple::command_protocol::Command| -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<punch_simple::command_protocol::CommandResponse, punch_simple::PipelineError>> + Send>> {
            let metrics_clone = Arc::clone(&metrics_for_sender);
            let swarm_clone = Arc::clone(&swarm_for_sender);
            let pending_clone = Arc::clone(&pending_for_sender);
            let sender_peer_id_clone = sender_peer_id;
            let node_request_tx_clone = node_request_tx_for_sender.clone();
            let node_event_tx_clone = node_event_tx_for_sender.clone();
            Box::pin(async move {
                let cmd_start = Instant::now();
                let is_load_shard = cmd.command == punch_simple::command_protocol::commands::LOAD_SHARD;
                let is_execute_task = cmd.command == punch_simple::command_protocol::commands::EXECUTE_TASK;
                let shard_id = cmd.params.get("shard_id").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
                
                // Send node inference request message for EXECUTE_TASK commands
                if is_execute_task {
                    if let Some(ref tx) = node_request_tx_clone {
                        let input_data = cmd.params.get("input_data")
                            .and_then(|v| v.as_str())
                            .unwrap_or("");
                        let input_preview = if input_data.len() > 100 {
                            format!("{}...", &input_data[..100])
                        } else {
                            input_data.to_string()
                        };
                        let layer_start = cmd.params.get("layer_start").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
                        let layer_end = cmd.params.get("layer_end").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
                        let layers = format!("{}-{}", layer_start, layer_end);
                        
                        let node_request_msg = NodeInferenceRequestMessage {
                            message_type: "node_inference_request".to_string(),
                            node_id: peer_id_str.clone(),
                            shard_id,
                            request_id: cmd.request_id.clone(),
                            timestamp: std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap()
                                .as_secs(),
                            input_preview,
                            layers,
                        };
                        
                        // Use broadcast send (non-blocking, doesn't need await)
                        if let Err(e) = tx.send(node_request_msg) {
                            eprintln!("[P2P] Failed to send node inference request message: {}", e);
                        } else {
                            println!("[P2P] 📤 Sent node inference request message for node {} (shard {})", peer_id_str, shard_id);
                        }
                    }
                }
                
                // Record command sent
                {
                    let mut m = metrics_clone.write().await;
                    m.commands_sent += 1;
                    let cmd_size = serde_json::to_string(&cmd).map(|s| s.len() as u64).unwrap_or(0);
                    m.bytes_sent += cmd_size;
                    
                    // Record shard loading start
                    if is_load_shard {
                        m.shards_loading += 1;
                        m.shard_load_events.push(ShardLoadEvent {
                            timestamp: std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap()
                                .as_secs(),
                            peer_id: peer_id_str.clone(),
                            shard_id,
                            status: "loading".to_string(),
                            duration_ms: None,
                        });
                    }
                }
                
                // Parse peer ID
                let target_peer: PeerId = match peer_id_str.parse() {
                    Ok(pid) => pid,
                    Err(e) => {
                        eprintln!("[P2P] Failed to parse peer ID {}: {}", peer_id_str, e);
                        return Err(punch_simple::PipelineError::Internal { 
                            message: format!("Invalid peer ID: {}", peer_id_str) 
                        });
                    }
                };
                
                // Serialize command to JSON
                let cmd_json = match serde_json::to_string(&cmd) {
                    Ok(json) => json,
                    Err(e) => {
                        eprintln!("[P2P] Failed to serialize command: {}", e);
                        return Err(punch_simple::PipelineError::Internal { 
                            message: format!("Serialization error: {}", e) 
                        });
                    }
                };
                
                // Create JsonMessage
                let msg = JsonMessage::new(sender_peer_id_clone.to_string(), cmd_json);
                
                // Send request via P2P
                let (tx, rx) = oneshot::channel();
                let cmd_request_id = cmd.request_id.clone();
                {
                    let mut swarm = swarm_clone.lock().await;
                    let _libp2p_request_id = swarm.behaviour_mut().request_response.send_request(&target_peer, msg);
                    // Store using command's request_id string for reliable matching
                    println!("[P2P] 📤 Sending command {} to node {} (libp2p request_id: {:?}, command request_id: {})", 
                        cmd.command, peer_id_str, _libp2p_request_id, cmd_request_id);
                }
                
                // Store channel for response using command's request_id string as key
                {
                    let mut pending = pending_clone.lock().await;
                    pending.insert(cmd_request_id.clone(), tx);
                    println!("[P2P]   Stored pending response channel for request_id: {}", cmd_request_id);
                }
                
                println!("[P2P]   Command details: {}", serde_json::to_string(&cmd).unwrap_or_default());
                
                // Broadcast node event: command sent
                if let Some(ref tx) = node_event_tx_clone {
                    let event = NodeEventMessage {
                        message_type: "node_event".to_string(),
                        event: "command_sent".to_string(),
                        node_id: peer_id_str.clone(),
                        shard_id: Some(shard_id),
                        command: Some(cmd.command.clone()),
                        status: None,
                        latency_ms: None,
                        timestamp: std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap()
                            .as_secs(),
                        details: Some(format!("Sending {} to node", cmd.command)),
                    };
                    let _ = tx.send(event);
                }
                
                // Wait for response with timeout
                println!("[P2P] ⏳ Waiting for response from {} (request_id: {})...", peer_id_str, cmd_request_id);
                let response = tokio::time::timeout(Duration::from_secs(30), rx).await;
                
                // Remove from pending
                {
                    let mut pending = pending_clone.lock().await;
                    pending.remove(&cmd_request_id);
                }
                
                let latency_ms = cmd_start.elapsed().as_millis() as f64;
                let duration_ms = latency_ms as u64;
                
                match response {
                    Ok(Ok(cmd_response)) => {
                        println!("[P2P] ✅ Received response from {}", peer_id_str);
                        
                        // Broadcast node event: command received
                        if let Some(ref tx) = node_event_tx_clone {
                            let status_str = format!("{:?}", cmd_response.status);
                            let event = NodeEventMessage {
                                message_type: "node_event".to_string(),
                                event: "command_received".to_string(),
                                node_id: peer_id_str.clone(),
                                shard_id: Some(shard_id),
                                command: Some(cmd.command.clone()),
                                status: Some(status_str.clone()),
                                latency_ms: Some(duration_ms),
                                timestamp: std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .unwrap()
                                    .as_secs(),
                                details: Some(format!("Response received: {}", status_str)),
                            };
                            let _ = tx.send(event);
                        }
                        
                        // Record command received
                        {
                            let mut m = metrics_clone.write().await;
                            m.commands_received += 1;
                            m.command_latency_samples.push(latency_ms);
                            let resp_size = serde_json::to_string(&cmd_response).map(|s| s.len() as u64).unwrap_or(0);
                            m.bytes_received += resp_size;
                            
                            // Record shard loaded
                            if is_load_shard {
                                if m.shards_loading > 0 {
                                    m.shards_loading -= 1;
                                }
                                m.shard_load_events.push(ShardLoadEvent {
                                    timestamp: std::time::SystemTime::now()
                                        .duration_since(std::time::UNIX_EPOCH)
                                        .unwrap()
                                        .as_secs(),
                                    peer_id: peer_id_str.clone(),
                                    shard_id,
                                    status: "loaded".to_string(),
                                    duration_ms: Some(duration_ms),
                                });
                                
                                // Broadcast shard loaded event
                                if let Some(ref tx) = node_event_tx_clone {
                                    let event = NodeEventMessage {
                                        message_type: "node_event".to_string(),
                                        event: "shard_loaded".to_string(),
                                        node_id: peer_id_str.clone(),
                                        shard_id: Some(shard_id),
                                        command: None,
                                        status: Some("loaded".to_string()),
                                        latency_ms: Some(duration_ms),
                                        timestamp: std::time::SystemTime::now()
                                            .duration_since(std::time::UNIX_EPOCH)
                                            .unwrap()
                                            .as_secs(),
                                        details: Some(format!("Shard {} loaded in {}ms", shard_id, duration_ms)),
                                    };
                                    let _ = tx.send(event);
                                }
                            }
                        }
                        
                        Ok(cmd_response)
                    }
                    Ok(Err(_)) => {
                        eprintln!("[P2P] ❌ Channel error waiting for response from {}", peer_id_str);
                        
                        // Broadcast node event: command error
                        if let Some(ref tx) = node_event_tx_clone {
                            let event = NodeEventMessage {
                                message_type: "node_event".to_string(),
                                event: "command_error".to_string(),
                                node_id: peer_id_str.clone(),
                                shard_id: Some(shard_id),
                                command: Some(cmd.command.clone()),
                                status: Some("error".to_string()),
                                latency_ms: None,
                                timestamp: std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .unwrap()
                                    .as_secs(),
                                details: Some("Channel error".to_string()),
                            };
                            let _ = tx.send(event);
                        }
                        
                        Err(punch_simple::PipelineError::Internal { 
                            message: format!("Channel error from {}", peer_id_str) 
                        })
                    }
                    Err(_) => {
                        eprintln!("[P2P] ❌ Timeout waiting for response from {}", peer_id_str);
                        {
                            let mut m = metrics_clone.write().await;
                            m.command_errors += 1;
                        }
                        
                        // Broadcast node event: command timeout
                        if let Some(ref tx) = node_event_tx_clone {
                            let event = NodeEventMessage {
                                message_type: "node_event".to_string(),
                                event: "command_error".to_string(),
                                node_id: peer_id_str.clone(),
                                shard_id: Some(shard_id),
                                command: Some(cmd.command.clone()),
                                status: Some("timeout".to_string()),
                                latency_ms: None,
                                timestamp: std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .unwrap()
                                    .as_secs(),
                                details: Some("Timeout waiting for response".to_string()),
                            };
                            let _ = tx.send(event);
                        }
                        
                        Err(punch_simple::PipelineError::Internal { 
                            message: format!("Timeout from {}", peer_id_str) 
                        })
                    }
                }
            })
        };
        
        coordinator = coordinator.with_command_sender(command_sender);
        let coordinator = Arc::new(coordinator);
        
        // Start background DHT discovery task with shared swarm
        let coordinator_clone = Arc::clone(&coordinator);
        let bootstrap_clone = bootstrap.to_string();
        let metrics_clone = Arc::clone(&metrics);
        let swarm_for_discovery = Arc::clone(&swarm_arc);
        let pending_for_discovery = Arc::clone(&pending_responses);
        let node_event_tx_for_discovery = node_event_tx.clone();
        let discovery_task = tokio::spawn(async move {
            Self::run_dht_discovery_with_swarm(
                bootstrap_clone, 
                coordinator_clone, 
                metrics_clone,
                swarm_for_discovery,
                pending_for_discovery,
                node_event_tx_for_discovery,
            ).await;
        });
        
        // Start metrics update task
        let metrics_clone = Arc::clone(&metrics);
        let coordinator_clone = Arc::clone(&coordinator);
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(1));
            loop {
                interval.tick().await;
                let mut m = metrics_clone.write().await;
                let (online, total, _, _) = coordinator_clone.get_pipeline_status().await;
                m.nodes_online = online;
                m.nodes_offline = total.saturating_sub(online);
                m.last_updated = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs();
                
                // Keep only last 100 events
                if m.node_join_events.len() > 100 {
                    m.node_join_events.remove(0);
                }
                if m.shard_load_events.len() > 100 {
                    m.shard_load_events.remove(0);
                }
                let latency_len = m.command_latency_samples.len();
                if latency_len > 1000 {
                    let remove_count = latency_len - 1000;
                    m.command_latency_samples.drain(0..remove_count);
                }
                
                // Calculate average latency
                if !m.command_latency_samples.is_empty() {
                    m.avg_command_latency_ms = m.command_latency_samples.iter().sum::<f64>() / m.command_latency_samples.len() as f64;
                }
            }
        });
        
        // Create node queue manager
        let (node_queue_manager, _status_rx) = NodeQueueManager::new();
        let node_queue_manager = Arc::new(node_queue_manager);
        
        // Start periodic status polling task - request status from all known nodes
        let queue_manager_clone = Arc::clone(&node_queue_manager);
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(10));
            loop {
                interval.tick().await;
                // Process any pending status updates (nodes will send status automatically)
                // This task can be extended to actively poll nodes if needed
            }
        });
        
        Ok(Self {
            coordinator,
            peer_id,
            swarm: swarm_arc,
            pending_responses,
            discovery_task: Arc::new(discovery_task),
            metrics,
            node_event_tx,
            node_queue_manager,
        })
    }

    /// Run DHT discovery in background to find shard nodes (using shared swarm)
    async fn run_dht_discovery_with_swarm(
        bootstrap: String, 
        coordinator: Arc<PipelineCoordinator>, 
        metrics: Arc<RwLock<SystemMetrics>>,
        swarm: Arc<Mutex<Swarm<DiscoveryBehaviour>>>,
        pending_responses: Arc<Mutex<HashMap<String, oneshot::Sender<punch_simple::command_protocol::CommandResponse>>>>,
        node_event_tx: Option<tokio::sync::broadcast::Sender<NodeEventMessage>>,
    ) {
        println!("[DHT] Starting background DHT discovery with shared swarm...");
        
        // Connect to bootstrap
        let bootstrap_addr: Multiaddr = match bootstrap.parse() {
            Ok(addr) => addr,
            Err(e) => {
                eprintln!("[DHT] Invalid bootstrap address: {}", e);
                return;
            }
        };
        
        let bootstrap_addr_for_dht = bootstrap_addr.clone();
        {
            let mut swarm = swarm.lock().await;
            println!("[DHT] Connecting to bootstrap: {}", bootstrap);
            if let Err(e) = swarm.dial(bootstrap_addr) {
                eprintln!("[DHT] Failed to dial bootstrap: {}", e);
                return;
            }
        }

        let cluster_name = "llama-cluster".to_string();
        let total_shards = 4;
        let bootstrapped = Arc::new(Mutex::new(false));
        let queries_sent = Arc::new(Mutex::new(false));

        println!("[DHT] Background discovery task started");

        // Spawn task to handle swarm events
        let swarm_for_events = Arc::clone(&swarm);
        let pending_for_events = Arc::clone(&pending_responses);
        let coordinator_for_events = Arc::clone(&coordinator);
        let metrics_for_events = Arc::clone(&metrics);
        let bootstrapped_for_events = Arc::clone(&bootstrapped);
        let node_event_tx_for_events = node_event_tx.clone();
        let bootstrap_addr_for_events = bootstrap_addr_for_dht.clone();
        tokio::spawn(async move {
            use futures_util::StreamExt;
            loop {
                let event = {
                    let mut swarm_guard = swarm_for_events.lock().await;
                    swarm_guard.select_next_some().await
                };
                
                match event {
                    SwarmEvent::ConnectionEstablished { peer_id: connected_peer, .. } => {
                        let mut boot = bootstrapped_for_events.lock().await;
                        if !*boot {
                            let mut swarm_guard = swarm_for_events.lock().await;
                            // Add bootstrap node's address to Kademlia
                            swarm_guard.behaviour_mut().kademlia.add_address(&connected_peer, bootstrap_addr_for_events.clone());
                            
                            if let Err(e) = swarm_guard.behaviour_mut().kademlia.bootstrap() {
                                eprintln!("[DHT] Bootstrap failed: {:?}", e);
                            } else {
                                println!("[DHT] ✓ Started Kademlia bootstrap with bootstrap node {}", connected_peer);
                                *boot = true;
                            }
                        }
                    }
                    SwarmEvent::Behaviour(behaviour_event) => {
                        // Handle request-response protocol responses
                        if let DiscoveryBehaviourEvent::RequestResponse(request_response::Event::Message { message, .. }) = &behaviour_event {
                            match message {
                                request_response::Message::Response { response, request_id: _libp2p_request_id, .. } => {
                                    // Parse response and send to waiting channel
                                    // The response.message contains the serialized CommandResponse
                                    println!("[P2P] 📥 Received response (libp2p request_id: {:?}): {}", _libp2p_request_id, response.message);
                                    
                                    if let Ok(cmd_response) = serde_json::from_str::<punch_simple::command_protocol::CommandResponse>(&response.message) {
                                        println!("[P2P] ✓ Parsed CommandResponse: status={:?}, command={}, request_id={}", 
                                            cmd_response.status, cmd_response.command, cmd_response.request_id);
                                        
                                        // Use command's request_id string to match with stored channel
                                        let mut pending = pending_for_events.lock().await;
                                        if let Some(tx) = pending.remove(&cmd_response.request_id) {
                                            println!("[P2P] ✓ Matched response to waiting channel for request_id: {}", cmd_response.request_id);
                                            let _ = tx.send(cmd_response);
                                        } else {
                                            eprintln!("[P2P] ⚠️  No waiting channel found for request_id: {} (libp2p request_id: {:?})", 
                                                cmd_response.request_id, _libp2p_request_id);
                                            eprintln!("[P2P]   Available pending request_ids: {:?}", pending.keys().collect::<Vec<_>>());
                                        }
                                    } else {
                                        eprintln!("[P2P] ❌ Failed to parse CommandResponse from: {}", response.message);
                                    }
                                }
                                request_response::Message::Request { .. } => {
                                    // We don't handle incoming requests in the web server
                                }
                            }
                        }
                        
                        // The NetworkBehaviour macro generates DiscoveryBehaviourEvent enum
                        // Match on Kademlia events to process discovered shards
                        match behaviour_event {
                            DiscoveryBehaviourEvent::Kademlia(kad::Event::OutboundQueryProgressed {
                                result: kad::QueryResult::GetRecord(Ok(kad::GetRecordOk::FoundRecord(peer_record))),
                                ..
                            }) => {
                                // Process discovered shard
                                // This query result came from Kademlia's queue - closer nodes queried first
                                match coordinator_for_events.process_dht_record(&peer_record.record).await {
                                Some(announcement) => {
                                    // Calculate routing depth for this node based on query result
                                    // Nodes returned earlier in queries are typically closer (queue ordering)
                                    let local_peer_id = {
                                        let swarm_guard = swarm_for_events.lock().await;
                                        *swarm_guard.local_peer_id()
                                    };
                                    if let Ok(peer_id) = announcement.peer_id.parse::<PeerId>() {
                                        let distance = calculate_xor_distance(&local_peer_id, &peer_id);
                                        let depth = estimate_bucket_depth(distance);
                                        
                                        // Update discovery with routing information
                                        coordinator_for_events.update_routing_depth(announcement.peer_id.clone(), depth).await;
                                    }
                                    
                                    println!("[DHT] ✓ Discovered shard {} from {} (using queue/depth tree for routing)", 
                                             announcement.shard_id, announcement.peer_id);
                                    
                                    // Record node join event
                                    {
                                        let mut m = metrics_for_events.write().await;
                                        m.total_nodes_joined += 1;
                                        m.node_join_events.push(NodeJoinEvent {
                                            timestamp: std::time::SystemTime::now()
                                                .duration_since(std::time::UNIX_EPOCH)
                                                .unwrap()
                                                .as_secs(),
                                            peer_id: announcement.peer_id.clone(),
                                            shard_id: Some(announcement.shard_id),
                                            multiaddr: Some(announcement.multiaddr.clone()),
                                        });
                                    }
                                    
                                    // Register node in queue manager (if we have access to it)
                                    // Note: We'll need to pass queue manager to discovery task
                                    
                                    // Broadcast node_joined event
                                    if let Some(ref tx) = node_event_tx_for_events {
                                        let event = NodeEventMessage {
                                            message_type: "node_event".to_string(),
                                            event: "node_joined".to_string(),
                                            node_id: announcement.peer_id.clone(),
                                            shard_id: Some(announcement.shard_id),
                                            command: None,
                                            status: Some("online".to_string()),
                                            latency_ms: None,
                                            timestamp: std::time::SystemTime::now()
                                                .duration_since(std::time::UNIX_EPOCH)
                                                .unwrap()
                                                .as_secs(),
                                            details: Some(format!("Node joined with shard {}", announcement.shard_id)),
                                        };
                                        let _ = tx.send(event);
                                    }
                                    
                                    // Immediately update pipeline status after discovering a node
                                    let (online_nodes, total_nodes, missing_shards, is_complete) = coordinator_for_events.get_pipeline_status().await;
                                    println!("[DHT] Pipeline status after discovery: {}/{} nodes online, complete: {}", 
                                             online_nodes, total_nodes, is_complete);
                                }
                                None => {
                                    eprintln!("[DHT] ⚠️  Failed to process DHT record - invalid or malformed announcement");
                                    eprintln!("[DHT]   Record key: {:?}", peer_record.record.key);
                                    eprintln!("[DHT]   Record value length: {} bytes", peer_record.record.value.len());
                                }
                            }
                            }
                            DiscoveryBehaviourEvent::Kademlia(kad::Event::RoutingUpdated { 
                                peer,
                                ..
                            }) => {
                                // Routing table updated - update depth information for weighting
                                // This reflects Kademlia's queue (k-buckets) and depth tree structure
                                let peer_id_str = peer.to_string();
                                
                                // Calculate routing depth based on XOR distance
                                // In Kademlia, nodes are organized in buckets by distance
                                // We can estimate bucket depth from the routing update
                                let local_peer_id = {
                                    let swarm_guard = swarm_for_events.lock().await;
                                    *swarm_guard.local_peer_id()
                                };
                                let distance = calculate_xor_distance(&local_peer_id, &peer);
                                let depth = estimate_bucket_depth(distance);
                                
                                // Update discovery with routing depth for better node selection
                                coordinator_for_events.update_routing_depth(peer_id_str.clone(), depth).await;
                                
                                println!("[DHT] Routing updated: peer={}, depth={} (using Kademlia queue/depth tree for weighting)", 
                                         peer, depth);
                            }
                            _ => {}
                        }
                    }
                    _ => {}
                }
            }
        });
        
        // Periodic query task
        let mut next_query = tokio::time::Instant::now() + Duration::from_secs(2);
        loop {
            tokio::time::sleep_until(next_query).await;
            
            let is_bootstrapped = *bootstrapped.lock().await;
            if is_bootstrapped {
                let mut sent = queries_sent.lock().await;
                if !*sent {
                    println!("[DHT] Querying for {} shards...", total_shards);
                    *sent = true;
                } else {
                    println!("[DHT] Re-querying shards...");
                }
                drop(sent);
                
                {
                    let mut swarm_guard = swarm.lock().await;
                    for shard_id in 0..total_shards {
                        let key = kad::RecordKey::new(&dht_keys::shard_key(&cluster_name, shard_id));
                        swarm_guard.behaviour_mut().kademlia.get_record(key);
                    }
                }
                
                // Schedule next query in 10 seconds
                next_query = tokio::time::Instant::now() + Duration::from_secs(10);
            } else {
                // Check again in 100ms if not bootstrapped
                next_query = tokio::time::Instant::now() + Duration::from_millis(100);
            }
        }
    }

    /// Spawn nodes for missing shards on startup
    async fn ensure_minimal_pipeline(&self) -> Result<(), Box<dyn std::error::Error>> {
        println!("[SERVER] Checking pipeline status for startup node spawning...");
        
        // Use coordinator's method to spawn missing nodes
        if let Err(e) = self.coordinator.spawn_missing_nodes_on_startup().await {
            return Err(format!("Failed to spawn startup nodes: {}", e).into());
        }
        
        Ok(())
    }

    /// Record a node join event
    async fn record_node_join(&self, peer_id: String, shard_id: Option<u32>, multiaddr: Option<String>) {
        let mut metrics = self.metrics.write().await;
        metrics.total_nodes_joined += 1;
        metrics.node_join_events.push(NodeJoinEvent {
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            peer_id,
            shard_id,
            multiaddr,
        });
    }
    
    /// Record a shard load event
    async fn record_shard_load(&self, peer_id: String, shard_id: u32, status: String, duration_ms: Option<u64>) {
        let mut metrics = self.metrics.write().await;
        if status == "loaded" {
            metrics.total_shards_loaded += 1;
            metrics.shards_available += 1;
        } else if status == "loading" {
            metrics.shards_loading += 1;
        }
        metrics.shard_load_events.push(ShardLoadEvent {
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            peer_id,
            shard_id,
            status,
            duration_ms,
        });
    }
    
    /// Record a command sent
    async fn record_command_sent(&self, latency_ms: f64, bytes: u64) {
        let mut metrics = self.metrics.write().await;
        metrics.commands_sent += 1;
        metrics.bytes_sent += bytes;
        metrics.command_latency_samples.push(latency_ms);
    }
    
    /// Record a command received
    async fn record_command_received(&self, bytes: u64) {
        let mut metrics = self.metrics.write().await;
        metrics.commands_received += 1;
        metrics.bytes_received += bytes;
    }
    
    /// Record a command error
    async fn record_command_error(&self) {
        let mut metrics = self.metrics.write().await;
        metrics.command_errors += 1;
    }

    async fn process_query(&self, query: &str, update_sender: Option<&tokio::sync::mpsc::Sender<PipelineUpdate>>) -> QueryResponse {
        let start = Instant::now();
        
        // Record inference request
        {
            let mut metrics = self.metrics.write().await;
            metrics.inference_requests += 1;
        }

        // Send initial status
        if let Some(sender) = update_sender {
            let _ = sender.send(PipelineUpdate {
                stage: "input".to_string(),
                status: "processing".to_string(),
                shard_id: None,
                latency_ms: None,
            }).await;
        }
        
        // Note: Preload messages will be sent during actual inference processing
        // when we have access to the pipeline from the coordinator
        let (online_nodes, _, _missing_shards, is_complete) = self.coordinator.get_pipeline_status().await;
        println!("[INFERENCE] Starting query processing, pipeline status: {} nodes, complete: {}", online_nodes, is_complete);

        // Create inference request
        let inference_request = InferenceRequest::new(query)
            .with_max_tokens(256)
            .with_temperature(0.7);

        if let Some(sender) = update_sender {
            let _ = sender.send(PipelineUpdate {
                stage: "discovery".to_string(),
                status: "processing".to_string(),
                shard_id: None,
                latency_ms: None,
            }).await;
        }

        // Submit to pipeline coordinator
        println!("[INFERENCE] ═══════════════════════════════════════════════════════════════════════════");
        println!("[INFERENCE] Submitting inference request: \"{}\"", query);
        println!("[INFERENCE]   Request ID: {}", inference_request.request_id);
        println!("[INFERENCE]   Max tokens: {}", inference_request.max_tokens);
        println!("[INFERENCE]   Temperature: {}", inference_request.temperature);
        
        // Check pipeline status before submitting
        let (online_nodes, total_nodes, missing_shards, is_complete) = self.coordinator.get_pipeline_status().await;
        println!("[INFERENCE] Pipeline status: {}/{} nodes online, complete: {}, missing: {:?}", 
                 online_nodes, total_nodes, is_complete, missing_shards);
        
        if online_nodes == 0 {
            eprintln!("[INFERENCE] ⚠️  No nodes online! Cannot process inference.");
            eprintln!("[INFERENCE]   Missing shards: {:?}", missing_shards);
            eprintln!("[INFERENCE]   Nodes may still be starting up. Please wait...");
        }
        
        println!("[INFERENCE] Calling coordinator.submit_inference()...");
        let result = self.coordinator.submit_inference(inference_request).await;
        println!("[INFERENCE] coordinator.submit_inference() returned");
        match &result {
            Ok(_) => {
                println!("[INFERENCE] ✓ Inference request succeeded");
                let mut metrics = self.metrics.write().await;
                metrics.inference_successes += 1;
            }
            Err(e) => {
                eprintln!("[INFERENCE] ✗ Inference request failed: {}", e);
                let mut metrics = self.metrics.write().await;
                metrics.inference_failures += 1;
            }
        }

        if let Some(sender) = update_sender {
            let _ = sender.send(PipelineUpdate {
                stage: "discovery".to_string(),
                status: "complete".to_string(),
                shard_id: None,
                latency_ms: Some(100),
            }).await;
        }

        let latency_ms = start.elapsed().as_millis() as u64;
        
        // Record inference latency
        {
            let mut metrics = self.metrics.write().await;
            metrics.avg_inference_latency_ms = (metrics.avg_inference_latency_ms * (metrics.inference_successes as f64 - 1.0) + latency_ms as f64) / metrics.inference_successes as f64;
        }
        
        match result {
            Ok(response) => {
                // Send preload messages first (before processing updates)
                let (online_nodes, _, _, is_complete) = self.coordinator.get_pipeline_status().await;
                if is_complete && online_nodes > 0 {
                    // Get pipeline info from response shard latencies
                    println!("[INFERENCE] 📦 Sending preload messages for {} shards", response.shard_latencies.len());
                    for shard_latency in &response.shard_latencies {
                        if let Some(sender) = update_sender {
                            println!("[INFERENCE] 📦 Preload: Node {} loading shard {}", 
                                     shard_latency.node_id, shard_latency.shard_id);
                            let _ = sender.send(PipelineUpdate {
                                stage: format!("preload{}", shard_latency.shard_id),
                                status: "loading".to_string(),
                                shard_id: Some(shard_latency.shard_id),
                                latency_ms: None,
                            }).await;
                        }
                    }
                    // Small delay to show preload messages
                    tokio::time::sleep(Duration::from_millis(200)).await;
                }
                
                // Send shard processing updates in real-time
                println!("[INFERENCE] Sending real-time updates for {} shards", response.shard_latencies.len());
                for (_idx, shard_latency) in response.shard_latencies.iter().enumerate() {
                    if let Some(sender) = update_sender {
                        // Send "processing" update
                        let stage_name = format!("shard{}", shard_latency.shard_id);
                        println!("[INFERENCE] 📡 Sending update: {} -> processing", stage_name);
                        if let Err(e) = sender.send(PipelineUpdate {
                            stage: stage_name.clone(),
                            status: "processing".to_string(),
                            shard_id: Some(shard_latency.shard_id),
                            latency_ms: None,
                        }).await {
                            eprintln!("[INFERENCE] Failed to send processing update: {}", e);
                        }
                        
                        // Small delay to show processing state
                        tokio::time::sleep(Duration::from_millis(50)).await;
                        
                        // Send "complete" update with latency
                        println!("[INFERENCE] 📡 Sending update: {} -> complete ({}ms)", stage_name, shard_latency.latency_ms);
                        if let Err(e) = sender.send(PipelineUpdate {
                            stage: stage_name,
                            status: "complete".to_string(),
                            shard_id: Some(shard_latency.shard_id),
                            latency_ms: Some(shard_latency.latency_ms as u64),
                        }).await {
                            eprintln!("[INFERENCE] Failed to send complete update: {}", e);
                        }
                    }
                }

                if let Some(sender) = update_sender {
                    let _ = sender.send(PipelineUpdate {
                        stage: "output".to_string(),
                        status: "processing".to_string(),
                        shard_id: None,
                        latency_ms: None,
                    }).await;
                }

                let shard_infos: Vec<ShardInfo> = response.shard_latencies.iter().map(|sl| {
                    ShardInfo {
                        shard_id: sl.shard_id,
                        layer_start: 0, // Will be filled from shard announcement
                        layer_end: 0,
                        latency_ms: sl.latency_ms as u64,
                    }
                }).collect();

                if let Some(sender) = update_sender {
                    let _ = sender.send(PipelineUpdate {
                        stage: "output".to_string(),
                        status: "complete".to_string(),
                        shard_id: None,
                        latency_ms: Some(50),
                    }).await;
                }

                QueryResponse {
                    response: response.text,
                    tokens: response.tokens_generated as usize,
                    latency_ms: response.total_latency_ms as u64,
                    shards_used: shard_infos,
                    success: response.success,
                    request_id: Some(response.request_id),
                }
            }
            Err(e) => {
                let error_msg = format!("Pipeline error: {}", e);
                eprintln!("[INFERENCE] {}", error_msg);
                
                QueryResponse {
                    response: error_msg,
                    tokens: 0,
                    latency_ms: start.elapsed().as_millis() as u64,
                    shards_used: vec![],
                    success: false,
                    request_id: None,
                }
            }
        }
    }

    /// Send a control command to a specific node
    pub async fn send_node_control_command(
        &self,
        node_id: &str,
        command: ControlCommandType,
        params: HashMap<String, serde_json::Value>,
    ) -> Result<serde_json::Value, String> {
        // Register node if not already registered
        if !self.node_queue_manager.is_node_registered(node_id).await {
            // Try to get shard_id from coordinator if available
            let shard_id = None; // Could query coordinator for this
            self.node_queue_manager.register_node(node_id.to_string(), shard_id).await
                .map_err(|e| format!("Failed to register node: {}", e))?;
        }

        // Create control command
        let control_cmd = self.node_queue_manager.create_control_command(node_id, command, params)
            .map_err(|e| format!("Failed to create command: {}", e))?;

        // Convert to P2P Command format
        let command_name = format!("NODE_CONTROL_{:?}", control_cmd.command);
        let mut params = HashMap::new();
        params.insert("control_command".to_string(), serde_json::to_value(&control_cmd).unwrap());
        params.insert("request_id".to_string(), serde_json::Value::String(control_cmd.request_id.clone()));
        
        let p2p_command = punch_simple::command_protocol::Command {
            command: command_name,
            request_id: control_cmd.request_id.clone(),
            from: self.peer_id.to_string(),
            to: Some(node_id.to_string()),
            timestamp: control_cmd.timestamp,
            params,
        };

        // Validate command before sending
        if let Err(validation_error) = validate_command(&p2p_command) {
            return Err(format!("Command validation failed: {}", validation_error));
        }

        // Send via P2P directly using swarm
        let target_peer: PeerId = node_id.parse()
            .map_err(|e| format!("Invalid peer ID: {}", e))?;
        
        let cmd_json = serde_json::to_string(&p2p_command)
            .map_err(|e| format!("Serialization error: {}", e))?;
        
        let msg = JsonMessage::new(self.peer_id.to_string(), cmd_json);
        
        // Send request via P2P
        let (tx, rx) = oneshot::channel();
        let cmd_request_id = p2p_command.request_id.clone();
        {
            let mut swarm = self.swarm.lock().await;
            let _libp2p_request_id = swarm.behaviour_mut().request_response.send_request(&target_peer, msg);
            println!("[NODE_CONTROL] 📤 Sending command {} to node {} (libp2p request_id: {:?}, command request_id: {})", 
                p2p_command.command, node_id, _libp2p_request_id, cmd_request_id);
        }
        
        // Store channel for response using command's request_id string
        {
            let mut pending = self.pending_responses.lock().await;
            pending.insert(cmd_request_id.clone(), tx);
            println!("[NODE_CONTROL]   Stored pending response channel for request_id: {}", cmd_request_id);
        }
        
        // Wait for response with timeout
        println!("[NODE_CONTROL] ⏳ Waiting for response from {} (request_id: {})...", node_id, cmd_request_id);
        let response = tokio::time::timeout(Duration::from_secs(30), rx).await;
        
        // Remove from pending
        {
            let mut pending = self.pending_responses.lock().await;
            pending.remove(&cmd_request_id);
        }
        
        let cmd_response = match response {
            Ok(Ok(cmd_response)) => cmd_response,
            Ok(Err(_)) => return Err("Channel error".to_string()),
            Err(_) => return Err("Timeout waiting for response".to_string()),
        };

        // Extract result from response
        match cmd_response.status {
            punch_simple::command_protocol::ResponseStatus::Success => {
                cmd_response.result
                    .ok_or_else(|| "No result in response".to_string())
                    .and_then(|r| {
                        r.get("result")
                            .cloned()
                            .ok_or_else(|| "No result field".to_string())
                    })
            }
            _ => Err(cmd_response.error.unwrap_or_else(|| "Command failed".to_string())),
        }
    }

    /// Get status of a specific node
    pub async fn get_node_status(&self, node_id: &str) -> Option<NodeStatusUpdate> {
        self.node_queue_manager.get_node_status(node_id).await
    }

    /// List all registered nodes
    pub async fn list_nodes(&self) -> Vec<String> {
        self.node_queue_manager.list_nodes().await
    }

    /// Update node status (called when receiving status from node)
    pub async fn update_node_status(&self, status: NodeStatusUpdate) {
        self.node_queue_manager.update_node_status(status).await;
    }

    /// Restart all 4 nodes (shards 0, 1, 2, 3)
    pub async fn restart_all_nodes(&self) -> Result<String, String> {
        println!("[SERVER] 🔄 Restarting all 4 nodes...");
        
        // Unregister all nodes from queue manager first
        let node_ids = self.node_queue_manager.list_nodes().await;
        for node_id in node_ids {
            self.node_queue_manager.unregister_node(&node_id).await;
        }
        
        // Use coordinator's restart method
        match self.coordinator.restart_all_nodes().await {
            Ok(_) => {
                println!("[SERVER] ✅ Successfully restarted all nodes");
                Ok("All 4 nodes restarted successfully. They will come online shortly.".to_string())
            }
            Err(e) => {
                let error_msg = format!("Failed to restart nodes: {}", e);
                eprintln!("[SERVER] ❌ {}", error_msg);
                Err(error_msg)
            }
        }
    }
}

/// Generate contextual responses (DEPRECATED - now using real inference)
#[allow(dead_code)]
fn generate_response(query: &str) -> String {
    let q = query.to_lowercase();
    
    // Music questions
    if q.contains("pinball wizard") {
        return "**Pete Townshend** wrote \"Pinball Wizard\" for The Who's 1969 rock opera \"Tommy\". The song tells the story of a \"deaf, dumb and blind kid\" who becomes a pinball champion. It reached #4 in the UK and #19 in the US. Elton John later covered it for the 1975 film.".to_string();
    }
    
    if q.contains("wonderwall") {
        return "**Noel Gallagher** wrote \"Wonderwall\" for **Oasis** in 1995. It appeared on \"(What's the Story) Morning Glory?\" and reached #2 in the UK. Noel said it's about \"an imaginary friend who's gonna save you from yourself.\" It's one of the most-covered songs ever.".to_string();
    }
    
    if q.contains("bohemian rhapsody") {
        return "**Freddie Mercury** wrote \"Bohemian Rhapsody\" for **Queen** in 1975. The 6-minute epic features an intro, ballad, operatic section, hard rock segment, and outro. Despite being \"too long for radio,\" it became one of the best-selling singles of all time.".to_string();
    }

    if q.contains("twist and shout") || q.contains("twist & shout") {
        return "\"Twist and Shout\" was written by **Phil Medley** and **Bert Berns** in 1961. The Beatles' 1963 version is most famous - recorded in one take at the end of a 10-hour session when John Lennon's voice was nearly gone, giving it that raw, powerful sound.".to_string();
    }

    if q.contains("imagine") && !q.contains("dragon") {
        return "**John Lennon** wrote \"Imagine\" in 1971. It envisions a world without borders, religion, or possessions. Yoko Ono was credited as co-writer in 2017. It's been voted the best song of the 20th century and remains an anthem for peace movements worldwide.".to_string();
    }

    if q.contains("stairway to heaven") {
        return "**Jimmy Page** (music) and **Robert Plant** (lyrics) wrote \"Stairway to Heaven\" for Led Zeppelin in 1971. At 8 minutes, it builds from acoustic to thundering rock. Never released as a single, yet became the most-requested song in radio history.".to_string();
    }

    if q.contains("hotel california") {
        return "**Don Felder** wrote the music, **Don Henley** and **Glenn Frey** wrote the lyrics to \"Hotel California\" for the Eagles in 1977. Often interpreted as a metaphor for excess in the music industry. The guitar outro with Felder and Joe Walsh is iconic.".to_string();
    }

    if q.contains("smells like teen spirit") {
        return "**Kurt Cobain** wrote \"Smells Like Teen Spirit\" for Nirvana in 1991. The title came from graffiti by Kathleen Hanna (referencing a deodorant brand). It knocked Michael Jackson off #1 and defined the grunge movement. Cobain grew to hate it due to its popularity.".to_string();
    }

    if q.contains("yesterday") && q.contains("beatles") || (q.contains("yesterday") && q.contains("wrote")) {
        return "**Paul McCartney** wrote \"Yesterday\" for the Beatles in 1965. It's the most-covered song in history with 2,200+ versions. McCartney woke up with the melody and initially used \"Scrambled eggs\" as placeholder lyrics. It was the first Beatles song featuring just one member.".to_string();
    }

    if q.contains("like a rolling stone") {
        return "**Bob Dylan** wrote \"Like a Rolling Stone\" in 1965. Rolling Stone magazine ranked it #1 greatest song of all time. At 6 minutes, it broke radio conventions. The opening snare hit by Bobby Gregg is one of rock's most famous drum sounds.".to_string();
    }

    if q.contains("sweet home alabama") {
        return "Lynyrd Skynyrd's **Ronnie Van Zant**, **Ed King**, and **Gary Rossington** wrote \"Sweet Home Alabama\" in 1974. It was a response to Neil Young's \"Southern Man.\" Despite the lyrical rivalry, Van Zant was a huge Neil Young fan and wore his t-shirt on stage.".to_string();
    }

    // Capital cities
    if q.contains("capital") && q.contains("france") {
        return "The capital of **France** is **Paris**. Located on the Seine River, it's known as the \"City of Light.\" Key landmarks include the Eiffel Tower (1889), Louvre Museum, Notre-Dame Cathedral, and Arc de Triomphe. Population: 2.1 million (12 million metro).".to_string();
    }
    
    if q.contains("capital") && q.contains("japan") {
        return "The capital of **Japan** is **Tokyo**. With 37 million people, it's the world's most populous metro area. Famous districts include Shibuya, Shinjuku, and Akihabara. It blends ancient temples like Senso-ji with ultramodern architecture and technology.".to_string();
    }
    
    if q.contains("capital") && q.contains("germany") {
        return "The capital of **Germany** is **Berlin**. Population: 3.7 million. It's been the capital of reunified Germany since 1990. Key sites include Brandenburg Gate, the Reichstag, Berlin Wall remnants, and Museum Island (UNESCO World Heritage).".to_string();
    }

    if q.contains("capital") && q.contains("italy") {
        return "The capital of **Italy** is **Rome**. Founded in 753 BC, it was the center of the Roman Empire. Home to the Vatican City, Colosseum, Pantheon, and Trevi Fountain. Population: 2.8 million. It's called \"The Eternal City.\"".to_string();
    }

    if q.contains("capital") && q.contains("spain") {
        return "The capital of **Spain** is **Madrid**. Located in the center of the Iberian Peninsula, it's Spain's largest city with 3.3 million people. Famous for the Prado Museum, Royal Palace, and vibrant nightlife. It became the capital in 1561.".to_string();
    }

    // Promethos/AI
    if q.contains("promethos") || q.contains("what are you") || q.contains("who are you") {
        return "I am **Promethos-AI**, a distributed AI running on a decentralized swarm network. Your queries are processed across 4 neural network shards via Kademlia DHT. The name references Prometheus, who brought fire to humanity - we're bringing AI to everyone through distributed computing.".to_string();
    }

    // Code
    if q.contains("rust") || q.contains("code") || q.contains("program") {
        return "Here's a Rust async example:\n\n```rust\n#[tokio::main]\nasync fn main() {\n    let result = fetch_data().await;\n    println!(\"Got: {}\", result);\n}\n\nasync fn fetch_data() -> String {\n    tokio::time::sleep(Duration::from_secs(1)).await;\n    \"Hello from async Rust!\".to_string()\n}\n```\n\nThis shows Rust's async/await with Tokio runtime.".to_string();
    }

    // Greetings
    if q.contains("hello") || q.contains("hi ") || q.starts_with("hi") || q.contains("hey") {
        return "**Hello!** 👋 I'm Promethos-AI, running on a distributed swarm network. Try asking me about:\n\n• 🎵 Music: \"Who wrote Bohemian Rhapsody?\"\n• 🌍 Geography: \"What is the capital of Japan?\"\n• 💻 Code: \"Show me some Rust code\"\n• 🤖 About me: \"What is Promethos?\"".to_string();
    }

    // Math
    if q.contains("2+2") || q.contains("2 + 2") {
        return "2 + 2 = **4**\n\nFun fact: This simple equation is processed through the same distributed pipeline as complex queries - tokenized, embedded into vectors, processed through transformer layers, and decoded into this response!".to_string();
    }

    // Weather
    if q.contains("weather") {
        return "I don't have real-time data access, but I can explain weather! It's determined by atmospheric pressure, humidity, temperature, and wind patterns. For current conditions, try weather.gov (US) or your phone's weather app.".to_string();
    }

    // Default - still informative
    format!("I processed your query \"{}\" through the distributed Promethos-AI pipeline.\n\nWhile I don't have specific information about that topic in my current knowledge base, I can help with:\n\n• 🎵 **Music**: Song writers and history\n• 🌍 **Geography**: World capitals and facts\n• 💻 **Code**: Rust programming examples\n• 🤖 **AI**: How this system works\n\nTry asking something like \"Who wrote Hotel California?\" or \"What is the capital of France?\"", query)
}

/// Handle a WebSocket connection
async fn handle_connection(
    stream: TcpStream, 
    addr: SocketAddr, 
    engine: Arc<InferenceEngine>, 
    mut node_request_rx: tokio::sync::broadcast::Receiver<NodeInferenceRequestMessage>,
    mut node_event_rx: tokio::sync::broadcast::Receiver<NodeEventMessage>,
    mut node_status_rx: tokio::sync::broadcast::Receiver<NodeStatusUpdate>,
) {
    println!("[WS] New TCP connection from: {}", addr);
    
    let ws_stream = match accept_async(stream).await {
        Ok(ws) => {
            println!("[WS] ✓ WebSocket upgrade successful from: {}", addr);
            ws
        }
        Err(e) => {
            eprintln!("[WS] ✗ Failed to upgrade WebSocket connection from {}: {}", addr, e);
            return;
        }
    };

    let (write, mut read) = ws_stream.split();
    
    // Create channel for all outgoing messages
    let (outgoing_tx, mut outgoing_rx) = tokio::sync::mpsc::unbounded_channel::<Message>();
    
    // Spawn task to send all outgoing messages
    let mut write_sink = write;
    tokio::spawn(async move {
        while let Some(msg) = outgoing_rx.recv().await {
            let msg_type = format!("{:?}", msg);
            if let Err(e) = write_sink.send(msg).await {
                eprintln!("[WS] ❌ Failed to send message: {}", e);
                eprintln!("[WS]   Message type: {}", msg_type);
                eprintln!("[WS]   Connection may be closed");
                break;
            }
        }
        eprintln!("[WS] Outgoing message channel closed");
    });
    
    // Wait a moment for WebSocket to be fully ready, then send initial pipeline status
    let engine_for_init = Arc::clone(&engine);
    let outgoing_tx_for_init = outgoing_tx.clone();
    let addr_for_init = addr;
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        let (online_nodes, total_nodes, missing_shards, is_complete) = engine_for_init.coordinator.get_pipeline_status().await;
        
        let status_msg = PipelineStatusMessage {
            message_type: "pipeline_status".to_string(),
            total_nodes,
            online_nodes,
            missing_shards,
            is_complete,
        };
        
        let status_json = serde_json::to_string(&status_msg).unwrap();
        let _ = outgoing_tx_for_init.send(Message::Text(status_json));
        println!("[WS] Sent initial pipeline status to {}: {} nodes online, complete: {}", addr_for_init, online_nodes, is_complete);
    });
    
    // Create channel for pipeline updates
    let (update_tx, mut update_rx) = tokio::sync::mpsc::channel::<PipelineUpdate>(32);
    
    // Get a receiver for node inference request messages from the engine
    // The engine has access to the broadcast sender, we need to get a receiver
    // For now, we'll get it from a shared location - actually, we need to pass it differently
    // Let's store it in the engine or pass it through a different mechanism
    // For simplicity, we'll create a new receiver from the broadcast channel
    // But we need access to the original tx - let's store it in the engine or use a different approach
    // Actually, let's just not handle it in handle_connection for now - we'll send it from the command sender directly
    
    // Spawn task to send periodic metrics updates
    let metrics_engine = Arc::clone(&engine);
    let metrics_tx = outgoing_tx.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(2));
        loop {
            interval.tick().await;
            let metrics = metrics_engine.metrics.read().await.clone();
            let metrics_msg = MetricsMessage {
                message_type: "metrics".to_string(),
                metrics,
            };
            if let Ok(json) = serde_json::to_string(&metrics_msg) {
                if let Err(e) = metrics_tx.send(Message::Text(json)) {
                    eprintln!("[WS] Failed to send metrics update: {}", e);
                }
            } else {
                eprintln!("[WS] Failed to serialize metrics message");
            }
        }
    });
    
    // Spawn task to send periodic pipeline status updates
    let status_engine = Arc::clone(&engine);
    let status_tx = outgoing_tx.clone();
    let mut was_ready = false; // Track if we've already sent ready message
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(2)); // More frequent updates
        let mut last_status: Option<(u32, u32, Vec<u32>, bool)> = None;
        loop {
            interval.tick().await;
            let (online_nodes, total_nodes, missing_shards, is_complete) = status_engine.coordinator.get_pipeline_status().await;
            
            // Check if pipeline is ready (complete and has nodes)
            let is_ready = is_complete && online_nodes > 0;
            
            // Send ready message when pipeline becomes ready (only once)
            if is_ready && !was_ready {
                println!("[WS] 🟢 Pipeline is READY! Sending ready status ({} nodes online)", online_nodes);
                let ready_msg = ReadyStatusMessage {
                    message_type: "ready".to_string(),
                    ready: true,
                    online_nodes,
                    total_nodes,
                };
                if let Ok(json) = serde_json::to_string(&ready_msg) {
                    if let Err(e) = status_tx.send(Message::Text(json)) {
                        eprintln!("[WS] Failed to send ready message: {}", e);
                    } else {
                        println!("[WS] ✓ Ready message sent");
                    }
                }
                was_ready = true;
            } else if !is_ready && was_ready {
                // Pipeline became not ready again
                was_ready = false;
            }
            
            // Always send update (removed change detection for now to ensure UI updates)
            let current_status = (online_nodes, total_nodes, missing_shards.clone(), is_complete);
            let missing_shards_clone = missing_shards.clone();
            let status_msg = PipelineStatusMessage {
                message_type: "pipeline_status".to_string(),
                total_nodes,
                online_nodes,
                missing_shards,
                is_complete,
            };
            if let Ok(json) = serde_json::to_string(&status_msg) {
                if let Err(e) = status_tx.send(Message::Text(json)) {
                    eprintln!("[WS] Failed to send status update: {}", e);
                } else if last_status.as_ref() != Some(&current_status) {
                    println!("[WS] Pipeline status update: {}/{} nodes online, complete: {}, missing: {:?}", 
                             online_nodes, total_nodes, is_complete, missing_shards_clone);
                }
            } else {
                eprintln!("[WS] Failed to serialize status message");
            }
            last_status = Some(current_status);
        }
    });
    
    // Use select to handle both incoming messages and updates
    loop {
        tokio::select! {
            msg = read.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        println!("[WS] Received: {}", text);
                        
                        // Check for restart_all_nodes command
                        if let Ok(restart_req) = serde_json::from_str::<serde_json::Value>(&text) {
                            if restart_req.get("type").and_then(|v| v.as_str()) == Some("restart_all_nodes") {
                                println!("[WS] Received restart_all_nodes command");
                                let engine_clone = Arc::clone(&engine);
                                let outgoing_tx_clone = outgoing_tx.clone();
                                
                                tokio::spawn(async move {
                                    let result = engine_clone.restart_all_nodes().await;
                                    
                                    let response = serde_json::json!({
                                        "type": "restart_all_nodes_response",
                                        "success": result.is_ok(),
                                        "message": result.as_ref().ok().cloned().unwrap_or_else(|| result.unwrap_err()),
                                    });
                                    
                                    let _ = outgoing_tx_clone.send(Message::Text(serde_json::to_string(&response).unwrap()));
                                });
                                
                                continue;
                            }
                        }
                        
                        // Try to parse as node control request first
                        if let Ok(mut control_req) = serde_json::from_str::<NodeControlRequest>(&text) {
                            if control_req.message_type == "node_control" {
                                println!("[WS] Processing node control command: {} for node {}", control_req.command, control_req.node_id);
                                
                                // Clone before using in match
                                let control_req_clone = control_req.clone();
                                let command_str = control_req.command.clone();
                                
                                // Parse command type
                                let command_type = match command_str.as_str() {
                                    "SET_OPERATION_MODE" => ControlCommandType::SetOperationMode,
                                    "GET_STATUS" => ControlCommandType::GetStatus,
                                    "GET_CAPABILITIES" => ControlCommandType::GetCapabilities,
                                    "PAUSE" => ControlCommandType::Pause,
                                    "RESUME" => ControlCommandType::Resume,
                                    "RESTART" => ControlCommandType::Restart,
                                    "SHUTDOWN" => ControlCommandType::Shutdown,
                                    "UPDATE_CONFIG" => ControlCommandType::UpdateConfig,
                                    _ => {
                                        let error_response = NodeControlResponse {
                                            message_type: "node_control_response".to_string(),
                                            node_id: control_req_clone.node_id.clone(),
                                            command: command_str.clone(),
                                            success: false,
                                            result: None,
                                            error: Some(format!("Unknown command: {}", command_str)),
                                            request_id: control_req_clone.request_id.clone(),
                                        };
                                        let _ = outgoing_tx.send(Message::Text(serde_json::to_string(&error_response).unwrap()));
                                        continue;
                                    }
                                };
                                
                                let params = control_req.params.take().unwrap_or_default();
                                let engine_clone = Arc::clone(&engine);
                                let outgoing_tx_clone = outgoing_tx.clone();
                                
                                tokio::spawn(async move {
                                    let result = engine_clone.send_node_control_command(
                                        &control_req_clone.node_id,
                                        command_type,
                                        params,
                                    ).await;
                                    
                                    let response = match result {
                                        Ok(result_value) => NodeControlResponse {
                                            message_type: "node_control_response".to_string(),
                                            node_id: control_req_clone.node_id,
                                            command: control_req_clone.command,
                                            success: true,
                                            result: Some(result_value),
                                            error: None,
                                            request_id: control_req_clone.request_id,
                                        },
                                        Err(e) => NodeControlResponse {
                                            message_type: "node_control_response".to_string(),
                                            node_id: control_req_clone.node_id,
                                            command: control_req_clone.command,
                                            success: false,
                                            result: None,
                                            error: Some(e),
                                            request_id: control_req_clone.request_id,
                                        },
                                    };
                                    
                                    let _ = outgoing_tx_clone.send(Message::Text(serde_json::to_string(&response).unwrap()));
                                });
                                
                                continue;
                            }
                        }
                        
                        // Parse as regular query request
                        let request: QueryRequest = match serde_json::from_str(&text) {
                            Ok(r) => r,
                            Err(_) => QueryRequest { query: text, request_id: None },
                        };
                        
                        // Process query
                        println!("[WS] Processing query: {}", request.query);
                        let mut response = engine.process_query(&request.query, Some(&update_tx)).await;
                        response.request_id = request.request_id;
                        println!("[WS] Query processed, sending response");
                        
                        // Send final response
                        let response_json = serde_json::to_string(&response).unwrap();
                        let _ = outgoing_tx.send(Message::Text(response_json));
                        
                        // Send updated pipeline status after query
                        {
                            let (online_nodes, total_nodes, missing_shards, is_complete) = engine.coordinator.get_pipeline_status().await;
                            
                            let status_msg = PipelineStatusMessage {
                                message_type: "pipeline_status".to_string(),
                                total_nodes,
                                online_nodes,
                                missing_shards,
                                is_complete,
                            };
                            
                            let status_json = serde_json::to_string(&status_msg).unwrap();
                            let _ = outgoing_tx.send(Message::Text(status_json));
                        }
                    }
                    Some(Ok(Message::Close(_))) => {
                        println!("[WS] Client {} disconnected", addr);
                        break;
                    }
                    Some(Err(e)) => {
                        eprintln!("[WS] Error: {}", e);
                        break;
                    }
                    None => break,
                    _ => {}
                }
            }
            update = update_rx.recv() => {
                match update {
                    Some(update) => {
                        let update_json = serde_json::to_string(&update).unwrap();
                        let _ = outgoing_tx.send(Message::Text(update_json));
                    }
                    None => {
                        // Channel closed
                        break;
                    }
                }
            }
            node_request = node_request_rx.recv() => {
                // Handle node inference request messages from broadcast channel
                match node_request {
                    Ok(msg) => {
                        println!("[WS] Sending node inference request message: node={}, shard={}", msg.node_id, msg.shard_id);
                        if let Ok(json) = serde_json::to_string(&msg) {
                            if let Err(e) = outgoing_tx.send(Message::Text(json)) {
                                eprintln!("[WS] Failed to send node inference request message: {}", e);
                            }
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                        // Messages were dropped, continue
                    }
                    Err(e) => {
                        eprintln!("[WS] Error receiving node inference request: {}", e);
                    }
                }
            }
            node_event = node_event_rx.recv() => {
                // Handle node event messages from broadcast channel
                match node_event {
                    Ok(event) => {
                        println!("[WS] Sending node event: {} from node {}", event.event, event.node_id);
                        if let Ok(json) = serde_json::to_string(&event) {
                            if let Err(e) = outgoing_tx.send(Message::Text(json)) {
                                eprintln!("[WS] Failed to send node event message: {}", e);
                            }
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                        // Messages were dropped, continue
                    }
                    Err(e) => {
                        eprintln!("[WS] Error receiving node event: {}", e);
                    }
                }
            }
            node_status = node_status_rx.recv() => {
                // Handle node status updates from broadcast channel
                match node_status {
                    Ok(status) => {
                        println!("[WS] Broadcasting node status update: node={}, status={:?}", status.node_id, status.status);
                        if let Ok(json) = serde_json::to_string(&status) {
                            if let Err(e) = outgoing_tx.send(Message::Text(json)) {
                                eprintln!("[WS] Failed to send node status update: {}", e);
                            }
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                        // Messages were dropped, continue
                    }
                    Err(e) => {
                        eprintln!("[WS] Error receiving node status update: {}", e);
                    }
                }
            }
        }
    }
}

/// Calculate XOR distance between two peer IDs (Kademlia distance metric)
/// Returns the XOR result as u64 for distance comparison
/// Uses Kademlia's queue ordering: closer nodes queried first
fn calculate_xor_distance(peer1: &PeerId, peer2: &PeerId) -> u64 {
    let bytes1 = peer1.to_bytes();
    let bytes2 = peer2.to_bytes();
    
    // XOR the peer ID bytes
    let min_len = bytes1.len().min(bytes2.len());
    let mut distance = 0u64;
    
    for i in 0..min_len {
        let xor_byte = bytes1[i] ^ bytes2[i];
        distance = (distance << 8) | (xor_byte as u64);
    }
    
    distance
}

/// Estimate bucket depth from XOR distance
/// In Kademlia, nodes are organized in k-buckets by distance (depth tree)
/// Lower distance = closer nodes = lower bucket index (depth)
/// Returns depth 0-160 where 0 is closest (top of queue)
fn estimate_bucket_depth(distance: u64) -> u32 {
    if distance == 0 {
        return 0; // Same node
    }
    
    // Count leading zeros in distance
    // More leading zeros = closer = lower depth
    let leading_zeros = distance.leading_zeros();
    
    // Convert to depth: 0-160 range
    // Closer nodes (more leading zeros) have lower depth
    // This reflects the depth tree structure where nodes are organized by prefix
    if leading_zeros >= 56 {
        0u32.saturating_add((leading_zeros as u32).saturating_sub(56) % 20)  // Very close (same bucket, top of queue)
    } else if leading_zeros >= 48 {
        20u32.saturating_add((leading_zeros as u32).saturating_sub(48) % 20)  // Close (early in queue)
    } else if leading_zeros >= 40 {
        40u32.saturating_add((leading_zeros as u32).saturating_sub(40) % 40)  // Medium (middle of queue)
    } else if leading_zeros >= 32 {
        80u32.saturating_add((leading_zeros as u32).saturating_sub(32) % 40)  // Far (later in queue)
    } else {
        120u32.saturating_add((leading_zeros as u32) % 40)  // Very far (bottom of queue)
    }
}

/// Serve static files
async fn serve_static(path: &str) -> Option<(String, Vec<u8>)> {
    let file_path = if path == "/" || path.is_empty() {
        "web/ai-console.html".to_string()
    } else {
        // Remove leading slash and prepend web/ directory
        let clean_path = path.trim_start_matches('/');
        if clean_path.starts_with("web/") {
            clean_path.to_string()
        } else {
            format!("web/{}", clean_path)
        }
    };
    
    // Log file access attempts
    if !file_path.starts_with("web/") {
        eprintln!("[HTTP] ⚠️  Security: Blocked access to file outside web/ directory: {}", file_path);
        return None;
    }

    let full_path = std::path::Path::new(&file_path);
    
    match tokio::fs::read(full_path).await {
        Ok(content) => {
            if content.is_empty() {
                eprintln!("[HTTP] ⚠️  Warning: File is empty: {}", file_path);
            }
            let content_type = match full_path.extension().and_then(|e| e.to_str()) {
                Some("html") => "text/html",
                Some("css") => "text/css",
                Some("js") => "application/javascript",
                Some("json") => "application/json",
                Some("png") => "image/png",
                Some("jpg") | Some("jpeg") => "image/jpeg",
                Some("svg") => "image/svg+xml",
                _ => "application/octet-stream",
            };
            Some((content_type.to_string(), content))
        }
        Err(e) => {
            eprintln!("[HTTP] ❌ Failed to read file {}: {}", file_path, e);
            None
        }
    }
}

/// Run web server (extracted for unified binary)
pub async fn run_web_server(bootstrap: String) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n");
    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║          🔥 PROMETHOS-AI WEB SERVER 🔥                       ║");
    println!("╠══════════════════════════════════════════════════════════════╣");
    println!("║  Web Console: http://localhost:8080                          ║");
    println!("║  WebSocket:   ws://localhost:8081                            ║");
    println!("╚══════════════════════════════════════════════════════════════╝\n");

    // Create channels for node messages - BEFORE creating engine
    let (node_request_tx, _) = tokio::sync::broadcast::channel::<NodeInferenceRequestMessage>(64);
    let (node_event_tx, _) = tokio::sync::broadcast::channel::<NodeEventMessage>(128);
    
    // Initialize real inference engine with DHT discovery
    println!("[SERVER] Connecting to DHT bootstrap: {}", bootstrap);
    
    let engine = Arc::new(InferenceEngine::new(&bootstrap, Some(node_request_tx.clone()), Some(node_event_tx.clone())).await?);
    println!("[SERVER] Inference engine initialized with real distributed pipeline");
    
    // Get node status broadcast receiver (for potential future use)
    let _node_status_rx = engine.node_queue_manager.status_broadcast_sender().subscribe();
    
    // Spawn nodes for missing shards on startup
    println!("[SERVER] Ensuring minimal pipeline is ready...");
    if let Err(e) = engine.ensure_minimal_pipeline().await {
        eprintln!("[SERVER] ⚠️  Warning: Failed to spawn startup nodes: {}", e);
        eprintln!("[SERVER] Nodes will be spawned on-demand when requests arrive");
    }

    // Start WebSocket server
    let ws_listener = TcpListener::bind("127.0.0.1:8081").await?;
    println!("[SERVER] WebSocket listening on ws://127.0.0.1:8081");

    // Start HTTP server for static files
    let http_listener = TcpListener::bind("127.0.0.1:8080").await?;
    println!("[SERVER] HTTP listening on http://127.0.0.1:8080");
    println!("\n[SERVER] Open http://localhost:8080 in your browser!\n");

    // Spawn HTTP server
    tokio::spawn(async move {
        loop {
            if let Ok((mut stream, _)) = http_listener.accept().await {
                tokio::spawn(async move {
                    use tokio::io::{AsyncReadExt, AsyncWriteExt};
                    
                    let mut buf = [0u8; 4096];
                    if let Ok(n) = stream.read(&mut buf).await {
                        let request = String::from_utf8_lossy(&buf[..n]);
                        let path = request.lines().next()
                            .and_then(|line| line.split_whitespace().nth(1))
                            .unwrap_or("/");
                        
                        let response = match serve_static(path).await {
                            Some((content_type, body)) => {
                                let header = format!(
                                    "HTTP/1.1 200 OK\r\nContent-Type: {}\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\n\r\n",
                                    content_type,
                                    body.len()
                                );
                                [header.into_bytes(), body].concat()
                            }
                            None => {
                                eprintln!("[HTTP] 404 Not Found: {}", path);
                                b"HTTP/1.1 404 Not Found\r\nContent-Length: 9\r\n\r\nNot Found".to_vec()
                            }
                        };
                        
                        if let Err(e) = stream.write_all(&response).await {
                            eprintln!("[HTTP] ❌ Error writing response for {}: {}", path, e);
                        }
                    }
                });
            }
        }
    });

    // Accept WebSocket connections
    println!("[SERVER] Waiting for WebSocket connections...");
    loop {
        match ws_listener.accept().await {
            Ok((stream, addr)) => {
                let engine_clone = Arc::clone(&engine);
                let node_request_rx_clone = node_request_tx.subscribe();
                let node_event_rx_clone = node_event_tx.subscribe();
                let node_status_rx_clone = engine_clone.node_queue_manager.status_broadcast_sender().subscribe();
                tokio::spawn(handle_connection(stream, addr, engine_clone, node_request_rx_clone, node_event_rx_clone, node_status_rx_clone));
            }
            Err(e) => {
                eprintln!("[SERVER] Error accepting WebSocket connection: {}", e);
                // Continue accepting connections even if one fails
            }
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let bootstrap = std::env::var("BOOTSTRAP").unwrap_or_else(|_| "/ip4/127.0.0.1/tcp/51820".to_string());
    run_web_server(bootstrap).await
}

