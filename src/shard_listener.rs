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
use serde::{Serialize, Deserialize};

/// Torrent protocol codec (same as torrent_server)
#[derive(Clone)]
struct TorrentCodec;

#[async_trait::async_trait]
impl request_response::Codec for TorrentCodec {
    type Request = TorrentMessage;
    type Response = TorrentMessage;
    type Protocol = StreamProtocol;

    async fn read_request<T>(&mut self, _: &Self::Protocol, io: &mut T) -> std::io::Result<Self::Request>
    where
        T: libp2p::futures::AsyncRead + Unpin + Send,
    {
        use libp2p::futures::AsyncReadExt;
        let mut buffer = Vec::new();
        io.read_to_end(&mut buffer).await?;
        serde_json::from_slice(&buffer).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
    }

    async fn read_response<T>(&mut self, _: &Self::Protocol, io: &mut T) -> std::io::Result<Self::Response>
    where
        T: libp2p::futures::AsyncRead + Unpin + Send,
    {
        use libp2p::futures::AsyncReadExt;
        let mut buffer = Vec::new();
        io.read_to_end(&mut buffer).await?;
        serde_json::from_slice(&buffer).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
    }

    async fn write_request<T>(&mut self, _: &Self::Protocol, io: &mut T, req: Self::Request) -> std::io::Result<()>
    where
        T: libp2p::futures::AsyncWrite + Unpin + Send,
    {
        use libp2p::futures::AsyncWriteExt;
        let json = serde_json::to_vec(&req).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        io.write_all(&json).await?;
        Ok(())
    }

    async fn write_response<T>(&mut self, _: &Self::Protocol, io: &mut T, res: Self::Response) -> std::io::Result<()>
    where
        T: libp2p::futures::AsyncWrite + Unpin + Send,
    {
        use libp2p::futures::AsyncWriteExt;
        let json = serde_json::to_vec(&res).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        io.write_all(&json).await?;
        Ok(())
    }
}

/// Torrent protocol messages
#[derive(Clone, Serialize, Deserialize, Debug)]
enum TorrentMessage {
    RequestPiece {
        info_hash: String,
        piece_index: u64,
    },
    PieceData {
        info_hash: String,
        piece_index: u64,
        data: Vec<u8>,
    },
    RequestMetadata {
        info_hash: String,
    },
    Metadata {
        metadata: TorrentMetadata,
    },
    ListFiles,
    FileList {
        files: Vec<TorrentFileInfo>,
    },
}

/// Torrent file metadata
#[derive(Clone, Serialize, Deserialize, Debug)]
struct TorrentMetadata {
    info_hash: String,
    filename: String,
    file_size: u64,
    piece_size: u64,
    pieces: Vec<String>, // SHA256 hashes of pieces
    announce: Vec<String>, // Peer addresses
}

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
    torrent_response: request_response::Behaviour<TorrentCodec>,
    relay: relay::Behaviour,
}

/// Torrent file metadata (simplified from torrent_server)
#[derive(Clone, Serialize, Deserialize, Debug)]
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
    // Download state
    active_downloads: HashMap<String, DownloadState>, // info_hash -> download state
}

/// State for an active torrent download
#[derive(Clone, Debug)]
struct DownloadState {
    info_hash: String,
    filename: String,
    target_path: PathBuf,
    metadata: Option<TorrentMetadata>,
    pieces: HashMap<u64, Vec<u8>>, // piece_index -> piece data
    total_pieces: usize,
    downloaded_pieces: usize,
    peer_id: Option<PeerId>, // Peer we're downloading from
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
            active_downloads: HashMap::new(),
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
    
    /// Get info hash for a shard file (if it exists in torrent_files)
    fn get_shard_info_hash(&self, shard_id: u32) -> Option<String> {
        let shard_filename = format!("shard-{}.gguf", shard_id);
        self.torrent_files.values()
            .find(|f| f.filename == shard_filename)
            .map(|f| f.info_hash.clone())
    }
    
