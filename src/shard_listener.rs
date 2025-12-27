//! Shard Listener - Kademlia node that announces its shard for distributed Llama inference
//!
//! This listener joins the Kademlia DHT and announces its model shard information,
//! enabling clients to discover all shards needed for distributed inference.
//!
//! Usage:
//!   cargo run --bin shard_listener -- \
//!     --bootstrap /ip4/SERVER/tcp/51820 \
//!     --cluster llama-8b-cluster \
//!     --shard-id 0 \
//!     --total-shards 4
//!
//! Or via environment variables:
//!   LLAMA_SHARD_ID=0 LLAMA_TOTAL_SHARDS=4 cargo run --bin shard_listener

mod message;
mod metrics;
mod command_protocol;
mod shard_optimization;
mod kademlia_shard_discovery;

use message::{JsonMessage, JsonCodec};
use metrics::{MetricsCodec, PeerMetrics};
use kademlia_shard_discovery::{KademliaShardDiscovery, ShardAnnouncement, dht_keys, PipelineStatus};
use command_protocol::{Command, CommandResponse, ResponseStatus, commands};

use clap::Parser;
use libp2p::{
    identity,
    tcp,
    noise,
    yamux,
    kad,
    relay,
    request_response::{self, ProtocolSupport},
    swarm::{NetworkBehaviour, Swarm, SwarmEvent},
    core::transport::Transport,
    PeerId, Multiaddr, StreamProtocol,
};
use libp2p::swarm::Config as SwarmConfig;
use libp2p::futures::StreamExt;
use std::error::Error;
use std::time::Duration;
use std::collections::HashMap;
use std::sync::Arc;
use std::path::{Path, PathBuf};
use tokio::sync::RwLock;
use sha2::{Sha256, Digest};

#[derive(Parser, Debug)]
#[command(name = "shard_listener")]
#[command(about = "Kademlia Shard Listener - Announces model shards for distributed Llama inference")]
struct Args {
    /// Bootstrap node address (Multiaddr format)
    #[arg(long, default_value = "/ip4/127.0.0.1/tcp/51820")]
    bootstrap: String,

    /// Cluster name for shard discovery
    #[arg(long, default_value = "llama-cluster")]
    cluster: String,

    /// Shard ID for this node (0, 1, 2, ...)
    #[arg(long, env = "LLAMA_SHARD_ID")]
    shard_id: Option<u32>,

    /// Total number of shards in cluster
    #[arg(long, env = "LLAMA_TOTAL_SHARDS", default_value = "4")]
    total_shards: u32,

    /// Total layers in the model
    #[arg(long, env = "LLAMA_TOTAL_LAYERS", default_value = "32")]
    total_layers: u32,

    /// Model name
    #[arg(long, env = "LLAMA_MODEL_NAME", default_value = "llama-8b")]
    model_name: String,

    /// Listen port (0 for random)
    #[arg(long, default_value = "0")]
    port: u16,

    /// Announcement refresh interval in seconds
    #[arg(long, default_value = "60")]
    refresh_interval: u64,

    /// Directory containing GGUF shards to seed via torrent
    #[arg(long, env = "LLAMA_SHARDS_DIR", default_value = "models_cache/shards")]
    shards_dir: String,

    /// Enable torrent server to seed all GGUF files
    #[arg(long, default_value = "true")]
    enable_torrent: bool,
}

#[derive(NetworkBehaviour)]
struct ShardBehaviour {
    kademlia: kad::Behaviour<kad::store::MemoryStore>,
    identify: libp2p::identify::Behaviour,
    request_response: request_response::Behaviour<JsonCodec>,
    metrics_response: request_response::Behaviour<MetricsCodec>,
    relay: relay::Behaviour,
}

/// Torrent file metadata (simplified from torrent_server)
#[derive(Clone, Debug)]
struct TorrentFileInfo {
    info_hash: String,
    filename: String,
    size: u64,
}

/// Shard node state
struct ShardNodeState {
    peer_id: PeerId,
    shard_id: u32,
    announcement: ShardAnnouncement,
    discovery: KademliaShardDiscovery,
    listen_addrs: Vec<Multiaddr>,
    active_requests: u32,
    total_requests: u64,
    successful_requests: u64,
    // Torrent server state
    torrent_files: HashMap<String, TorrentFileInfo>, // info_hash -> file info
    shards_dir: PathBuf,
    loaded_shards: HashMap<u32, PathBuf>, // shard_id -> path to loaded GGUF file
}