    /// Find info hash for a shard by querying other nodes
    fn find_shard_info_hash(&self, shard_id: u32) -> String {
        // Generate a deterministic info hash based on shard_id
        // In production, this would query DHT for actual file records
        let mut hasher = Sha256::new();
        hasher.update(format!("shard-{}.gguf", shard_id).as_bytes());
        format!("{:x}", hasher.finalize())
    }
    
    /// Start downloading a shard via torrent
    fn start_download(&mut self, shard_id: u32, peer_id: PeerId) -> Result<String, String> {
        let shard_filename = format!("shard-{}.gguf", shard_id);
        let shard_path = self.shards_dir.join(&shard_filename);
        
        // Get or generate info hash
        let info_hash = self.get_shard_info_hash(shard_id)
            .unwrap_or_else(|| self.find_shard_info_hash(shard_id));
        
        // Check if already downloading
        if self.active_downloads.contains_key(&info_hash) {
            return Ok(info_hash.clone());
        }
        
        // Create download state
        let download = DownloadState {
            info_hash: info_hash.clone(),
            filename: shard_filename.clone(),
            target_path: shard_path.clone(),
            metadata: None,
            pieces: HashMap::new(),
            total_pieces: 0,
            downloaded_pieces: 0,
            peer_id: Some(peer_id),
        };
        
        self.active_downloads.insert(info_hash.clone(), download);
        println!("[TORRENT] Started download for shard {} (info_hash: {})", shard_id, &info_hash[..16]);
        
        Ok(info_hash)
    }
    
    /// Check if download is complete and save file
    fn check_download_complete(&mut self, info_hash: &str) -> Result<Option<PathBuf>, String> {
        let download = self.active_downloads.get_mut(info_hash)
            .ok_or_else(|| format!("Download not found: {}", info_hash))?;
        
        if let Some(metadata) = &download.metadata {
            if download.downloaded_pieces >= metadata.pieces.len() {
                // All pieces downloaded - assemble file
                println!("[TORRENT] All pieces downloaded, assembling file: {}", download.filename);
                
                // Sort pieces by index
                let mut sorted_pieces: Vec<_> = download.pieces.iter().collect();
                sorted_pieces.sort_by_key(|(idx, _)| **idx);
                
                // Concatenate pieces
                let mut file_data = Vec::new();
                for (_, piece_data) in sorted_pieces {
                    file_data.extend_from_slice(piece_data);
                }
                
                // Truncate to actual file size
                if file_data.len() > metadata.file_size as usize {
                    file_data.truncate(metadata.file_size as usize);
                }
                
                // Save file
                std::fs::create_dir_all(&download.target_path.parent().unwrap())
                    .map_err(|e| format!("Failed to create directory: {}", e))?;
                std::fs::write(&download.target_path, &file_data)
                    .map_err(|e| format!("Failed to write file: {}", e))?;
                
                println!("[TORRENT] ✓ File saved: {}", download.target_path.display());
                
                // Extract shard_id from filename and get path before removing download
                let target_path = download.target_path.clone();
                let shard_id_opt = download.filename.strip_prefix("shard-")
                    .and_then(|s| s.strip_suffix(".gguf"))
                    .and_then(|s| s.parse::<u32>().ok());
                
                // Drop the mutable borrow of download
                drop(download);
                
                // Now we can modify self.active_downloads and self.loaded_shards
                if let Some(shard_id) = shard_id_opt {
                    self.loaded_shards.insert(shard_id, target_path.clone());
                }
                
                // Remove from active downloads
                self.active_downloads.remove(info_hash);
                
                return Ok(Some(target_path));
            }
        }
        
        Ok(None)
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
            println!("[SHARD] ⚠️  Assigned shard {} not found locally.", shard_id);
            println!("[SHARD]   Shard will be downloaded via torrent when LOAD_SHARD command is received.");
            println!("[SHARD]   Or it can be pre-downloaded by querying DHT for available peers.");
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

    // Torrent protocol for file sharing
    let torrent_response = request_response::Behaviour::with_codec(
        TorrentCodec,
        [(StreamProtocol::new("/torrent/1.0"), ProtocolSupport::Full)],
        request_response::Config::default(),
    );

    // Relay
    let relay = relay::Behaviour::new(peer_id, relay::Config::default());

    let behaviour = ShardBehaviour {
        kademlia,
        identify,
        request_response,
        metrics_response,
        torrent_response,
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

                    SwarmEvent::ConnectionEstablished { peer_id: connected_peer, endpoint, .. } => {
                        let direction = if endpoint.is_dialer() { "outbound" } else { "inbound" };
                        println!("\n[CONNECT] ═══════════════════════════════════════════════════════════════════════════");
                        println!("[CONNECT] ✓ Connection established!");
                        println!("[CONNECT]   Peer ID: {}", connected_peer);
                        println!("[CONNECT]   Direction: {}", direction);
                        println!("[CONNECT]   Endpoint: {:?}", endpoint);
                        println!("[CONNECT] ═══════════════════════════════════════════════════════════════════════════\n");

                        if !bootstrapped {
                            // Start Kademlia bootstrap
                            if let Err(e) = swarm.behaviour_mut().kademlia.bootstrap() {
                                eprintln!("[WARN] Bootstrap failed: {:?}", e);
                            } else {
                                println!("[DHT] ✓ Started Kademlia bootstrap");
                                bootstrapped = true;
                            }
                        }
                        
                        // Check if we have pending downloads for this peer
                        let info_hash_opt = {
                            let s = state.read().await;
                            let mut found_info_hash = None;
                            for (info_hash, download) in &s.active_downloads {
                                if download.metadata.is_none() {
                                    if let Some(peer_id) = download.peer_id {
                                        if peer_id == connected_peer {
                                            found_info_hash = Some(info_hash.clone());
                                            break;
                                        }
                                    }
                                }
                            }
                            found_info_hash
                        };
                        
                        if let Some(info_hash) = info_hash_opt {
                            let _ = swarm.behaviour_mut().torrent_response.send_request(
                                &connected_peer,
                                TorrentMessage::RequestMetadata {
                                    info_hash: info_hash.clone(),
                                }
                            );
                            println!("[TORRENT] Requested metadata for {} from {}", &info_hash[..16], connected_peer);
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
                                        eprintln!("[DHT] ❌ Failed to announce shard: {:?}", e);
                                    } else {
                                        println!("\n[DHT] ═══════════════════════════════════════════════════════════════════════════");
                                        println!("[DHT] ✓✓✓ ANNOUNCED SHARD {} TO DHT ✓✓✓", shard_id);
                                        println!("[DHT]   Cluster: {}", cluster_name);
                                        println!("[DHT]   Shard ID: {}", shard_id);
                                        println!("[DHT]   Peer ID: {}", peer_id);
                                        println!("[DHT]   Layers: {}-{}", 
                                            shard_id * (args.total_layers / args.total_shards),
                                            if shard_id == args.total_shards - 1 { args.total_layers } 
                                            else { (shard_id + 1) * (args.total_layers / args.total_shards) }
                                        );
                                        println!("[DHT] ═══════════════════════════════════════════════════════════════════════════\n");
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
                                println!("\n[DISCOVERY] ═══════════════════════════════════════════════════════════════════════════");
                                println!("[DISCOVERY] 🔍 Found shard record in DHT!");
                                
                                // Process discovered shard
                                let mut s = state.write().await;
                                if let Some(ann) = s.discovery.process_shard_record(&peer_record.record) {
                                    println!("[DISCOVERY] ✓ Processed shard announcement:");
                                    println!("[DISCOVERY]   Shard ID: {}", ann.shard_id);
                                    println!("[DISCOVERY]   Peer ID: {}", ann.peer_id);
                                    println!("[DISCOVERY]   Layers: {}-{}", ann.layer_start, ann.layer_end);
                                    println!("[DISCOVERY]   Model: {}", ann.model_name);
                                    println!("[DISCOVERY]   Multiaddr: {}", ann.multiaddr);
                                }

                                let status = s.discovery.status();
                                println!("[PIPELINE] Status: {}", status);
                                println!("[DISCOVERY] ═══════════════════════════════════════════════════════════════════════════\n");
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
                                println!("\n[REQUEST] ═══════════════════════════════════════════════════════════════════════════");
                                println!("[REQUEST] 📥 Received message from peer: {}", peer);
                                println!("[REQUEST] Message: {}", request.message);
                                println!("[REQUEST] ═══════════════════════════════════════════════════════════════════════════\n");
                                
                                // Parse command from message
                                if let Ok(cmd) = serde_json::from_str::<Command>(&request.message) {
                                    println!("[COMMAND] ✓ Parsed command: {}", cmd.command);
                                    println!("[COMMAND]   Request ID: {}", cmd.request_id);
                                    println!("[COMMAND]   From: {} → To: {}", cmd.from, peer);
                                    println!("[COMMAND]   Params: {:?}", cmd.params);
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
                                                            Err(_e) => {
                                                                // Shard not found locally - start torrent download
                                                                println!("[LOAD_SHARD] Shard {} not found locally, starting torrent download", shard_id);
                                                                
                                                                // Start download from the requesting peer (they likely have it)
                                                                match s.start_download(shard_id, peer) {
                                                                    Ok(info_hash) => {
                                                                        let mut result = HashMap::new();
                                                                        result.insert("shard_id".to_string(), serde_json::json!(shard_id));
                                                                        result.insert("status".to_string(), serde_json::json!("downloading"));
                                                                        result.insert("info_hash".to_string(), serde_json::json!(info_hash.clone()));
                                                                        
                                                                        // Request metadata will be sent in event loop
                                                                        // Store pending request
                                                                        
                                                                        CommandResponse::success(
                                                                            &cmd.command,
                                                                            &cmd.request_id,
                                                                            &peer_id.to_string(),
                                                                            &cmd.from,
                                                                            result,
                                                                        )
                                                                    }
                                                                    Err(e) => {
                                                                        CommandResponse::error(
                                                                            &cmd.command,
                                                                            &cmd.request_id,
                                                                            &peer_id.to_string(),
                                                                            &cmd.from,
                                                                            &format!("Failed to start download: {}", e),
                                                                        )
                                                                    }
                                                                }
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
                                            println!("\n[EXECUTE_TASK] ═══════════════════════════════════════════════════════════════════════════");
                                            println!("[EXECUTE_TASK] Processing inference task...");
                                            
                                            s.handle_inference_request();
                                            
                                            // Check task type
                                            let task_type = cmd.params.get("task_type")
                                                .and_then(|v| v.as_str())
                                                .unwrap_or("unknown");
                                            
                                            println!("[EXECUTE_TASK]   Task type: {}", task_type);
                                            
                                            if task_type == "llama_fragment" || task_type == "ai_inference" {
                                                let input_data = cmd.params.get("input_data").and_then(|v| v.as_str()).unwrap_or("");
                                                let max_tokens = cmd.params.get("max_tokens").and_then(|v| v.as_u64()).unwrap_or(256);
                                                let temperature = cmd.params.get("temperature").and_then(|v| v.as_f64()).unwrap_or(0.7);
                                                let current_shard_id = cmd.params.get("shard_id").and_then(|v| v.as_u64()).unwrap_or(s.shard_id as u64) as u32;
                                                let layer_start = cmd.params.get("layer_start").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
                                                let layer_end = cmd.params.get("layer_end").and_then(|v| v.as_u64()).unwrap_or(32) as u32;
                                                
                                                println!("[EXECUTE_TASK]   Shard ID: {}", current_shard_id);
                                                println!("[EXECUTE_TASK]   Layers: {}-{}", layer_start, layer_end);
                                                println!("[EXECUTE_TASK]   Input data length: {} chars", input_data.len());
                                                println!("[EXECUTE_TASK]   Max tokens: {}, Temperature: {:.2}", max_tokens, temperature);
                                                println!("[EXECUTE_TASK]   Processing inference through shard {}...", current_shard_id);
                                                // Ensure shard is loaded before processing
                                                let current_shard_id = s.shard_id;
                                                let shard_load_error = if !s.is_shard_loaded(current_shard_id) {
                                                    match s.load_shard_file(current_shard_id) {
                                                        Ok(shard_path) => {
                                                            println!("[INFERENCE] Loaded shard {} from: {}", current_shard_id, shard_path.display());
                                                            None
                                                        }
                                                        Err(e) => {
                                                            s.complete_request(false);
                                                            Some(CommandResponse::error(
                                                                &cmd.command,
                                                                &cmd.request_id,
                                                                &peer_id.to_string(),
                                                                &cmd.from,
                                                                &format!("Shard {} not loaded: {}. Use LOAD_SHARD command first.", current_shard_id, e),
                                                            ))
                                                        }
                                                    }
                                                } else {
                                                    None
                                                };
                                                
                                                if let Some(error_response) = shard_load_error {
                                                    error_response
                                                } else {
                                                    // Process the fragment/inference request
                                                    // Extract input data from command
                                                    let input_data = cmd.params.get("input_data")
                                                        .and_then(|v| v.as_str())
                                                        .unwrap_or("")
                                                        .to_string();
                                                    
                                                    // Get inference parameters
                                                    let max_tokens = cmd.params.get("max_tokens")
                                                        .and_then(|v| v.as_u64())
                                                        .unwrap_or(100) as u32;
                                                    let temperature = cmd.params.get("temperature")
                                                        .and_then(|v| v.as_f64())
                                                        .unwrap_or(0.7) as f32;
                                                    
                                                    println!("[EXECUTE_TASK] Processing on shard {} (layers {}-{}): {}",
                                                        s.shard_id,
                                                        s.announcement.layer_start,
                                                        s.announcement.layer_end,
                                                        if input_data.len() > 50 {
                                                            format!("{}...", &input_data[..50])
                                                        } else {
                                                            input_data.clone()
                                                        }
                                                    );
                                                    
                                                    // In production, this would:
                                                    // 1. Load the .gguf shard file using llama.cpp or candle
                                                    // 2. Process the input through the shard's layers
                                                    // 3. Return the processed activations/output
                                                    
                                                    // For now, simulate processing with the loaded shard
                                                    // The shard file is loaded and ready at: s.loaded_shards.get(&s.shard_id)
                                                    let shard_path = s.loaded_shards.get(&s.shard_id)
                                                        .map(|p| p.to_string_lossy().to_string())
                                                        .unwrap_or_else(|| "unknown".to_string());
                                                    
                                                    // Simulate processing time based on input length
                                                    let processing_time = 50.0 + (input_data.len() as f64 * 0.1);
                                                    
                                                    // Create output that shows the question was processed through this shard
                                                    // In pipeline parallelism, each shard processes the activations from the previous shard
                                                    let output = if s.shard_id == 0 {
                                                        // First shard: process the input question
                                                        format!("[Shard {} processed input: '{}' through layers {}-{} using {}]",
                                                            s.shard_id, input_data, s.announcement.layer_start, s.announcement.layer_end, shard_path)
                                                    } else {
                                                        // Subsequent shards: process activations from previous shard
                                                        format!("[Shard {} processed activations through layers {}-{} using {}]",
                                                            s.shard_id, s.announcement.layer_start, s.announcement.layer_end, shard_path)
                                                    };
                                                    
                                                    let mut result = HashMap::new();
                                                    result.insert("output".to_string(), serde_json::json!(output));
                                                    result.insert("shard_id".to_string(), serde_json::json!(s.shard_id));
                                                    result.insert("layer_start".to_string(), serde_json::json!(s.announcement.layer_start));
                                                    result.insert("layer_end".to_string(), serde_json::json!(s.announcement.layer_end));
                                                    result.insert("tokens_generated".to_string(), serde_json::json!(max_tokens.min(50)));
                                                    result.insert("processing_time_ms".to_string(), serde_json::json!(processing_time));
                                                    
                                                    s.complete_request(true);
                                                    
                                                    CommandResponse::success(
                                                        &cmd.command,
                                                        &cmd.request_id,
                                                        &peer_id.to_string(),
                                                        &cmd.from,
                                                        result,
                                                    )
                                                }
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
                                        
                                        // Get status string before dropping s
                                        let status_string = s.get_status_string();
                                        
                                        println!("\n[RESPONSE] ═══════════════════════════════════════════════════════════════════════════");
                                        println!("[RESPONSE] 📤 Sending response to peer: {}", peer);
                                        println!("[RESPONSE]   Command: {}", cmd.command);
                                        println!("[RESPONSE]   Request ID: {}", cmd.request_id);
                                        println!("[RESPONSE]   Status: {:?}", response.status);
                                        if let Some(ref result) = response.result {
                                            println!("[RESPONSE]   Result keys: {:?}", result.keys().collect::<Vec<_>>());
                                            if let Some(output) = result.get("output").and_then(|v| v.as_str()) {
                                                println!("[RESPONSE]   Output (first 200 chars): {}", 
                                                    if output.len() > 200 { &output[..200] } else { output });
                                            }
                                        }
                                        if let Some(ref error) = response.error {
                                            println!("[RESPONSE]   Error: {}", error);
                                        }
                                        
                                        // Send response as JsonMessage
                                        let response_json = serde_json::to_string(&response).unwrap_or_default();
                                        let response_msg = JsonMessage::new(peer_id.to_string(), response_json);
                                        if let Err(e) = swarm.behaviour_mut().request_response.send_response(
                                            channel,
                                            response_msg,
                                        ) {
                                            eprintln!("[RESPONSE] ❌ Failed to send response: {:?}", e);
                                        } else {
                                            println!("[RESPONSE] ✓ Response sent successfully");
                                        }
                                        println!("[RESPONSE] ═══════════════════════════════════════════════════════════════════════════\n");
                                        
                                        println!("[STATUS] {}", status_string);
                                        response
                                    };
                                } else {
                                    eprintln!("[REQUEST] ❌ Failed to parse command JSON: {}", request.message);
                                }
                            }

                            ShardBehaviourEvent::Identify(libp2p::identify::Event::Received { peer_id: identified_peer, info }) => {
                                println!("[IDENTIFY] {} running {}", identified_peer, info.agent_version);
                            }
                            
                            // Handle torrent protocol messages
                            ShardBehaviourEvent::TorrentResponse(request_response::Event::Message {
                                peer,
                                message: request_response::Message::Request { request, channel, request_id: _ },
                                ..
                            }) => {
                                // Handle incoming torrent requests (serving files)
                                let mut s = state.write().await;
                                
                                match request {
                                    TorrentMessage::ListFiles => {
                                        let files: Vec<TorrentFileInfo> = s.get_torrent_file_list()
                                            .into_iter()
                                            .cloned()
                                            .collect();
                                        
                                        let response = TorrentMessage::FileList { files };
                                        let _ = swarm.behaviour_mut().torrent_response.send_response(channel, response);
                                    }
                                    
                                    TorrentMessage::RequestMetadata { info_hash } => {
                                        // Find file and return metadata
                                        if let Some(file_info) = s.torrent_files.get(&info_hash) {
                                            // Load file and create metadata
                                            let file_path = s.shards_dir.join(&file_info.filename);
                                            if let Ok(file_data) = std::fs::read(&file_path) {
                                                let piece_size = 64 * 1024; // 64 KB
                                                let mut pieces = Vec::new();
                                                
                                                for chunk in file_data.chunks(piece_size) {
                                                    let mut hasher = Sha256::new();
                                                    hasher.update(chunk);
                                                    pieces.push(format!("{:x}", hasher.finalize()));
                                                }
                                                
                                                let metadata = TorrentMetadata {
                                                    info_hash: info_hash.clone(),
                                                    filename: file_info.filename.clone(),
                                                    file_size: file_info.size,
                                                    piece_size: piece_size as u64,
                                                    pieces,
                                                    announce: vec![],
                                                };
                                                
                                                let response = TorrentMessage::Metadata { metadata };
                                                let _ = swarm.behaviour_mut().torrent_response.send_response(channel, response);
                                            }
                                        }
                                    }
                                    
                                    TorrentMessage::RequestPiece { info_hash, piece_index } => {
                                        // Serve piece data
                                        if let Some(file_info) = s.torrent_files.get(&info_hash) {
                                            let file_path = s.shards_dir.join(&file_info.filename);
                                            if let Ok(file_data) = std::fs::read(&file_path) {
                                                let piece_size = 64 * 1024;
                                                let start = (piece_index as usize) * piece_size;
                                                let end = std::cmp::min(start + piece_size, file_data.len());
                                                
                                                if start < file_data.len() {
                                                    let piece_data = file_data[start..end].to_vec();
                                                    let response = TorrentMessage::PieceData {
                                                        info_hash: info_hash.clone(),
                                                        piece_index,
                                                        data: piece_data,
                                                    };
                                                    let _ = swarm.behaviour_mut().torrent_response.send_response(channel, response);
                                                }
                                            }
                                        }
                                    }
                                    
                                    _ => {}
                                }
                            }
                            
                            ShardBehaviourEvent::TorrentResponse(request_response::Event::Message {
                                peer,
                                message: request_response::Message::Response { response, .. },
                                ..
                            }) => {
                                // Handle torrent responses (downloading files)
                                let mut s = state.write().await;
                                
                                match response {
                                    TorrentMessage::Metadata { metadata } => {
                                        println!("[TORRENT] Received metadata for: {} ({} pieces)", metadata.filename, metadata.pieces.len());
                                        
                                        if let Some(download) = s.active_downloads.get_mut(&metadata.info_hash) {
                                            download.metadata = Some(metadata.clone());
                                            download.total_pieces = metadata.pieces.len();
                                            
                                            // Request all pieces
                                            if let Some(peer_id) = download.peer_id {
                                                for i in 0..metadata.pieces.len() {
                                                    let _ = swarm.behaviour_mut().torrent_response.send_request(
                                                        &peer_id,
                                                        TorrentMessage::RequestPiece {
                                                            info_hash: metadata.info_hash.clone(),
                                                            piece_index: i as u64,
                                                        }
                                                    );
                                                }
                                                println!("[TORRENT] Requested {} pieces from {}", metadata.pieces.len(), peer_id);
                                            }
                                        }
                                    }
                                    
                                    TorrentMessage::PieceData { info_hash, piece_index, data } => {
                                        if let Some(download) = s.active_downloads.get_mut(&info_hash) {
                                            download.pieces.insert(piece_index, data);
                                            download.downloaded_pieces += 1;
                                            
                                            println!("[TORRENT] Received piece {}/{} for {}", 
                                                download.downloaded_pieces, download.total_pieces, download.filename);
                                            
                                            // Check if download is complete
                                            if let Ok(Some(file_path)) = s.check_download_complete(&info_hash) {
                                                println!("[TORRENT] ✓ Download complete: {}", file_path.display());
                                            }
                                        }
                                    }
                                    
                                    _ => {}
                                }
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