impl ShardNodeState {
    fn new(peer_id: PeerId, shard_id: u32, total_shards: u32, total_layers: u32, model_name: &str, cluster: &str, shards_dir: &str) -> Self {
        let announcement = ShardAnnouncement::new(
            &peer_id.to_string(),
            shard_id,
            total_shards,
            total_layers,
            "", // Will be updated with actual listen address
            model_name,
        );

        let discovery = KademliaShardDiscovery::with_expected_shards(cluster, total_shards);
        
        let shards_path = PathBuf::from(shards_dir);
        let mut state = Self {
            peer_id,
            shard_id,
            announcement,
            discovery,
            listen_addrs: Vec::new(),
            active_requests: 0,
            total_requests: 0,
            successful_requests: 0,
            torrent_files: HashMap::new(),
            shards_dir: shards_path.clone(),
            loaded_shards: HashMap::new(),
        };
        
        // Scan for GGUF files to seed
        state.scan_gguf_files();
        
        state
    }
    
    /// Scan shards directory for GGUF files and create torrent metadata
    fn scan_gguf_files(&mut self) {
        if !self.shards_dir.exists() {
            println!("[TORRENT] Shards directory does not exist: {}", self.shards_dir.display());
            return;
        }
        
        match std::fs::read_dir(&self.shards_dir) {
            Ok(entries) => {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_file() && path.extension().map(|e| e == "gguf").unwrap_or(false) {
                        if let Some(file_info) = Self::create_torrent_file_info(&path) {
                            println!("[TORRENT] Found GGUF shard to seed: {} (hash: {})", 
                                file_info.filename, &file_info.info_hash[..16]);
                            self.torrent_files.insert(file_info.info_hash.clone(), file_info);
                        }
                    }
                }
                println!("[TORRENT] Scanning complete: {} GGUF file(s) available for seeding", self.torrent_files.len());
            }
            Err(e) => {
                eprintln!("[TORRENT] Failed to scan shards directory: {}", e);
            }
        }
    }
    
    /// Create torrent file info from a GGUF file path
    fn create_torrent_file_info(path: &Path) -> Option<TorrentFileInfo> {
        let metadata = std::fs::metadata(path).ok()?;
        let file_size = metadata.len();
        let filename = path.file_name()?.to_string_lossy().to_string();
        
        // Calculate info hash (SHA256 of filename + size)
        let mut hasher = Sha256::new();
        hasher.update(filename.as_bytes());
        hasher.update(&file_size.to_le_bytes());
        let info_hash = format!("{:x}", hasher.finalize());
        
        Some(TorrentFileInfo {
            info_hash,
            filename,
            size: file_size,
        })
    }
    
    /// Get list of available GGUF files for torrent
    fn get_torrent_file_list(&self) -> Vec<&TorrentFileInfo> {
        self.torrent_files.values().collect()
    }
    
    /// Check if a shard is already loaded
    fn is_shard_loaded(&self, shard_id: u32) -> bool {
        self.loaded_shards.contains_key(&shard_id)
    }
    
    /// Load a shard file (if it exists locally)
    fn load_shard_file(&mut self, shard_id: u32) -> Result<PathBuf, String> {
        // Check if already loaded
        if let Some(path) = self.loaded_shards.get(&shard_id) {
            return Ok(path.clone());
        }
        
        // Try to find the shard file
        let shard_filename = format!("shard-{}.gguf", shard_id);
        let shard_path = self.shards_dir.join(&shard_filename);
        
        if shard_path.exists() {
            println!("[SHARD] Loading shard {} from: {}", shard_id, shard_path.display());
            self.loaded_shards.insert(shard_id, shard_path.clone());
            Ok(shard_path)
        } else {
            Err(format!("Shard file not found: {}", shard_path.display()))
        }
    }
    
    /// Download a shard via torrent (placeholder - will be implemented with actual torrent client)
    /// This would query the DHT for peers sharing the shard file and download it
    async fn download_shard_via_torrent(&mut self, shard_id: u32) -> Result<PathBuf, String> {
        let shard_filename = format!("shard-{}.gguf", shard_id);
        let shard_path = self.shards_dir.join(&shard_filename);
        
        println!("[TORRENT] Attempting to download shard {} via torrent...", shard_id);
        
        // TODO: Implement actual torrent download
        // 1. Query DHT for peers sharing this shard file
        // 2. Connect to peers
        // 3. Request file metadata
        // 4. Download file pieces
        // 5. Verify and save file
        
        // For now, return error indicating torrent download is needed
        Err(format!("Torrent download not yet implemented. Shard {} needs to be downloaded from other nodes.", shard_id))
    }

    fn update_listen_addr(&mut self, addr: &Multiaddr) {
        if !self.listen_addrs.contains(addr) {
            self.listen_addrs.push(addr.clone());
        }
        // Update announcement with primary listen address
        if let Some(primary) = self.listen_addrs.first() {
            self.announcement.multiaddr = primary.to_string();
        }
    }

    fn create_announcement_record(&self) -> kad::Record {
        self.discovery.create_announcement_record(&self.announcement)
    }

    fn handle_inference_request(&mut self) {
        self.active_requests += 1;
        self.total_requests += 1;
        self.announcement.capabilities.active_requests = self.active_requests;
    }

    fn complete_request(&mut self, success: bool) {
        self.active_requests = self.active_requests.saturating_sub(1);
        if success {
            self.successful_requests += 1;
        }
        self.announcement.capabilities.active_requests = self.active_requests;
    }

    fn get_status_string(&self) -> String {
        format!(
            "Shard {} (layers {}-{}) | Requests: {}/{} active, {}/{} total",
            self.shard_id,
            self.announcement.layer_start,
            self.announcement.layer_end,
            self.active_requests,
            self.announcement.capabilities.max_concurrent,
            self.successful_requests,
            self.total_requests
        )
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();

    // Determine shard ID
    let shard_id = args.shard_id.unwrap_or_else(|| {
        eprintln!("Error: --shard-id or LLAMA_SHARD_ID environment variable required");
        std::process::exit(1);
    });

    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║         Shard Listener - Distributed Llama Inference         ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!();
    println!("Configuration:");
    println!("  Cluster: {}", args.cluster);
    println!("  Shard ID: {} / {}", shard_id, args.total_shards - 1);
    println!("  Model: {}", args.model_name);
    println!("  Layers: {}-{}", 
        shard_id * (args.total_layers / args.total_shards),
        if shard_id == args.total_shards - 1 { args.total_layers } 
        else { (shard_id + 1) * (args.total_layers / args.total_shards) }
    );
    println!("  Bootstrap: {}", args.bootstrap);
    println!();

    // Generate keys
    let key = identity::Keypair::generate_ed25519();
    let peer_id = PeerId::from(key.public());
    println!("Peer ID: {}", peer_id);

    // Initialize state
    let state = Arc::new(RwLock::new(ShardNodeState::new(
        peer_id,
        shard_id,
        args.total_shards,
        args.total_layers,
        &args.model_name,
        &args.cluster,
        &args.shards_dir,
    )));
    
    // Try to load the assigned shard if it exists
    {
        let mut s = state.write().await;
        if let Ok(shard_path) = s.load_shard_file(shard_id) {
            println!("[SHARD] ✓ Loaded assigned shard {} from: {}", shard_id, shard_path.display());
        } else {
            println!("[SHARD] ⚠️  Assigned shard {} not found locally. Will download via torrent when needed.", shard_id);
        }
    }

    // Transport
    let transport = tcp::tokio::Transport::default()
        .upgrade(libp2p::core::upgrade::Version::V1)
        .authenticate(noise::Config::new(&key)?)
        .multiplex(yamux::Config::default())
        .boxed();

    // Kademlia DHT
    let store = kad::store::MemoryStore::new(peer_id);
    let mut kademlia_config = kad::Config::default();
    kademlia_config.set_query_timeout(Duration::from_secs(60));
    let mut kademlia = kad::Behaviour::with_config(peer_id, store, kademlia_config);

    // Add bootstrap node
    let bootstrap_addr: Multiaddr = args.bootstrap.parse()?;
    kademlia.add_address(&peer_id, bootstrap_addr.clone());

    // Identify
    let identify = libp2p::identify::Behaviour::new(
        libp2p::identify::Config::new(
            format!("shard-listener/{}/{}", args.cluster, shard_id),
            key.public(),
        )
    );

    // Request-Response for JSON messaging
    let request_response = request_response::Behaviour::with_codec(
        JsonCodec,
        [(StreamProtocol::new("/json-message/1.0"), ProtocolSupport::Full)],
        request_response::Config::default(),
    );

    // Metrics
    let metrics_response = request_response::Behaviour::with_codec(
        MetricsCodec,
        [(StreamProtocol::new("/metrics/1.0"), ProtocolSupport::Full)],
        request_response::Config::default(),
    );

    // Relay
    let relay = relay::Behaviour::new(peer_id, relay::Config::default());

    let behaviour = ShardBehaviour {
        kademlia,
        identify,
        request_response,
        metrics_response,
        relay,
    };

    // Swarm
    let swarm_config = SwarmConfig::with_tokio_executor()
        .with_idle_connection_timeout(Duration::from_secs(60));
    let mut swarm = Swarm::new(transport, behaviour, peer_id, swarm_config);

    // Listen
    let listen_addr: Multiaddr = format!("/ip4/0.0.0.0/tcp/{}", args.port).parse()?;
    swarm.listen_on(listen_addr)?;

    // Connect to bootstrap
    println!("\n🔗 Connecting to bootstrap node...");
    swarm.dial(bootstrap_addr)?;

    let mut bootstrapped = false;
    let mut announced = false;
    let cluster_name = args.cluster.clone();

    // Announcement refresh timer
    let refresh_interval = Duration::from_secs(args.refresh_interval);
    let mut next_refresh = tokio::time::Instant::now() + refresh_interval;

    println!("\n✅ Shard listener started! Waiting for connections...\n");

    loop {
        tokio::select! {
            event = swarm.select_next_some() => {
                match event {
                    SwarmEvent::NewListenAddr { address, .. } => {
                        println!("[LISTEN] Listening on: {}", address);
                        let mut s = state.write().await;
                        s.update_listen_addr(&address);
                        swarm.add_external_address(address);
                    }

                    SwarmEvent::ConnectionEstablished { peer_id: connected_peer, .. } => {
                        println!("[CONNECT] ✓ Connected to: {}", connected_peer);

                        if !bootstrapped {
                            // Start Kademlia bootstrap
                            if let Err(e) = swarm.behaviour_mut().kademlia.bootstrap() {
                                eprintln!("[WARN] Bootstrap failed: {:?}", e);
                            } else {
                                println!("[DHT] ✓ Started Kademlia bootstrap");
                                bootstrapped = true;
                            }
                        }
                    }

                    SwarmEvent::ConnectionClosed { peer_id: closed_peer, cause, .. } => {
                        println!("[DISCONNECT] ✗ Peer disconnected: {} ({:?})", closed_peer, cause);
                    }

                    SwarmEvent::Behaviour(behaviour_event) => {
                        match behaviour_event {
                            ShardBehaviourEvent::Kademlia(kad::Event::RoutingUpdated { peer, .. }) => {
                                println!("[DHT] Routing updated: {}", peer);

                                // Announce shard after routing table is populated
                                if !announced {
                                    let s = state.read().await;
                                    let record = s.create_announcement_record();
                                    drop(s);

                                    if let Err(e) = swarm.behaviour_mut().kademlia.put_record(record, kad::Quorum::One) {
                                        eprintln!("[DHT] Failed to announce shard: {:?}", e);
                                    } else {
                                        println!("[DHT] ✓ Announced shard {} to DHT", shard_id);
                                        announced = true;
                                    }

                                    // Also query for other shards
                                    for i in 0..args.total_shards {
                                        if i != shard_id {
                                            let key = kad::RecordKey::new(&dht_keys::shard_key(&cluster_name, i));
                                            swarm.behaviour_mut().kademlia.get_record(key);
                                        }
                                    }
                                }
                            }

                            ShardBehaviourEvent::Kademlia(kad::Event::OutboundQueryProgressed { 
                                result: kad::QueryResult::GetRecord(Ok(kad::GetRecordOk::FoundRecord(peer_record))),
                                ..
                            }) => {
                                // Process discovered shard
                                let mut s = state.write().await;
                                if let Some(ann) = s.discovery.process_shard_record(&peer_record.record) {
                                    println!("[DISCOVERY] Found shard {} at {}", ann.shard_id, ann.peer_id);
                                }

                                let status = s.discovery.status();
                                println!("[PIPELINE] {}", status);
                            }

                            ShardBehaviourEvent::Kademlia(kad::Event::OutboundQueryProgressed {
                                result: kad::QueryResult::PutRecord(Ok(_)),
                                ..
                            }) => {
                                println!("[DHT] ✓ Shard announcement stored in DHT");
                            }

                            ShardBehaviourEvent::RequestResponse(request_response::Event::Message { 
                                peer, 
                                message: request_response::Message::Request { request, channel, .. },
                                ..
                            }) => {
                                println!("[REQUEST] Received from {}: {}", peer, request.message);
                                
                                // Parse command from message
                                if let Ok(cmd) = serde_json::from_str::<Command>(&request.message) {
                                    let mut s = state.write().await;
                                    
                                    let response = match cmd.command.as_str() {
                                        commands::GET_CAPABILITIES => {
                                            // Return shard capabilities
                                            let mut result = HashMap::new();
                                            result.insert("shard_id".to_string(), serde_json::json!(s.shard_id));
                                            result.insert("layer_start".to_string(), serde_json::json!(s.announcement.layer_start));
                                            result.insert("layer_end".to_string(), serde_json::json!(s.announcement.layer_end));
                                            result.insert("has_embeddings".to_string(), serde_json::json!(s.announcement.has_embeddings));
                                            result.insert("has_output".to_string(), serde_json::json!(s.announcement.has_output));
                                            result.insert("active_requests".to_string(), serde_json::json!(s.active_requests));
                                            result.insert("capabilities".to_string(), serde_json::to_value(&s.announcement.capabilities).unwrap_or_default());
                                            
                                            CommandResponse::success(
                                                &cmd.command,
                                                &cmd.request_id,
                                                &peer_id.to_string(),
                                                &cmd.from,
                                                result,
                                            )
                                        }
                                        
                                        commands::LOAD_SHARD => {
                                            // Request to load a specific shard for inference
                                            let requested_shard_id = cmd.params.get("shard_id")
                                                .and_then(|v| v.as_u64())
                                                .map(|v| v as u32);
                                            
                                            match requested_shard_id {
                                                Some(shard_id) => {
                                                    println!("[LOAD_SHARD] Request to load shard {}", shard_id);
                                                    
                                                    // Check if already loaded
                                                    if s.is_shard_loaded(shard_id) {
                                                        let mut result = HashMap::new();
                                                        result.insert("shard_id".to_string(), serde_json::json!(shard_id));
                                                        result.insert("status".to_string(), serde_json::json!("already_loaded"));
                                                        if let Some(path) = s.loaded_shards.get(&shard_id) {
                                                            result.insert("path".to_string(), serde_json::json!(path.to_string_lossy()));
                                                        }
                                                        
                                                        CommandResponse::success(
                                                            &cmd.command,
                                                            &cmd.request_id,
                                                            &peer_id.to_string(),
                                                            &cmd.from,
                                                            result,
                                                        )
                                                    } else {
                                                        // Try to load from local directory first
                                                        match s.load_shard_file(shard_id) {
                                                            Ok(shard_path) => {
                                                                println!("[LOAD_SHARD] ✓ Loaded shard {} from local directory", shard_id);
                                                                let mut result = HashMap::new();
                                                                result.insert("shard_id".to_string(), serde_json::json!(shard_id));
                                                                result.insert("status".to_string(), serde_json::json!("loaded"));
                                                                result.insert("path".to_string(), serde_json::json!(shard_path.to_string_lossy()));
                                                                
                                                                CommandResponse::success(
                                                                    &cmd.command,
                                                                    &cmd.request_id,
                                                                    &peer_id.to_string(),
                                                                    &cmd.from,
                                                                    result,
                                                                )
                                                            }
                                                            Err(e) => {
                                                                // Shard not found locally - need to download via torrent
                                                                println!("[LOAD_SHARD] Shard {} not found locally: {}", shard_id, e);
                                                                println!("[LOAD_SHARD] TODO: Download shard {} via torrent from other nodes", shard_id);
                                                                
                                                                // For now, return error indicating torrent download needed
                                                                // In production, this would trigger torrent download
                                                                CommandResponse::error(
                                                                    &cmd.command,
                                                                    &cmd.request_id,
                                                                    &peer_id.to_string(),
                                                                    &cmd.from,
                                                                    &format!("Shard {} not found. Torrent download required.", shard_id),
                                                                )
                                                            }
                                                        }
                                                    }
                                                }
                                                None => {
                                                    CommandResponse::error(
                                                        &cmd.command,
                                                        &cmd.request_id,
                                                        &peer_id.to_string(),
                                                        &cmd.from,
                                                        "Missing shard_id parameter",
                                                    )
                                                }
                                            }
                                        }
                                        
                                        commands::LIST_FILES => {
                                            // List available GGUF files for torrent
                                            let file_list: Vec<serde_json::Value> = s.get_torrent_file_list()
                                                .iter()
                                                .map(|f| serde_json::json!({
                                                    "info_hash": f.info_hash,
                                                    "filename": f.filename,
                                                    "size": f.size,
                                                }))
                                                .collect();
                                            
                                            let mut result = HashMap::new();
                                            result.insert("files".to_string(), serde_json::json!(file_list));
                                            
                                            CommandResponse::success(
                                                &cmd.command,
                                                &cmd.request_id,
                                                &peer_id.to_string(),
                                                &cmd.from,
                                                result,
                                            )
                                        }
                                        
                                        commands::EXECUTE_TASK => {
                                            s.handle_inference_request();
                                            
                                            // Check task type
                                            let task_type = cmd.params.get("task_type")
                                                .and_then(|v| v.as_str())
                                                .unwrap_or("unknown");
                                            
                                            if task_type == "llama_fragment" || task_type == "ai_inference" {
                                                // Ensure shard is loaded before processing
                                                if !s.is_shard_loaded(s.shard_id) {
                                                    match s.load_shard_file(s.shard_id) {
                                                        Ok(shard_path) => {
                                                            println!("[INFERENCE] Loaded shard {} from: {}", s.shard_id, shard_path.display());
                                                        }
                                                        Err(e) => {
                                                            s.complete_request(false);
                                                            return CommandResponse::error(
                                                                &cmd.command,
                                                                &cmd.request_id,
                                                                &peer_id.to_string(),
                                                                &cmd.from,
                                                                &format!("Shard {} not loaded: {}. Use LOAD_SHARD command first.", s.shard_id, e),
                                                            );
                                                        }
                                                    }
                                                }
                                                
                                                // Process the fragment/inference request
                                                // In production, this would run actual model inference using the loaded shard
                                                
                                                let mut result = HashMap::new();
                                                result.insert("output".to_string(), serde_json::json!(
                                                    format!("[Shard {} processed layers {}-{}]", 
                                                        s.shard_id, s.announcement.layer_start, s.announcement.layer_end)
                                                ));
                                                result.insert("shard_id".to_string(), serde_json::json!(s.shard_id));
                                                result.insert("tokens_generated".to_string(), serde_json::json!(50));
                                                result.insert("processing_time_ms".to_string(), serde_json::json!(100.0));
                                                
                                                s.complete_request(true);
                                                
                                                CommandResponse::success(
                                                    &cmd.command,
                                                    &cmd.request_id,
                                                    &peer_id.to_string(),
                                                    &cmd.from,
                                                    result,
                                                )
                                            } else {
                                                s.complete_request(false);
                                                CommandResponse::error(
                                                    &cmd.command,
                                                    &cmd.request_id,
                                                    &peer_id.to_string(),
                                                    &cmd.from,
                                                    &format!("Unknown task type: {}", task_type),
                                                )
                                            }
                                        }
                                        
                                        _ => {
                                            CommandResponse::error(
                                                &cmd.command,
                                                &cmd.request_id,
                                                &peer_id.to_string(),
                                                &cmd.from,
                                                &format!("Unknown command: {}", cmd.command),
                                            )
                                        }
                                    };
                                    
                                    // Send response as JsonMessage
                                    let response_json = serde_json::to_string(&response).unwrap_or_default();
                                    let response_msg = JsonMessage::new(peer_id.to_string(), response_json);
                                    let _ = swarm.behaviour_mut().request_response.send_response(
                                        channel,
                                        response_msg,
                                    );
                                    
                                    println!("[STATUS] {}", s.get_status_string());
                                }
                            }

                            ShardBehaviourEvent::Identify(libp2p::identify::Event::Received { peer_id: identified_peer, info }) => {
                                println!("[IDENTIFY] {} running {}", identified_peer, info.agent_version);
                            }

                            _ => {}
                        }
                    }

                    SwarmEvent::OutgoingConnectionError { error, peer_id: failed_peer, .. } => {
                        eprintln!("[ERROR] Connection failed to {:?}: {:?}", failed_peer, error);
                    }

                    _ => {}
                }
            }

            // Periodic announcement refresh
            _ = tokio::time::sleep_until(next_refresh) => {
                if announced {
                    let s = state.read().await;
                    let record = s.create_announcement_record();
                    drop(s);

                    if let Err(e) = swarm.behaviour_mut().kademlia.put_record(record, kad::Quorum::One) {
                        eprintln!("[DHT] Refresh failed: {:?}", e);
                    } else {
                        println!("[DHT] ↻ Refreshed shard announcement");
                    }
                }
                next_refresh = tokio::time::Instant::now() + refresh_interval;
            }
        }
    }
}

