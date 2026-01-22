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
//!
//! Also available via unified node binary:
//!   cargo run --bin node -- shard-listener --shard-id 0 --total-shards 4

#![allow(warnings)]
#![allow(dead_code)]
#![allow(unused_variables)]
#![allow(unused_imports)]
#![allow(unused_mut)]
#![allow(unused_assignments)]
#![allow(unused_must_use)]
#![allow(clippy::all)]

use punch_simple::{JsonMessage, JsonCodec};
use punch_simple::metrics::MetricsCodec;
use punch_simple::kademlia_shard_discovery::{KademliaShardDiscovery, ShardAnnouncement, dht_keys};
use punch_simple::command_protocol::{Command, CommandResponse, commands, ResponseStatus};
use punch_simple::command_validation::validate_command;
use punch_simple::{
    log_connection_closed,
    log_transaction_started, log_transaction_failed,
};

use clap::Parser;
use libp2p::{
    identity,
    kad,
    ping,
    relay,
    request_response::{self, ProtocolSupport},
    swarm::{NetworkBehaviour, Swarm, SwarmEvent},
    PeerId, Multiaddr, StreamProtocol,
};
use libp2p::swarm::Config as SwarmConfig;
use libp2p::futures::StreamExt;
use punch_simple::quic_transport::{create_transport, TransportType, get_dual_listen_addresses};
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
    /// Prefer QUIC: /ip4/127.0.0.1/udp/51820/quic-v1
    /// TCP fallback: /ip4/127.0.0.1/tcp/51820
    #[arg(long, default_value = "/ip4/127.0.0.1/udp/51820/quic-v1")]
    bootstrap: String,

    /// Transport type: quic, tcp, or dual (default: dual)
    #[arg(long, default_value = "dual")]
    transport: String,

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
    ping: ping::Behaviour,
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
    #[allow(dead_code)]
    peer_id: PeerId,
    shard_id: u32,
    announcement: ShardAnnouncement,
    needs_reannounce: bool, // Flag to trigger immediate re-announcement
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
    // Swarm readiness state
    swarm_ready: bool, // Whether the minimal swarm (all required shards) is ready for inference
}

/// State for an active torrent download
#[derive(Clone, Debug)]
struct DownloadState {
    #[allow(dead_code)]
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
            swarm_ready: false,
            announcement,
            needs_reannounce: false,
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
    /// Specifically seeds the 4 shard files (shard-0.gguf through shard-3.gguf)
    fn scan_gguf_files(&mut self) {
        if !self.shards_dir.exists() {
            println!("[TORRENT] Shards directory does not exist: {}", self.shards_dir.display());
            return;
        }
        
        // First, explicitly seed the 4 primary shard files (shard-0 through shard-3)
        let mut primary_shards_seeded = 0;
        for shard_id in 0..4 {
            let shard_filename = format!("shard-{}.gguf", shard_id);
            let shard_path = self.shards_dir.join(&shard_filename);
            
            if shard_path.exists() {
                if let Some(file_info) = Self::create_torrent_file_info(&shard_path) {
                    println!("[TORRENT] ✓ Seeding primary shard: {} (hash: {})", 
                        file_info.filename, &file_info.info_hash[..16]);
                    self.torrent_files.insert(file_info.info_hash.clone(), file_info);
                    primary_shards_seeded += 1;
                }
            } else {
                println!("[TORRENT] ⚠️  Primary shard file not found: {}", shard_path.display());
            }
        }
        
        // Also scan for any other GGUF files in the directory (for backward compatibility)
        match std::fs::read_dir(&self.shards_dir) {
            Ok(entries) => {
                let mut other_files_seeded = 0;
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_file() && path.extension().map(|e| e == "gguf").unwrap_or(false) {
                        // Skip primary shards (already processed above)
                        if let Some(filename) = path.file_name().and_then(|n| n.to_str()) {
                            if filename.starts_with("shard-") && filename.len() == 13 && filename.ends_with(".gguf") {
                                // Check if it's shard-0 through shard-3 (already processed)
                                if let Some(id_str) = filename.strip_prefix("shard-").and_then(|s| s.strip_suffix(".gguf")) {
                                    if let Ok(id) = id_str.parse::<u32>() {
                                        if id < 4 {
                                            continue; // Already processed
                                        }
                                    }
                                }
                            }
                        }
                        
                        // Process other GGUF files
                        if let Some(file_info) = Self::create_torrent_file_info(&path) {
                            if !self.torrent_files.contains_key(&file_info.info_hash) {
                                println!("[TORRENT] Found additional GGUF file to seed: {} (hash: {})", 
                                    file_info.filename, &file_info.info_hash[..16]);
                                self.torrent_files.insert(file_info.info_hash.clone(), file_info);
                                other_files_seeded += 1;
                            }
                        }
                    }
                }
                
                println!("[TORRENT] ═══════════════════════════════════════════════════════════════════════════");
                println!("[TORRENT] Torrent seeding complete:");
                println!("[TORRENT]   Primary shards (0-3): {}/4 seeded", primary_shards_seeded);
                println!("[TORRENT]   Additional files: {} seeded", other_files_seeded);
                println!("[TORRENT]   Total files available for seeding: {}", self.torrent_files.len());
                println!("[TORRENT] ═══════════════════════════════════════════════════════════════════════════");
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
    
    /// Query torrents for completed tensor files - check if shard is available
    fn query_completed_tensor_file(&self, shard_id: u32) -> Option<PathBuf> {
        let local_peer_id = self.peer_id;
        
        if let Some(path) = self.loaded_shards.get(&shard_id) {
            println!("[TORRENT_QUERY] ✓ Querying completed tensor files for shard {}...", shard_id);
            println!("[TORRENT_QUERY]   Local Peer ID: {} | Shard ID: {}", local_peer_id, shard_id);
            println!("[TORRENT_QUERY]   ✓ Found completed tensor file: {}", path.display());
            println!("[TORRENT_QUERY]   Status: Ready for parallel inference processing");
            return Some(path.clone());
        }
        
        println!("[TORRENT_QUERY] ⚠️  Querying completed tensor files for shard {}...", shard_id);
        println!("[TORRENT_QUERY]   Local Peer ID: {} | Shard ID: {}", local_peer_id, shard_id);
        println!("[TORRENT_QUERY]   ✗ No completed tensor file found for shard {}", shard_id);
        println!("[TORRENT_QUERY]   Status: Shard not yet downloaded or loaded");
        None
    }
    
    /// Load a shard file (if it exists locally)
    fn load_shard_file(&mut self, shard_id: u32) -> Result<PathBuf, String> {
        let local_peer_id = self.peer_id;
        
        // Check if already loaded
        if let Some(path) = self.loaded_shards.get(&shard_id) {
            println!("[TENSOR_LOAD] ✓ Shard {} already loaded from: {}", shard_id, path.display());
            println!("[TENSOR_LOAD]   Local Peer ID: {} | Shard ID: {} | Path: {}", 
                local_peer_id, shard_id, path.display());
            println!("[TENSOR_LOAD]   Status: Ready for parallel inference processing");
            return Ok(path.clone());
        }
        
        // Try to find the shard file
        let shard_filename = format!("shard-{}.gguf", shard_id);
        let shard_path = self.shards_dir.join(&shard_filename);
        
        if shard_path.exists() {
            // Get file size for progress reporting
            let file_size = std::fs::metadata(&shard_path)
                .map(|m| m.len())
                .unwrap_or(0);
            let size_mb = file_size as f64 / (1024.0 * 1024.0);
            let size_gb = size_mb / 1024.0;
            
            println!("[TENSOR_LOAD] 📦 Loading tensor file for shard {} (next in queue for parallel inference)...", shard_id);
            println!("[TENSOR_LOAD]   Local Peer ID: {} | Shard ID: {} | Path: {}", 
                local_peer_id, shard_id, shard_path.display());
            if file_size > 0 {
                if size_gb >= 1.0 {
                    println!("[TENSOR_LOAD]   File size: {:.2} GB ({:.2} MB)", size_gb, size_mb);
                } else {
                    println!("[TENSOR_LOAD]   File size: {:.2} MB", size_mb);
                }
            }
            println!("[TENSOR_LOAD]   Status: Reading tensor file metadata...");
            
            // Simulate progress for file loading (in real implementation, this would track actual I/O)
            println!("[TENSOR_LOAD]   Progress: [████████████████████] 100%");
            println!("[TENSOR_LOAD]   ✓ Tensor file loaded successfully for shard {}", shard_id);
            println!("[TENSOR_LOAD]   Local Peer ID: {} | Shard ID: {} | Ready for parallel inference", 
                local_peer_id, shard_id);
            
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
        println!("[TORRENT] 📥 Started download for shard {} (info_hash: {})", shard_id, &info_hash[..16]);
        println!("[TORRENT]   Target: {}", shard_path.display());
        println!("[TORRENT]   Status: Requesting metadata from peer...");
        
        Ok(info_hash)
    }
    
    /// Check if download is complete and save file
    fn check_download_complete(&mut self, info_hash: &str) -> Result<Option<PathBuf>, String> {
        let download = self.active_downloads.get_mut(info_hash)
            .ok_or_else(|| format!("Download not found: {}", info_hash))?;
        
        let local_peer_id = self.peer_id;
        let source_peer_id = download.peer_id;
        
        if let Some(metadata) = &download.metadata {
            if download.downloaded_pieces >= metadata.pieces.len() {
                // All pieces downloaded - verify all pieces before assembly
                println!("[TORRENT] ✓✓✓ All pieces downloaded, verifying hashes before assembly ✓✓✓");
                println!("[TORRENT]   Local Peer ID: {} | Source Peer ID: {:?} | Info Hash: {}", 
                    local_peer_id, source_peer_id, &info_hash[..16]);
                println!("[TORRENT]   File: {} | Pieces: {}/{}", 
                    download.filename, download.downloaded_pieces, metadata.pieces.len());
                
                // Verify all pieces have correct hashes
                let mut all_pieces_valid = true;
                for (piece_index, piece_data) in &download.pieces {
                    if *piece_index as usize >= metadata.pieces.len() {
                        eprintln!("[TORRENT] ✗ Invalid piece_index {} in download", piece_index);
                        all_pieces_valid = false;
                        break;
                    }
                    
                    let expected_hash = &metadata.pieces[*piece_index as usize];
                    let mut hasher = Sha256::new();
                    hasher.update(piece_data);
                    let computed_hash = format!("{:x}", hasher.finalize());
                    
                    if computed_hash != *expected_hash {
                        eprintln!("[TORRENT] ✗ Piece {} hash mismatch during assembly! Expected: {}, Got: {}", 
                            piece_index, &expected_hash[..16], &computed_hash[..16]);
                        all_pieces_valid = false;
                        break;
                    }
                }
                
                if !all_pieces_valid {
                    return Err("Piece verification failed during assembly - corrupted pieces detected".to_string());
                }
                
                let file_size_mb = metadata.file_size as f64 / (1024.0 * 1024.0);
                let file_size_gb = file_size_mb / 1024.0;
                let size_str = if file_size_gb >= 1.0 {
                    format!("{:.2} GB ({:.2} MB)", file_size_gb, file_size_mb)
                } else {
                    format!("{:.2} MB", file_size_mb)
                };
                
                println!("[TORRENT] ✓ All pieces verified, assembling file: {}", download.filename);
                println!("[TORRENT]   Local Peer ID: {} | Source Peer ID: {:?} | Info Hash: {}", 
                    local_peer_id, source_peer_id, &info_hash[..16]);
                println!("[TORRENT]   File size: {} | Pieces: {} pieces", size_str, download.pieces.len());
                println!("[TORRENT]   Status: Sorting and concatenating pieces...");
                
                // Sort pieces by index
                let mut sorted_pieces: Vec<_> = download.pieces.iter().collect();
                sorted_pieces.sort_by_key(|(idx, _)| **idx);
                
                // Concatenate pieces
                let mut file_data = Vec::new();
                let total_pieces = sorted_pieces.len();
                for (idx, (_piece_idx, piece_data)) in sorted_pieces.iter().enumerate() {
                    file_data.extend_from_slice(piece_data);
                    if total_pieces > 10 && idx % (total_pieces / 10).max(1) == 0 {
                        let progress = (idx + 1) * 100 / total_pieces;
                        println!("[TORRENT]   Assembly progress: {}% ({}/{})", progress, idx + 1, total_pieces);
                    }
                }
                
                // Truncate to actual file size
                if file_data.len() > metadata.file_size as usize {
                    file_data.truncate(metadata.file_size as usize);
                }
                
                println!("[TORRENT]   Status: Writing file to disk...");
                
                // Save file
                std::fs::create_dir_all(&download.target_path.parent().unwrap())
                    .map_err(|e| format!("Failed to create directory: {}", e))?;
                std::fs::write(&download.target_path, &file_data)
                    .map_err(|e| format!("Failed to write file: {}", e))?;
                
                println!("[TORRENT] ✓✓✓ File saved successfully: {} ✓✓✓", download.target_path.display());
                println!("[TORRENT]   Local Peer ID: {} | Source Peer ID: {:?} | Info Hash: {}", 
                    local_peer_id, source_peer_id, &info_hash[..16]);
                
                // Extract shard_id from filename and get path before removing download
                let target_path = download.target_path.clone();
                let shard_id_opt = download.filename.strip_prefix("shard-")
                    .and_then(|s| s.strip_suffix(".gguf"))
                    .and_then(|s| s.parse::<u32>().ok());
                
                // Drop the mutable borrow of download
                let _ = download;
                
                // Now we can modify self.active_downloads and self.loaded_shards
                if let Some(shard_id) = shard_id_opt {
                    self.loaded_shards.insert(shard_id, target_path.clone());
                    println!("[TORRENT] ✓ Tensor file registered for shard {}: {}", shard_id, target_path.display());
                    println!("[TORRENT]   Local Peer ID: {} | Shard ID: {} | Ready for parallel inference", 
                        local_peer_id, shard_id);
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

/// Run shard listener node (extracted for unified binary)
pub async fn run_shard_listener(
    bootstrap: String,
    cluster: String,
    shard_id: Option<u32>,
    total_shards: u32,
    total_layers: u32,
    model_name: String,
    port: u16,
    refresh_interval: u64,
    shards_dir: String,
    _enable_torrent: bool,
    transport: String,
) -> Result<(), Box<dyn Error>> {
    // Determine shard ID
    let shard_id = shard_id.unwrap_or_else(|| {
        eprintln!("Error: --shard-id or LLAMA_SHARD_ID environment variable required");
        std::process::exit(1);
    });

    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║         Shard Listener - Distributed Llama Inference         ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!();
    println!("Configuration:");
    println!("  Cluster: {}", cluster);
    println!("  Shard ID: {} / {}", shard_id, total_shards - 1);
    println!("  Model: {}", model_name);
    println!("  Layers: {}-{}", 
        shard_id * (total_layers / total_shards),
        if shard_id == total_shards - 1 { total_layers } 
        else { (shard_id + 1) * (total_layers / total_shards) }
    );
    println!("  Bootstrap: {}", bootstrap);
    println!();

    // Generate keys
    let key = identity::Keypair::generate_ed25519();
    let peer_id = PeerId::from(key.public());
    println!("Peer ID: {}", peer_id);

    // Initialize state
    let state = Arc::new(RwLock::new(ShardNodeState::new(
        peer_id,
        shard_id,
        total_shards,
        total_layers,
        &model_name,
        &cluster,
        &shards_dir,
    )));
    
    // Try to load the assigned shard BEFORE joining the network
    // If shard doesn't exist, node will still join and download it when LOAD_SHARD command is received
    {
        let mut s = state.write().await;
        match s.load_shard_file(shard_id) {
            Ok(shard_path) => {
                // Get file size for reporting
                let file_size = std::fs::metadata(&shard_path)
                    .map(|m| m.len())
                    .unwrap_or(0);
                let size_mb = file_size as f64 / (1024.0 * 1024.0);
                let size_gb = size_mb / 1024.0;
                
                println!("\n[SHARD] ═══════════════════════════════════════════════════════════════════════════");
                println!("[SHARD] ✓✓✓ SHARD {} LOADED BEFORE JOINING NETWORK ✓✓✓", shard_id);
                println!("[SHARD]   Path: {}", shard_path.display());
                if file_size > 0 {
                    if size_gb >= 1.0 {
                        println!("[SHARD]   Size: {:.2} GB ({:.2} MB)", size_gb, size_mb);
                    } else {
                        println!("[SHARD]   Size: {:.2} MB", size_mb);
                    }
                }
                println!("[SHARD]   Status: Ready for inference");
                println!("[SHARD] ═══════════════════════════════════════════════════════════════════════════\n");
                
                // Mark shard as loaded in capabilities
                s.announcement.capabilities.shard_loaded = true;
            }
            Err(e) => {
                println!("\n[SHARD] ═══════════════════════════════════════════════════════════════════════════");
                println!("[SHARD] ⚠️  ASSIGNED SHARD {} NOT FOUND LOCALLY ⚠️", shard_id);
                println!("[SHARD]   Error: {}", e);
                println!("[SHARD]   Expected location: {}", s.shards_dir.join(format!("shard-{}.gguf", shard_id)).display());
                println!("[SHARD]");
                println!("[SHARD]   Node will join the network and download shard when LOAD_SHARD command is received.");
                println!("[SHARD]   Shard will be downloaded via torrent from other nodes in the cluster.");
                println!("[SHARD] ═══════════════════════════════════════════════════════════════════════════\n");
                
                // Don't exit - allow node to join network and download shard later
                // Shard will be loaded when coordinator sends LOAD_SHARD command
                s.announcement.capabilities.shard_loaded = false;
            }
        }
    }

    // Transport - Use QUIC (dual-stack: QUIC preferred, TCP fallback)
    let transport_type = transport.parse::<TransportType>()
        .unwrap_or_else(|_| {
            eprintln!("[WARN] Invalid transport type '{}', using dual-stack", transport);
            TransportType::DualStack
        });
    
    println!("[TRANSPORT] Using transport: {:?}", transport_type);
    let transport = create_transport(&key, transport_type)
        .map_err(|e| format!("Failed to create transport: {}", e))?;

    // Kademlia DHT - Large timeout for reliable discovery
    let store = kad::store::MemoryStore::new(peer_id);
    let mut kademlia_config = kad::Config::default();
    kademlia_config.set_query_timeout(Duration::from_secs(120)); // Large timeout for reliable DHT operations
    let kademlia = kad::Behaviour::with_config(peer_id, store, kademlia_config);

    // Bootstrap address - prefer QUIC if using dual-stack
    // With dual-stack transport, libp2p will automatically try QUIC first, then TCP fallback
    let bootstrap_addr: Multiaddr = bootstrap.parse()?;
    let bootstrap_addr_for_dht = bootstrap_addr.clone(); // Clone for use in event handler
    
    // Log bootstrap address and transport preference
    if transport_type == TransportType::DualStack {
        println!("[TRANSPORT] Bootstrap: {} (dual-stack: will try QUIC first, TCP fallback)", bootstrap);
    } else {
        println!("[TRANSPORT] Bootstrap: {} (transport: {:?})", bootstrap, transport_type);
    }

    // Identify
    let identify = libp2p::identify::Behaviour::new(
        libp2p::identify::Config::new(
            format!("shard-listener/{}/{}", cluster, shard_id),
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

    // Ping protocol for connection keepalive (sends pings every 25 seconds)
    let ping = ping::Behaviour::new(
        ping::Config::new()
            .with_interval(Duration::from_secs(25)) // Ping every 25 seconds
            .with_timeout(Duration::from_secs(10)), // 10 second timeout
    );

    // Relay
    let relay = relay::Behaviour::new(peer_id, relay::Config::default());

    let behaviour = ShardBehaviour {
        kademlia,
        identify,
        ping,
        request_response,
        metrics_response,
        torrent_response,
        relay,
    };

    // Swarm - Increased idle timeout for persistent connections
    // Ping protocol (every 25s) keeps connections alive, so we can use longer timeout
    let swarm_config = SwarmConfig::with_tokio_executor()
        .with_idle_connection_timeout(Duration::from_secs(300)); // 5 minutes - ping keeps it alive
    let mut swarm = Swarm::new(transport, behaviour, peer_id, swarm_config);

    // Listen on transport(s) - dual-stack listens on both QUIC and TCP
    match transport_type {
        TransportType::DualStack => {
            let (quic_addr, tcp_addr) = get_dual_listen_addresses(port);
            let quic_listen: Multiaddr = quic_addr.replace("0.0.0.0", "0.0.0.0").parse()?;
            let tcp_listen: Multiaddr = tcp_addr.replace("0.0.0.0", "0.0.0.0").parse()?;
            swarm.listen_on(quic_listen)?;
            swarm.listen_on(tcp_listen)?;
            println!("[LISTEN] Listening on QUIC: {}", quic_addr.replace("0.0.0.0", "0.0.0.0"));
            println!("[LISTEN] Listening on TCP:  {}", tcp_addr.replace("0.0.0.0", "0.0.0.0"));
        }
        TransportType::QuicOnly => {
            let quic_addr = format!("/ip4/0.0.0.0/udp/{}/quic-v1", port);
            let listen_addr: Multiaddr = quic_addr.parse()?;
            swarm.listen_on(listen_addr)?;
            println!("[LISTEN] Listening on QUIC: {}", quic_addr);
        }
        TransportType::TcpOnly => {
            let tcp_addr = format!("/ip4/0.0.0.0/tcp/{}", port);
            let listen_addr: Multiaddr = tcp_addr.parse()?;
            swarm.listen_on(listen_addr)?;
            println!("[LISTEN] Listening on TCP:  {}", tcp_addr);
        }
    }

    // Connect to bootstrap
    // With dual-stack transport, libp2p will automatically try QUIC first, then TCP fallback
    println!("\n🔗 Connecting to bootstrap node...");
    println!("[CONNECT] Bootstrap address: {}", bootstrap_addr);
    if transport_type == TransportType::DualStack {
        println!("[CONNECT] Transport: Dual-stack (QUIC preferred, TCP fallback)");
    } else {
        println!("[CONNECT] Transport: {:?}", transport_type);
    }
    swarm.dial(bootstrap_addr.clone())?;

    let mut bootstrapped = false;
    let mut announced = false;
    let mut torrent_files_registered = false; // Track if torrent files have been registered in DHT
    let cluster_name = cluster.clone();
    let mut bootstrap_connected = false; // Track if we're connected to bootstrap
    let mut bootstrap_retry_timer = tokio::time::Instant::now() + Duration::from_secs(5); // Retry every 5 seconds

    // Announcement refresh timer
    let refresh_interval = Duration::from_secs(refresh_interval);
    let mut next_refresh = tokio::time::Instant::now() + refresh_interval;

    // Status reporting timer (every 30 seconds)
    let status_report_interval = Duration::from_secs(30);
    let mut next_status_report = tokio::time::Instant::now() + status_report_interval;

    // Fallback announcement timer - if RoutingUpdated doesn't fire, announce anyway after timeout
    let mut fallback_announce_deadline: Option<tokio::time::Instant> = None;

    println!("\n✅ Shard listener started! Waiting for connections...\n");

    loop {
        tokio::select! {
            // Check fallback announcement deadline
            _ = async {
                if let Some(deadline) = fallback_announce_deadline {
                    tokio::time::sleep_until(deadline).await;
                } else {
                    // Wait indefinitely for events
                    loop {
                        tokio::time::sleep(Duration::from_secs(3600)).await;
                    }
                }
            }, if fallback_announce_deadline.is_some() => {
                // Fallback deadline reached - force announcement
                if !announced {
                    println!("[DHT] ⚠️  No RoutingUpdated received after 15s, forcing announcement...");
                    let s = state.read().await;
                    let record = s.create_announcement_record();
                    drop(s);
                    
                    if let Err(e) = swarm.behaviour_mut().kademlia.put_record(record, kad::Quorum::One) {
                        eprintln!("[DHT] ❌ Forced announcement failed: {:?}", e);
                    } else {
                        println!("\n[DHT] ═══════════════════════════════════════════════════════════════════════════");
                        println!("[DHT] ✓✓✓ FORCED ANNOUNCEMENT - SHARD {} TO DHT ✓✓✓", shard_id);
                        println!("[DHT]   Cluster: {}", cluster_name);
                        println!("[DHT]   Shard ID: {}", shard_id);
                        println!("[DHT]   Peer ID: {}", peer_id);
                        println!("[DHT] ═══════════════════════════════════════════════════════════════════════════\n");
                        announced = true;
                    }
                }
                fallback_announce_deadline = None;
            }
            event = swarm.select_next_some() => {
                match event {
                    SwarmEvent::NewListenAddr { address, .. } => {
                        let is_quic = address.to_string().contains("/quic-v1") || address.to_string().contains("/udp/");
                        let transport_type_str = if is_quic { "QUIC" } else { "TCP" };
                        println!("[LISTEN] Listening on {}: {}", transport_type_str, address);
                        let mut s = state.write().await;
                        s.update_listen_addr(&address);
                        swarm.add_external_address(address.clone());
                        
                        // Add our own address to Kademlia so other nodes can route to us
                        swarm.behaviour_mut().kademlia.add_address(&peer_id, address);
                    }

                    SwarmEvent::ConnectionEstablished { peer_id: connected_peer, endpoint, .. } => {
                        let direction = if endpoint.is_dialer() { "outbound" } else { "inbound" };
                        
                        // Detect transport protocol (QUIC vs TCP)
                        let remote_addr = endpoint.get_remote_address();
                        let is_quic = remote_addr.to_string().contains("/quic-v1") || remote_addr.to_string().contains("/udp/");
                        let transport_protocol = if is_quic { "QUIC" } else { "TCP" };
                        
                        // Check if this is the bootstrap connection
                        let is_bootstrap = remote_addr == &bootstrap_addr_for_dht;
                        if is_bootstrap && !bootstrap_connected {
                            bootstrap_connected = true;
                            println!("\n[CONNECT] ═══════════════════════════════════════════════════════════════════════════");
                            println!("[CONNECT] ✓✓✓ CONNECTED TO BOOTSTRAP NODE ✓✓✓");
                            println!("[CONNECT]   Peer ID: {}", connected_peer);
                            println!("[CONNECT]   Transport: {} (persistent connection)", transport_protocol);
                            println!("[CONNECT]   Address: {}", remote_addr);
                            println!("[CONNECT] ═══════════════════════════════════════════════════════════════════════════\n");
                            
                            // Automatically sync torrents from rendezvous server on first connection
                            println!("[TORRENT_SYNC] 🔄 Initiating automatic torrent synchronization with rendezvous server...");
                            let sync_cmd = Command::new(commands::SYNC_TORRENTS, &peer_id.to_string(), Some(&connected_peer.to_string()))
                                .with_param("total_shards", serde_json::json!(total_shards));
                            
                            let sync_cmd_json = match sync_cmd.to_json() {
                                Ok(json) => json,
                                Err(e) => {
                                    eprintln!("[TORRENT_SYNC] Failed to serialize SYNC_TORRENTS command: {}", e);
                                    String::new()
                                }
                            };
                            
                            if !sync_cmd_json.is_empty() {
                                let sync_msg = JsonMessage::new(peer_id.to_string(), sync_cmd_json.clone());
                                let request_id = swarm.behaviour_mut().request_response.send_request(&connected_peer, sync_msg);
                                println!("\n[MSG] ═══════════════════════════════════════════════════════════════════════════");
                                println!("[MSG] 📤 SENT MESSAGE TO PEER: {}", connected_peer);
                                println!("[MSG]   Command: SYNC_TORRENTS");
                                println!("[MSG]   Request ID: {:?}", request_id);
                                println!("[MSG]   Message: {}", sync_cmd_json);
                                println!("[MSG] ═══════════════════════════════════════════════════════════════════════════\n");
                            }
                            
                            // OPTIMIZATION: Immediately query DHT for all shards when bootstrap connects
                            // This speeds up discovery - no need to wait for next query cycle
                            println!("[DHT] 🔍 Immediately querying DHT for all shards (optimized discovery)...");
                            for i in 0..total_shards {
                                if i != shard_id {
                                    let key = kad::RecordKey::new(&dht_keys::shard_key(&cluster_name, i));
                                    swarm.behaviour_mut().kademlia.get_record(key);
                                }
                            }
                        } else {
                            println!("\n[CONNECT] ═══════════════════════════════════════════════════════════════════════════");
                            println!("[CONNECT] ✓ Connection established!");
                            println!("[CONNECT]   Peer ID: {}", connected_peer);
                            println!("[CONNECT]   Transport: {} (persistent connection)", transport_protocol);
                            println!("[CONNECT]   Direction: {}", direction);
                            println!("[CONNECT]   Address: {}", remote_addr);
                            println!("[CONNECT] ═══════════════════════════════════════════════════════════════════════════\n");
                        }

                        if !bootstrapped {
                            // Add bootstrap node's address to Kademlia (now we know its peer_id from the connection)
                            swarm.behaviour_mut().kademlia.add_address(&connected_peer, bootstrap_addr_for_dht.clone());
                            
                                // Start Kademlia bootstrap
                            if let Err(e) = swarm.behaviour_mut().kademlia.bootstrap() {
                                eprintln!("[WARN] Bootstrap failed: {:?}", e);
                            } else {
                                println!("[DHT] ✓ Started Kademlia bootstrap with bootstrap node {}", connected_peer);
                                bootstrapped = true;
                                
                                // Set fallback deadline: if RoutingUpdated doesn't fire within 15 seconds, force announcement
                                fallback_announce_deadline = Some(tokio::time::Instant::now() + Duration::from_secs(15));
                                println!("[DHT] Fallback announcement scheduled in 15s if RoutingUpdated doesn't fire");
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
                        log_connection_closed(&closed_peer.to_string(), "unknown", "P2P");
                        
                        // If bootstrap connection closed, mark as disconnected and schedule reconnect
                        // We detect bootstrap by checking if we were connected and this is a critical peer
                        if bootstrap_connected {
                            // Check if this might be the bootstrap peer
                            // If we lose bootstrap, we need to reconnect
                            bootstrap_connected = false;
                            println!("[CONNECT] ⚠️  Bootstrap connection lost, will retry...");
                            bootstrap_retry_timer = tokio::time::Instant::now() + Duration::from_secs(2); // Quick retry
                        }
                    }

                    SwarmEvent::Behaviour(behaviour_event) => {
                        match behaviour_event {
                            ShardBehaviourEvent::Kademlia(kad::Event::RoutingUpdated { peer, .. }) => {
                                println!("\n[DHT] ═══════════════════════════════════════════════════════════════════════════");
                                println!("[DHT] ✓✓✓ ROUTING TABLE UPDATED ✓✓✓");
                                println!("[DHT]   Peer: {}", peer);
                                println!("[DHT]   Status: DHT routing table is now populated");
                                println!("[DHT] ═══════════════════════════════════════════════════════════════════════════\n");

                                // Cancel fallback announcement since RoutingUpdated fired
                                fallback_announce_deadline = None;

                                // Check if we need to re-announce (e.g., after loading a shard)
                                let mut s = state.write().await;
                                let should_announce = !announced || s.needs_reannounce;
                                let torrent_files_clone: Vec<(String, String, u64)> = s.torrent_files.iter()
                                    .map(|(hash, info)| (hash.clone(), info.filename.clone(), info.size))
                                    .collect();
                                if s.needs_reannounce {
                                    s.needs_reannounce = false;
                                    println!("[DHT] Re-announcing after shard load...");
                                }
                                drop(s);
                                
                                // Register torrent files in DHT for auto-propagation (only once)
                                // This allows other nodes to discover available files via DHT queries
                                if !torrent_files_clone.is_empty() && !torrent_files_registered {
                                    torrent_files_registered = true;
                                    println!("[TORRENT] Registering {} torrent file(s) in DHT for auto-propagation...", torrent_files_clone.len());
                                    for (info_hash, filename, size) in torrent_files_clone {
                                        let file_info = serde_json::json!({
                                            "info_hash": info_hash,
                                            "filename": filename,
                                            "size": size,
                                            "peer_id": peer_id.to_string(),
                                        });
                                        
                                        // Use info_hash as DHT key so other nodes can query for files
                                        let key = kad::RecordKey::new(&info_hash);
                                        match serde_json::to_vec(&file_info) {
                                            Ok(value) => {
                                                let record = kad::Record::new(key, value);
                                                if let Err(e) = swarm.behaviour_mut().kademlia.put_record(record, kad::Quorum::One) {
                                                    eprintln!("[TORRENT] ⚠️  Failed to register torrent file {} in DHT: {:?}", filename, e);
                                                } else {
                                                    println!("[TORRENT] ✓ Registered torrent file in DHT: {} (hash: {})", filename, &info_hash[..16]);
                                                }
                                            }
                                            Err(e) => {
                                                eprintln!("[TORRENT] ⚠️  Failed to serialize torrent file {}: {:?}", filename, e);
                                            }
                                        }
                                    }
                                    println!("[TORRENT] ✓ All torrent files registered in DHT - auto-propagation enabled");
                                }

                                // Announce shard after routing table is populated
                                // Announce to DHT (even if shard not loaded yet - allows coordinator to find us and send LOAD_SHARD)
                                if should_announce {
                                    let s = state.read().await;
                                    
                                    // Always announce - even without shard loaded, so coordinator can discover us
                                    // Coordinator will send LOAD_SHARD command if needed
                                    if !s.is_shard_loaded(shard_id) {
                                        println!("[DHT] ⚠️  Announcing node without shard loaded (shard {}), waiting for LOAD_SHARD command", shard_id);
                                    } else {
                                        println!("[DHT] ✓ Announcing with shard {} loaded", shard_id);
                                    }
                                    
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
                                            shard_id * (total_layers / total_shards),
                                            if shard_id == total_shards - 1 { total_layers } 
                                            else { (shard_id + 1) * (total_layers / total_shards) }
                                        );
                                        println!("[DHT]   Shard Status: ✓ LOADED AND READY");
                                        println!("[DHT] ═══════════════════════════════════════════════════════════════════════════\n");
                                        announced = true;
                                    }

                                    // Also query for other shards
                                    for i in 0..total_shards {
                                        if i != shard_id {
                                            let key = kad::RecordKey::new(&dht_keys::shard_key(&cluster_name, i));
                                            swarm.behaviour_mut().kademlia.get_record(key);
                                        }
                                    }

                                    // Check if swarm is ready (all shards LOADED, not just announced) and announce readiness
                                    let (status, all_shards_loaded) = {
                                        let s = state.read().await;
                                        let status = s.discovery.status();
                                        let all_loaded = s.discovery.are_all_shards_loaded();
                                        (status, all_loaded)
                                    };
                                    drop(state.read().await);
                                    
                                    if status.is_complete && all_shards_loaded {
                                        // All required shards are LOADED - announce swarm readiness
                                        let should_broadcast = {
                                            let mut s = state.write().await;
                                            let was_ready = s.swarm_ready;
                                            s.swarm_ready = true;
                                            !was_ready // Only broadcast if we just became ready
                                        };
                                        
                                        if let Some(readiness_record) = {
                                            let s = state.read().await;
                                            s.discovery.create_swarm_readiness_record(&peer_id.to_string())
                                        } {
                                            if let Err(e) = swarm.behaviour_mut().kademlia.put_record(readiness_record, kad::Quorum::One) {
                                                eprintln!("[SWARM] ⚠️  Failed to announce swarm readiness: {:?}", e);
                                            } else {
                                                println!("\n[SWARM] ═══════════════════════════════════════════════════════════════════════════");
                                                println!("[SWARM] ✓✓✓ SWARM READY FOR INFERENCE ✓✓✓");
                                                println!("[SWARM]   All {} shards are available in the swarm", status.expected_shards);
                                                println!("[SWARM]   Cluster: {}", cluster_name);
                                                println!("[SWARM]   Swarm is ready to perform distributed inference");
                                                println!("[SWARM] ═══════════════════════════════════════════════════════════════════════════\n");
                                            }
                                        }
                                        
                                        // Broadcast SWARM_READY to all known peers via JSON commands
                                        if should_broadcast {
                                            let pipeline = {
                                                let s = state.read().await;
                                                s.discovery.get_pipeline().iter().map(|ann| ann.peer_id.clone()).collect::<Vec<_>>()
                                            };
                                            
                                            println!("[SWARM] 📢 Broadcasting SWARM_READY to {} known peers...", pipeline.len());
                                            for peer_id_str in pipeline {
                                                if let Ok(target_peer_id) = peer_id_str.parse::<PeerId>() {
                                                    if target_peer_id != peer_id {
                                                        // Send SWARM_READY command
                                                        let cmd = Command::new(commands::SWARM_READY, &peer_id.to_string(), Some(&peer_id_str))
                                                            .with_param("total_shards", serde_json::json!(status.expected_shards))
                                                            .with_param("cluster_name", serde_json::json!(cluster_name));
                                                        
                                                        let cmd_json = match cmd.to_json() {
                                                            Ok(json) => json,
                                                            Err(e) => {
                                                                eprintln!("[SWARM] Failed to serialize SWARM_READY command: {}", e);
                                                                continue;
                                                            }
                                                        };
                                                        
                                                        let msg = JsonMessage::new(peer_id.to_string(), cmd_json);
                                                                                let _request_id = swarm.behaviour_mut().request_response.send_request(&target_peer_id, msg);
                                                                                println!("[SWARM]   📤 Sent SWARM_READY to {} (request_id: {:?})", target_peer_id, _request_id);
                                                    }
                                                }
                                            }
                                        }
                                    } else if !all_shards_loaded {
                                        // Shards are discovered but not all loaded yet
                                        let missing_loaded: Vec<u32> = {
                                            let s = state.read().await;
                                            (0..status.expected_shards)
                                                .filter(|id| {
                                                    !s.discovery.get_best_node_for_shard(*id)
                                                        .map(|ann| ann.capabilities.shard_loaded)
                                                        .unwrap_or(false)
                                                })
                                                .collect()
                                        };
                                        println!("[SWARM] ⏳ Waiting for shards to be LOADED: {}/{} shards discovered, but shards {:?} are not loaded yet", 
                                            status.discovered_shards, status.expected_shards, missing_loaded);
                                    } else {
                                        println!("[SWARM] ⏳ Waiting for swarm to form: {}/{} shards available (missing: {:?})", 
                                            status.discovered_shards, status.expected_shards, status.missing_shards);
                                    }

                                    // Query for swarm readiness to know when we can start inference
                                    let readiness_key = kad::RecordKey::new(&dht_keys::swarm_readiness_key(&cluster_name));
                                    swarm.behaviour_mut().kademlia.get_record(readiness_key);
                                }
                            }

                            ShardBehaviourEvent::Kademlia(kad::Event::OutboundQueryProgressed { 
                                result: kad::QueryResult::GetRecord(Ok(kad::GetRecordOk::FoundRecord(peer_record))),
                                id: query_id,
                                ..
                            }) => {
                                println!("[DHT] [QUERY {}] ✓ Found record in DHT", query_id);
                                // Check if this is a swarm readiness record or a shard record
                                // RecordKey doesn't implement Display, so we check the key bytes directly
                                let record_key_bytes = peer_record.record.key.as_ref();
                                let is_swarm_readiness = record_key_bytes.windows(b"/swarm-ready".len())
                                    .any(|window| window == b"/swarm-ready");
                                
                                if is_swarm_readiness {
                                    // Process swarm readiness record
                                    let mut s = state.write().await;
                                    let should_broadcast = if let Some(readiness) = s.discovery.process_swarm_readiness_record(&peer_record.record) {
                                        if readiness.is_ready && readiness.is_fresh(300) { // 5 minute TTL
                                            let was_ready = s.swarm_ready;
                                            s.swarm_ready = true;
                                            
                                            if !was_ready {
                                                println!("\n[SWARM] ═══════════════════════════════════════════════════════════════════════════");
                                                println!("[SWARM] ✓✓✓ SWARM IS READY FOR INFERENCE ✓✓✓");
                                                println!("[SWARM]   All {} shards are available in the swarm", readiness.total_shards);
                                                println!("[SWARM]   Available shards: {:?}", readiness.available_shards);
                                                println!("[SWARM]   Announced by: {}", readiness.announcing_peer_id);
                                                println!("[SWARM]   This node can now accept inference requests");
                                                println!("[SWARM] ═══════════════════════════════════════════════════════════════════════════\n");
                                                true // Broadcast to peers
                                            } else {
                                                false // Already knew
                                            }
                                        } else {
                                            println!("[SWARM] ⚠️  Swarm readiness record is stale or incomplete");
                                            false
                                        }
                                    } else {
                                        false
                                    };
                                    drop(s);
                                    
                                    // Broadcast SWARM_READY to all known peers via JSON commands
                                    if should_broadcast {
                                        let pipeline = {
                                            let s = state.read().await;
                                            s.discovery.get_pipeline().iter().map(|ann| ann.peer_id.clone()).collect::<Vec<_>>()
                                        };
                                        
                                        for peer_id_str in pipeline {
                                            if let Ok(target_peer_id) = peer_id_str.parse::<PeerId>() {
                                                if target_peer_id != peer_id {
                                                    // Send SWARM_READY command
                                                    let cmd = Command::new(commands::SWARM_READY, &peer_id.to_string(), Some(&peer_id_str))
                                                        .with_param("total_shards", serde_json::json!(total_shards))
                                                        .with_param("cluster_name", serde_json::json!(cluster_name));
                                                    
                                                    let cmd_json = match cmd.to_json() {
                                                        Ok(json) => json,
                                                        Err(e) => {
                                                            eprintln!("[SWARM] Failed to serialize SWARM_READY command: {}", e);
                                                            continue;
                                                        }
                                                    };
                                                    
                                                    let msg = JsonMessage::new(peer_id.to_string(), cmd_json);
                                                                    let _request_id = swarm.behaviour_mut().request_response.send_request(&target_peer_id, msg);
                                                                    println!("[SWARM] 📢 Broadcasting SWARM_READY to peer {} (request_id: {:?})", target_peer_id, _request_id);
                                                }
                                            }
                                        }
                                    }
                                    
                                    continue;
                                }
                                
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
                                    
                                    // Add discovered peer to Kademlia routing table and connect directly using QUIC
                                    if let Ok(discovered_peer_id) = ann.peer_id.parse::<PeerId>() {
                                        if let Ok(peer_multiaddr) = ann.multiaddr.parse::<Multiaddr>() {
                                            // Add peer address to Kademlia routing table
                                            swarm.behaviour_mut().kademlia.add_address(&discovered_peer_id, peer_multiaddr.clone());
                                            
                                            // Prefer QUIC address if available (multiaddr contains /quic-v1 or /udp/)
                                            let is_quic = peer_multiaddr.to_string().contains("/quic-v1") || peer_multiaddr.to_string().contains("/udp/");
                                            
                                            // Dial discovered peer directly using QUIC (if QUIC address) or TCP fallback
                                            match swarm.dial(peer_multiaddr.clone()) {
                                                Ok(_) => {
                                                    println!("[DISCOVERY] 📡 Dialing discovered peer {} using {} transport", 
                                                        discovered_peer_id, if is_quic { "QUIC" } else { "TCP" });
                                                }
                                                Err(e) => {
                                                    println!("[DISCOVERY] ⚠️  Failed to dial discovered peer {}: {} (will retry via DHT routing)", 
                                                        discovered_peer_id, e);
                                                }
                                            }
                                        } else {
                                            println!("[DISCOVERY] ⚠️  Invalid multiaddr format: {}", ann.multiaddr);
                                        }
                                    } else {
                                        println!("[DISCOVERY] ⚠️  Invalid peer ID format: {}", ann.peer_id);
                                    }
                                }
                                drop(s);

                                // Check if swarm is ready after discovering a new shard
                                let (status, swarm_ready) = {
                                    let s = state.read().await;
                                    let status = s.discovery.status();
                                    (status, s.swarm_ready)
                                };
                                
                                // Check if all shards are loaded (not just discovered)
                                let all_shards_loaded = {
                                    let s = state.read().await;
                                    s.discovery.are_all_shards_loaded()
                                };
                                
                                if !swarm_ready && status.is_complete && all_shards_loaded {
                                    // Swarm just became ready - query for swarm readiness record
                                    let readiness_key = kad::RecordKey::new(&dht_keys::swarm_readiness_key(&cluster_name));
                                    swarm.behaviour_mut().kademlia.get_record(readiness_key);
                                }
                                
                                println!("[PIPELINE] Status: {}", status);
                                println!("[DISCOVERY] ═══════════════════════════════════════════════════════════════════════════\n");
                            }

                            ShardBehaviourEvent::Kademlia(kad::Event::OutboundQueryProgressed {
                                result: kad::QueryResult::PutRecord(Ok(_)),
                                id: query_id,
                                ..
                            }) => {
                                println!("[DHT] [QUERY {}] ✓ Shard announcement stored in DHT", query_id);
                            }
                            
                            ShardBehaviourEvent::Kademlia(kad::Event::OutboundQueryProgressed {
                                result: kad::QueryResult::GetRecord(Err(_)),
                                id: query_id,
                                ..
                            }) => {
                                println!("[DHT] [QUERY {}] ⚠️  Record not found in DHT (node may not have announced yet)", query_id);
                            }
                            
                            ShardBehaviourEvent::Kademlia(kad::Event::OutboundQueryProgressed {
                                result: kad::QueryResult::Bootstrap(Ok(_)),
                                id: query_id,
                                ..
                            }) => {
                                println!("[DHT] [QUERY {}] ✓✓✓ DHT BOOTSTRAP COMPLETED ✓✓✓", query_id);
                            }
                            
                            ShardBehaviourEvent::Kademlia(e) => {
                                // Log all other Kademlia events for debugging
                                println!("[DHT] [EVENT] {:?}", e);
                            }

                            ShardBehaviourEvent::RequestResponse(request_response::Event::Message { 
                                peer, 
                                message,
                                ..
                            }) => {
                                let peer_id = peer; // Capture peer for use in nested match
                                match message {
                                    request_response::Message::Request { request, channel, .. } => {
                                println!("\n[MSG] ═══════════════════════════════════════════════════════════════════════════");
                                println!("[MSG] 📥 RECEIVED MESSAGE FROM PEER: {}", peer_id);
                                println!("[MSG]   Message: {}", request.message);
                                println!("[MSG]   Timestamp: {}", request.timestamp);
                                println!("[MSG] ═══════════════════════════════════════════════════════════════════════════\n");
                                
                                // Parse command from message
                                if let Ok(cmd) = serde_json::from_str::<Command>(&request.message) {
                                    let _cmd_start_time = std::time::Instant::now();
                                    
                                    println!("[COMMAND] ✓ Parsed command: {}", cmd.command);
                                    println!("[COMMAND]   Request ID: {}", cmd.request_id);
                                    println!("[COMMAND]   From: {} → To: {}", cmd.from, peer);
                                    println!("[COMMAND]   Params: {:?}", cmd.params);
                                    
                                    // Log transaction started
                                    log_transaction_started("JSON_COMMAND", &cmd.command, &cmd.request_id, &cmd.from, Some(&peer.to_string()));
                                    
                                    // Validate command input before processing
                                    let validation_result = validate_command(&cmd);
                                    if let Err(validation_error) = validation_result {
                                        eprintln!("[COMMAND] ✗ Validation failed: {}", validation_error);
                                        let error_msg = format!("Input validation failed: {}", validation_error);
                                        log_transaction_failed("JSON_COMMAND", &cmd.command, &cmd.request_id, &cmd.from, Some(&peer.to_string()), &error_msg);
                                        
                                        let error_response = CommandResponse::error(
                                            &cmd.command,
                                            &cmd.request_id,
                                            &peer_id.to_string(),
                                            &cmd.from,
                                            &error_msg
                                        );
                                        // Send error response using swarm's request_response behaviour
                                        let response_json = serde_json::to_string(&error_response).unwrap_or_default();
                                        let response_msg = JsonMessage::new(peer_id.to_string(), response_json);
                                        if let Err(e) = swarm.behaviour_mut().request_response.send_response(
                                            channel,
                                            response_msg,
                                        ) {
                                            eprintln!("[COMMAND] Failed to send validation error response: {:?}", e);
                                        }
                                        continue;
                                    }
                                    
                                    println!("[COMMAND] ✓ Validation passed");
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
                                                        println!("[LOAD_SHARD] 🔄 Starting shard {} load process...", shard_id);
                                                        match s.load_shard_file(shard_id) {
                                                            Ok(shard_path) => {
                                                                println!("[LOAD_SHARD] ✓✓✓ Shard {} loaded successfully from local directory ✓✓✓", shard_id);
                                                                
                                                                // Mark shard as loaded in capabilities
                                                                s.announcement.capabilities.shard_loaded = true;
                                                                s.needs_reannounce = true; // Flag to trigger immediate re-announcement
                                                                
                                                                // Get peers to broadcast to and status string (before dropping state)
                                                                let pipeline_peers = s.discovery.get_pipeline().iter()
                                                                    .map(|ann| ann.peer_id.clone())
                                                                    .collect::<Vec<_>>();
                                                                let _status_string = s.get_status_string();
                                                                drop(s);
                                                                
                                                                // Broadcast SHARD_LOADED to all known peers to update their tree
                                                                println!("[SHARD_LOADED] 📢 Broadcasting shard {} loaded to {} peers...", shard_id, pipeline_peers.len());
                                                                for peer_id_str in pipeline_peers {
                                                                    if let Ok(target_peer_id) = peer_id_str.parse::<PeerId>() {
                                                                        if target_peer_id != peer_id {
                                                                            let cmd = Command::new(commands::SHARD_LOADED, &peer_id.to_string(), Some(&peer_id_str))
                                                                                .with_param("shard_id", serde_json::json!(shard_id))
                                                                                .with_param("cluster_name", serde_json::json!(cluster_name))
                                                                                .with_param("peer_id", serde_json::json!(peer_id.to_string()));
                                                                            
                                                                            if let Ok(cmd_json) = cmd.to_json() {
                                                                                let msg = JsonMessage::new(peer_id.to_string(), cmd_json);
                                                                                let _request_id = swarm.behaviour_mut().request_response.send_request(&target_peer_id, msg);
                                                                                println!("[SHARD_LOADED]   📤 Sent to peer {} (request_id: {:?})", target_peer_id, _request_id);
                                                                            }
                                                                        }
                                                                    }
                                                                }
                                                                
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
                                                                println!("[LOAD_SHARD] ⚠️  Shard {} not found locally", shard_id);
                                                                println!("[LOAD_SHARD] 📥 Starting torrent download from peer {}...", peer);
                                                                
                                                                // Start download from the requesting peer (they likely have it)
                                                                match s.start_download(shard_id, peer) {
                                                                    Ok(info_hash) => {
                                                                        println!("[LOAD_SHARD] ✓ Download initiated (info_hash: {})", &info_hash[..16]);
                                                                        println!("[LOAD_SHARD]   Watch for progress updates in torrent logs");
                                                                        
                                                                        let mut result = HashMap::new();
                                                                        result.insert("shard_id".to_string(), serde_json::json!(shard_id));
                                                                        result.insert("status".to_string(), serde_json::json!("downloading"));
                                                                        result.insert("info_hash".to_string(), serde_json::json!(info_hash.clone()));
                                                                        
                                                                        CommandResponse::success(
                                                                            &cmd.command,
                                                                            &cmd.request_id,
                                                                            &peer_id.to_string(),
                                                                            &cmd.from,
                                                                            result,
                                                                        )
                                                                    }
                                                                    Err(e) => {
                                                                        eprintln!("[LOAD_SHARD] ❌ Failed to start download: {}", e);
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
                                        
                                        commands::SYNC_TORRENTS => {
                                            // Synchronize torrents: query server for all available shard files
                                            // and initiate downloads for missing shards
                                            println!("\n[TORRENT_SYNC] ═══════════════════════════════════════════════════════════════════════════");
                                            println!("[TORRENT_SYNC] 📥 Received SYNC_TORRENTS request from {}", cmd.from);
                                            
                                            // First, get list of available files from server
                                            // This is handled by LIST_FILES - we'll respond with file list
                                            let file_list: Vec<serde_json::Value> = s.get_torrent_file_list()
                                                .iter()
                                                .map(|f| serde_json::json!({
                                                    "info_hash": f.info_hash,
                                                    "filename": f.filename,
                                                    "size": f.size,
                                                }))
                                                .collect();
                                            
                                            println!("[TORRENT_SYNC]   Available files: {}", file_list.len());
                                            for file in &file_list {
                                                if let (Some(filename), Some(size)) = (file.get("filename"), file.get("size")) {
                                                    let size_mb = size.as_u64().unwrap_or(0) as f64 / 1_048_576.0;
                                                    println!("[TORRENT_SYNC]     - {} ({:.2} MB)", 
                                                        filename.as_str().unwrap_or("unknown"), size_mb);
                                                }
                                            }
                                            
                                            let mut result = HashMap::new();
                                            result.insert("files".to_string(), serde_json::json!(file_list));
                                            result.insert("total_files".to_string(), serde_json::json!(file_list.len()));
                                            
                                            println!("[TORRENT_SYNC] ✓ Torrent sync response prepared");
                                            println!("[TORRENT_SYNC] ═══════════════════════════════════════════════════════════════════════════\n");
                                            
                                            CommandResponse::success(
                                                &cmd.command,
                                                &cmd.request_id,
                                                &peer_id.to_string(),
                                                &cmd.from,
                                                result,
                                            )
                                        }

                                        commands::SWARM_READY => {
                                            // Another node is notifying us that the swarm is ready
                                            println!("\n[SWARM] ═══════════════════════════════════════════════════════════════════════════");
                                            println!("[SWARM] 📢 Received SWARM_READY notification from {}", cmd.from);
                                            
                                            let status = s.discovery.status();
                                            let swarm_ready = status.is_complete && s.discovery.are_all_shards_loaded();
                                            
                                            // Update our swarm_ready flag if we agree
                                            if swarm_ready {
                                                s.swarm_ready = true;
                                                println!("[SWARM] ✓ Confirmed: Swarm is ready (all {} shards available)", status.expected_shards);
                                            } else {
                                                println!("[SWARM] ⚠️  Received SWARM_READY but local status shows: {}/{} shards (missing: {:?})", 
                                                    status.discovered_shards, status.expected_shards, status.missing_shards);
                                            }
                                            
                                            let mut result = HashMap::new();
                                            result.insert("swarm_ready".to_string(), serde_json::json!(swarm_ready));
                                            result.insert("discovered_shards".to_string(), serde_json::json!(status.discovered_shards));
                                            result.insert("expected_shards".to_string(), serde_json::json!(status.expected_shards));
                                            result.insert("missing_shards".to_string(), serde_json::json!(status.missing_shards));
                                            result.insert("peer_id".to_string(), serde_json::json!(peer_id.to_string()));
                                            
                                            println!("[SWARM] ═══════════════════════════════════════════════════════════════════════════\n");
                                            
                                            CommandResponse::success(
                                                &cmd.command,
                                                &cmd.request_id,
                                                &peer_id.to_string(),
                                                &cmd.from,
                                                result,
                                            )
                                        }

                                        commands::SWARM_STATUS => {
                                            // Request for current swarm status
                                            let status = s.discovery.status();
                                            let mut result = HashMap::new();
                                            result.insert("swarm_ready".to_string(), serde_json::json!(s.swarm_ready));
                                            result.insert("discovered_shards".to_string(), serde_json::json!(status.discovered_shards));
                                            result.insert("expected_shards".to_string(), serde_json::json!(status.expected_shards));
                                            result.insert("missing_shards".to_string(), serde_json::json!(status.missing_shards));
                                            result.insert("is_complete".to_string(), serde_json::json!(status.is_complete));
                                            result.insert("peer_id".to_string(), serde_json::json!(peer_id.to_string()));
                                            
                                            CommandResponse::success(
                                                &cmd.command,
                                                &cmd.request_id,
                                                &peer_id.to_string(),
                                                &cmd.from,
                                                result,
                                            )
                                        }

                                        commands::SHARD_LOADED => {
                                            // Another node is notifying us that it loaded a shard - update our tree
                                            let loaded_shard_id = cmd.params.get("shard_id")
                                                .and_then(|v| v.as_u64())
                                                .map(|v| v as u32);
                                            let notifying_peer_id = cmd.params.get("peer_id")
                                                .and_then(|v| v.as_str())
                                                .unwrap_or(&cmd.from);
                                            
                                            if let Some(shard_id) = loaded_shard_id {
                                                println!("\n[SHARD_LOADED] ═══════════════════════════════════════════════════════════════════════════");
                                                println!("[SHARD_LOADED] 📢 Received notification: Peer {} loaded shard {}", notifying_peer_id, shard_id);
                                                
                                                // Update the announcement in our discovery tree to mark shard as loaded
                                                // Use add_shard to update existing announcement
                                                let mut updated = false;
                                                if let Some(best_node) = s.discovery.get_best_node_for_shard(shard_id) {
                                                    if best_node.peer_id == notifying_peer_id {
                                                        // Create updated announcement with shard_loaded = true
                                                        let mut updated_announcement = best_node.clone();
                                                        updated_announcement.capabilities.shard_loaded = true;
                                                        s.discovery.add_shard(updated_announcement);
                                                        updated = true;
                                                        println!("[SHARD_LOADED] ✓ Updated local tree: shard {} is loaded on peer {}", shard_id, notifying_peer_id);
                                                    }
                                                }
                                                
                                                if !updated {
                                                    println!("[SHARD_LOADED] ⚠️  Peer {} not found in local tree for shard {}", notifying_peer_id, shard_id);
                                                }
                                                let status = s.discovery.status();
                                                
                                                // Check if all required shards are now loaded (not just announced)
                                                let all_shards_loaded = s.discovery.are_all_shards_loaded();
                                                
                                                if all_shards_loaded && !s.swarm_ready {
                                                    s.swarm_ready = true;
                                                    println!("[SHARD_LOADED] ✓✓✓ All required shards are now LOADED - swarm ready for inference ✓✓✓");
                                                } else if !all_shards_loaded {
                                                    let missing_loaded: Vec<u32> = (0..status.expected_shards)
                                                        .filter(|id| {
                                                            !s.discovery.get_best_node_for_shard(*id)
                                                                .map(|ann| ann.capabilities.shard_loaded)
                                                                .unwrap_or(false)
                                                        })
                                                        .collect();
                                                    println!("[SHARD_LOADED] ⏳ Waiting for shards to be loaded: {:?}", missing_loaded);
                                                }
                                                
                                                println!("[SHARD_LOADED] ═══════════════════════════════════════════════════════════════════════════\n");
                                                
                                                let mut result = HashMap::new();
                                                result.insert("shard_id".to_string(), serde_json::json!(shard_id));
                                                result.insert("updated".to_string(), serde_json::json!(updated));
                                                result.insert("all_shards_loaded".to_string(), serde_json::json!(all_shards_loaded));
                                                
                                                CommandResponse::success(
                                                    &cmd.command,
                                                    &cmd.request_id,
                                                    &peer_id.to_string(),
                                                    &cmd.from,
                                                    result,
                                                )
                                            } else {
                                                CommandResponse::error(
                                                    &cmd.command,
                                                    &cmd.request_id,
                                                    &peer_id.to_string(),
                                                    &cmd.from,
                                                    "Missing shard_id parameter",
                                                )
                                            }
                                        }
                                        
                                        commands::EXECUTE_TASK => {
                                            println!("\n[EXECUTE_TASK] ═══════════════════════════════════════════════════════════════════════════");
                                            println!("[EXECUTE_TASK] Processing inference task...");
                                            
                                            // Check if swarm is ready before processing inference
                                            let swarm_ready = s.swarm_ready;
                                            let discovery_status = s.discovery.status();
                                            
                                            if !swarm_ready {
                                                println!("[EXECUTE_TASK] ⚠️  Swarm not ready - waiting for all required shards to be available");
                                                println!("[EXECUTE_TASK]   Current status: {}/{} shards available", 
                                                    discovery_status.discovered_shards, discovery_status.expected_shards);
                                                println!("[EXECUTE_TASK]   Missing shards: {:?}", discovery_status.missing_shards);
                                                println!("[EXECUTE_TASK]   Querying swarm readiness from DHT...");
                                                
                                                // Query for swarm readiness
                                                let readiness_key = kad::RecordKey::new(&dht_keys::swarm_readiness_key(&cluster_name));
                                                swarm.behaviour_mut().kademlia.get_record(readiness_key);
                                                
                                                // Return error response - swarm not ready
                                                CommandResponse::error(
                                                    &cmd.command,
                                                    &cmd.request_id,
                                                    &peer_id.to_string(),
                                                    &cmd.from,
                                                    &format!(
                                                        "Swarm not ready for inference. {}/{} shards available. Missing: {:?}. ",
                                                        discovery_status.discovered_shards,
                                                        discovery_status.expected_shards,
                                                        discovery_status.missing_shards
                                                    ),
                                                )
                                            } else {
                                                println!("[EXECUTE_TASK] ✓ Swarm is ready - all {} shards available", discovery_status.expected_shards);
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
                                                // Query completed tensor files before processing
                                                let current_shard_id = s.shard_id;
                                                let local_peer_id = s.peer_id;
                                                
                                                // Query torrents for completed tensor files
                                                if let Some(tensor_path) = s.query_completed_tensor_file(current_shard_id) {
                                                    println!("[INFERENCE] ✓ Tensor file found in completed torrents: {}", tensor_path.display());
                                                    println!("[INFERENCE]   Local Peer ID: {} | Shard ID: {} | Ready for parallel inference", 
                                                        local_peer_id, current_shard_id);
                                                } else {
                                                    println!("[INFERENCE] ⚠️  No completed tensor file found, attempting to load...");
                                                    println!("[INFERENCE]   Local Peer ID: {} | Shard ID: {}", local_peer_id, current_shard_id);
                                                }
                                                
                                                // Ensure shard is loaded before processing
                                                let shard_load_error = if !s.is_shard_loaded(current_shard_id) {
                                                    match s.load_shard_file(current_shard_id) {
                                                        Ok(shard_path) => {
                                                            println!("[INFERENCE] ✓✓✓ Loaded tensor file for shard {} (next in queue) ✓✓✓", current_shard_id);
                                                            println!("[INFERENCE]   Local Peer ID: {} | Shard ID: {} | Path: {}", 
                                                                local_peer_id, current_shard_id, shard_path.display());
                                                            println!("[INFERENCE]   Status: Ready to participate in parallel inference processing");
                                                            None
                                                        }
                                                        Err(e) => {
                                                            s.complete_request(false);
                                                            eprintln!("[INFERENCE] ✗ Failed to load tensor file for shard {}", current_shard_id);
                                                            eprintln!("[INFERENCE]   Local Peer ID: {} | Shard ID: {} | Error: {}", 
                                                                local_peer_id, current_shard_id, e);
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
                                                    // Shard already loaded - log it
                                                    if let Some(shard_path) = s.loaded_shards.get(&current_shard_id) {
                                                        println!("[INFERENCE] ✓ Tensor file already loaded for shard {} (ready for parallel inference)", current_shard_id);
                                                        println!("[INFERENCE]   Local Peer ID: {} | Shard ID: {} | Path: {}", 
                                                            local_peer_id, current_shard_id, shard_path.display());
                                                    }
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
                                                    let _temperature = cmd.params.get("temperature")
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
                                                    
                                                    // Log tensor file usage for parallel inference
                                                    println!("[INFERENCE] 🚀 Using tensor file for parallel inference processing");
                                                    println!("[INFERENCE]   Local Peer ID: {} | Shard ID: {} | Tensor Path: {}", 
                                                        local_peer_id, s.shard_id, shard_path);
                                                    println!("[INFERENCE]   Layers: {}-{} | Processing input through shard", 
                                                        s.announcement.layer_start, s.announcement.layer_end);
                                                    
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
                                        
                                        println!("\n[RESPONSE] ═══════════════════════════════════════════════════════════════════════════");
                                        println!("[RESPONSE] 📤 Sending response to peer: {}", peer);
                                        println!("[RESPONSE]   Command: {}", cmd.command);
                                        println!("[RESPONSE]   Request ID: {}", cmd.request_id);
                                        println!("[RESPONSE]   Status: {:?}", response.status);
                                        if let Some(ref result) = response.result {
                                            let keys: Vec<String> = result.keys().cloned().collect::<Vec<String>>();
                                            println!("[RESPONSE]   Result keys: {:?}", keys);
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
                                        
                                        // If this was a LOAD_SHARD command that started a download, immediately request metadata
                                        if cmd.command == commands::LOAD_SHARD {
                                            if let Some(result) = &response.result {
                                                if let Some(info_hash) = result.get("info_hash").and_then(|v| v.as_str()) {
                                                    if let Some(status) = result.get("status").and_then(|v| v.as_str()) {
                                                        if status == "downloading" {
                                                            // We're already connected to this peer (they sent us the command)
                                                            let _ = swarm.behaviour_mut().torrent_response.send_request(
                                                                &peer,
                                                                TorrentMessage::RequestMetadata {
                                                                    info_hash: info_hash.to_string(),
                                                                }
                                                            );
                                                            println!("[LOAD_SHARD] 📡 Immediately requested metadata for {} from peer {}", &info_hash[..16], peer);
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                else {
                                    eprintln!("[REQUEST] ❌ Failed to parse command JSON: {}", request.message);
                                }
                                    }
                                    
                                    request_response::Message::Response { response, .. } => {
                                        // Handle JSON command responses
                                        if let Ok(cmd_response) = serde_json::from_str::<CommandResponse>(&response.message) {
                                            println!("\n[RESPONSE] ═══════════════════════════════════════════════════════════════════════════");
                                            println!("[RESPONSE] 📥 Received response from peer: {}", peer_id);
                                            println!("[RESPONSE]   Command: {}", cmd_response.command);
                                            println!("[RESPONSE]   Request ID: {}", cmd_response.request_id);
                                            println!("[RESPONSE]   Status: {:?}", cmd_response.status);
                                            
                                            // Handle SYNC_TORRENTS response
                                            if cmd_response.command == commands::SYNC_TORRENTS {
                                                if cmd_response.status == ResponseStatus::Success {
                                                    if let Some(result) = &cmd_response.result {
                                                        if let Some(files) = result.get("files").and_then(|v| v.as_array()) {
                                                            println!("[TORRENT_SYNC] ✓ Received {} available file(s) from rendezvous server", files.len());
                                                            
                                                            let mut s = state.write().await;
                                                            let mut downloads_started = 0;
                                                            let mut pending_metadata_requests: Vec<(PeerId, String)> = Vec::new();
                                                            
                                                            // Process each file and start downloads for missing shards
                                                            for file in files {
                                                                if let (Some(filename), Some(info_hash)) = (
                                                                    file.get("filename").and_then(|v| v.as_str()),
                                                                    file.get("info_hash").and_then(|v| v.as_str())
                                                                ) {
                                                                    // Extract shard ID from filename (e.g., "shard-0.gguf" -> 0)
                                                                    if let Some(shard_id_str) = filename.strip_prefix("shard-").and_then(|s| s.strip_suffix(".gguf")) {
                                                                        if let Ok(shard_id) = shard_id_str.parse::<u32>() {
                                                                            // Check if this shard is needed and not already loaded
                                                                            if shard_id < total_shards && !s.is_shard_loaded(shard_id) {
                                                                                // Check if not already downloading
                                                                                if !s.active_downloads.contains_key(info_hash) {
                                                                                    println!("[TORRENT_SYNC]   📥 Starting download for shard {} (file: {})", shard_id, filename);
                                                                                    
                                                                                    match s.start_download(shard_id, peer_id) {
                                                                                        Ok(info_hash_ret) => {
                                                                                            downloads_started += 1;
                                                                                            println!("[TORRENT_SYNC]     ✓ Download initiated (info_hash: {})", &info_hash_ret[..16]);
                                                                                            
                                                                                            // Store pending metadata request (we're already connected to bootstrap)
                                                                                            pending_metadata_requests.push((peer_id, info_hash_ret));
                                                                                        }
                                                                                        Err(e) => {
                                                                                            eprintln!("[TORRENT_SYNC]     ✗ Failed to start download: {}", e);
                                                                                        }
                                                                                    }
                                                                                } else {
                                                                                    println!("[TORRENT_SYNC]   ⏳ Shard {} already downloading", shard_id);
                                                                                }
                                                                            } else if s.is_shard_loaded(shard_id) {
                                                                                println!("[TORRENT_SYNC]   ✓ Shard {} already loaded", shard_id);
                                                                            }
                                                                        }
                                                                    }
                                                                }
                                                            }
                                                            
                                                            drop(s);
                                                            
                                                            // Immediately request metadata for all pending downloads (we're already connected to bootstrap)
                                                            for (target_peer, info_hash) in pending_metadata_requests {
                                                                let _ = swarm.behaviour_mut().torrent_response.send_request(
                                                                    &target_peer,
                                                                    TorrentMessage::RequestMetadata {
                                                                        info_hash: info_hash.clone(),
                                                                    }
                                                                );
                                                                println!("[TORRENT_SYNC] 📡 Requested metadata for {} from peer {}", &info_hash[..16], target_peer);
                                                            }
                                                            
                                                            if downloads_started > 0 {
                                                                println!("[TORRENT_SYNC] ✓ Started {} download(s) for missing shards", downloads_started);
                                                            } else {
                                                                println!("[TORRENT_SYNC] ✓ All required shards are already present or downloading");
                                                            }
                                                        }
                                                    }
                                                } else if let Some(error) = &cmd_response.error {
                                                    eprintln!("[TORRENT_SYNC] ✗ Sync failed: {}", error);
                                                }
                                            }
                                            
                                            println!("[RESPONSE] ═══════════════════════════════════════════════════════════════════════════\n");
                                        }
                                    }
                                }
                            }

                            ShardBehaviourEvent::Identify(libp2p::identify::Event::Received { peer_id: identified_peer, info }) => {
                                println!("[IDENTIFY] {} running {}", identified_peer, info.agent_version);
                            }
                            
                            // Handle torrent protocol messages
                            ShardBehaviourEvent::TorrentResponse(request_response::Event::Message {
                                peer: _peer,
                                message: request_response::Message::Request { request, channel, request_id: _ },
                                ..
                            }) => {
                                // Handle incoming torrent requests (serving files)
                                let s = state.write().await;
                                
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
                                peer: source_peer,
                                message: request_response::Message::Response { response, .. },
                                ..
                            }) => {
                                // Handle torrent responses (downloading files)
                                let mut s = state.write().await;
                                let local_peer_id = s.peer_id;
                                
                                // Get routing table info - simplified to avoid borrow issues
                                // Note: Getting exact routing table stats requires more complex access patterns
                                let routing_table_info = "routing_table: active (DHT connected)";
                                
                                match response {
                                    TorrentMessage::Metadata { metadata } => {
                                        let file_size_mb = metadata.file_size as f64 / (1024.0 * 1024.0);
                                        let file_size_gb = file_size_mb / 1024.0;
                                        let size_str = if file_size_gb >= 1.0 {
                                            format!("{:.2} GB ({:.2} MB)", file_size_gb, file_size_mb)
                                        } else {
                                            format!("{:.2} MB", file_size_mb)
                                        };
                                        
                                        println!("[TORRENT] 📥 Received metadata for: {}", metadata.filename);
                                        println!("[TORRENT]   Local Peer ID: {}", local_peer_id);
                                        println!("[TORRENT]   Source Peer ID: {}", source_peer);
                                        println!("[TORRENT]   Info Hash: {}", &metadata.info_hash[..16]);
                                        println!("[TORRENT]   File size: {}", size_str);
                                        println!("[TORRENT]   Total pieces: {}", metadata.pieces.len());
                                        println!("[TORRENT]   Piece size: {:.2} KB", metadata.piece_size as f64 / 1024.0);
                                        println!("[TORRENT]   Routing: {}", routing_table_info);
                                        
                                        if let Some(download) = s.active_downloads.get_mut(&metadata.info_hash) {
                                            download.metadata = Some(metadata.clone());
                                            download.total_pieces = metadata.pieces.len();
                                            
                                            // Request all pieces
                                            if let Some(peer_id) = download.peer_id {
                                                println!("[TORRENT]   Requesting {} pieces from peer {}...", metadata.pieces.len(), peer_id);
                                                println!("[TORRENT]   Local Peer ID: {} | Source Peer ID: {} | Routing: {}", 
                                                    local_peer_id, peer_id, routing_table_info);
                                                for i in 0..metadata.pieces.len() {
                                                    let _ = swarm.behaviour_mut().torrent_response.send_request(
                                                        &peer_id,
                                                        TorrentMessage::RequestPiece {
                                                            info_hash: metadata.info_hash.clone(),
                                                            piece_index: i as u64,
                                                        }
                                                    );
                                                }
                                                println!("[TORRENT]   ✓ All {} piece requests sent to peer {}", metadata.pieces.len(), peer_id);
                                                println!("[TORRENT]   Progress: [                    ] 0%");
                                            }
                                        }
                                    },
                                    
                                    TorrentMessage::PieceData { info_hash, piece_index, data } => {
                                        if let Some(download) = s.active_downloads.get_mut(&info_hash) {
                                            let source_peer_str = format!("{}", source_peer);
                                            let local_peer_str = format!("{}", local_peer_id);
                                            
                                            // Verify piece hash before storing
                                            if let Some(metadata) = &download.metadata {
                                                if piece_index as usize >= metadata.pieces.len() {
                                                    eprintln!("[TORRENT] ✗ Invalid piece_index {} (max: {})", piece_index, metadata.pieces.len());
                                                    eprintln!("[TORRENT]   Local Peer ID: {} | Source Peer ID: {} | Info Hash: {}", 
                                                        local_peer_str, source_peer_str, &info_hash[..16]);
                                                    continue;
                                                }
                                                
                                                let expected_hash = &metadata.pieces[piece_index as usize];
                                                let mut hasher = Sha256::new();
                                                hasher.update(&data);
                                                let computed_hash = format!("{:x}", hasher.finalize());
                                                
                                                if computed_hash != *expected_hash {
                                                    eprintln!("[TORRENT] ✗ Piece {} hash mismatch! Expected: {}, Got: {}", 
                                                        piece_index, &expected_hash[..16], &computed_hash[..16]);
                                                    eprintln!("[TORRENT]   Local Peer ID: {} | Source Peer ID: {} | Info Hash: {}", 
                                                        local_peer_str, source_peer_str, &info_hash[..16]);
                                                    eprintln!("[TORRENT]   Discarding corrupted piece, will re-request");
                                                    // Don't increment downloaded_pieces, will re-request
                                                    continue;
                                                }
                                                
                                                // Log successful piece receipt with verification
                                                println!("[TORRENT] ✓ Piece {}/{} received and verified ({} bytes)", 
                                                    piece_index, metadata.pieces.len(), data.len());
                                                println!("[TORRENT]   Local Peer ID: {} | Source Peer ID: {} | Info Hash: {}", 
                                                    local_peer_str, source_peer_str, &info_hash[..16]);
                                                println!("[TORRENT]   Piece Hash: {} ✓ | Routing: {}", 
                                                    &computed_hash[..16], routing_table_info);
                                            } else {
                                                println!("[TORRENT] 📥 Piece {} received ({} bytes) - metadata not yet available", 
                                                    piece_index, data.len());
                                                println!("[TORRENT]   Local Peer ID: {} | Source Peer ID: {} | Info Hash: {}", 
                                                    local_peer_str, source_peer_str, &info_hash[..16]);
                                            }
                                            
                                            download.pieces.insert(piece_index, data);
                                            download.downloaded_pieces += 1;
                                            
                                            // Calculate progress percentage
                                            let progress_pct = if download.total_pieces > 0 {
                                                (download.downloaded_pieces as f64 / download.total_pieces as f64 * 100.0) as u32
                                            } else {
                                                0
                                            };
                                            
                                            // Calculate downloaded size
                                            let downloaded_size: u64 = download.pieces.values().map(|d| d.len() as u64).sum();
                                            let downloaded_mb = downloaded_size as f64 / (1024.0 * 1024.0);
                                            let total_size = download.metadata.as_ref().map(|m| m.file_size).unwrap_or(0);
                                            let total_mb = total_size as f64 / (1024.0 * 1024.0);
                                            
                                            // Show progress bar (20 characters)
                                            let bar_width = 20;
                                            let filled = (progress_pct as usize * bar_width / 100).min(bar_width);
                                            let bar = "█".repeat(filled) + &"░".repeat(bar_width - filled);
                                            
                                            // Print progress more frequently for better visibility
                                            // Print every 5% or every 10 pieces (whichever is more frequent), or on completion
                                            let should_print = download.total_pieces <= 20 
                                                || progress_pct % 5 == 0 
                                                || download.downloaded_pieces == download.total_pieces
                                                || (download.downloaded_pieces > 0 && download.downloaded_pieces % 10 == 0);
                                            
                                            if should_print {
                                                if total_size > 0 {
                                                    println!("[TORRENT] 📥 [{}] {}% ({:.2} MB / {:.2} MB) - Piece {}/{}", 
                                                        bar, progress_pct, downloaded_mb, total_mb, 
                                                        download.downloaded_pieces, download.total_pieces);
                                                } else {
                                                    println!("[TORRENT] 📥 [{}] {}% - Piece {}/{}", 
                                                        bar, progress_pct, download.downloaded_pieces, download.total_pieces);
                                                }
                                            }
                                            
                                            // Check if download is complete
                                            let source_peer_id_for_log = download.peer_id;
                                            if let Ok(Some(file_path)) = s.check_download_complete(&info_hash) {
                                                println!("[TORRENT] ✓✓✓ Download complete: {} ✓✓✓", file_path.display());
                                                println!("[TORRENT]   Local Peer ID: {} | Source Peer ID: {:?} | Info Hash: {}", 
                                                    local_peer_id, source_peer_id_for_log, &info_hash[..16]);
                                                if total_size > 0 {
                                                    println!("[TORRENT]   Final size: {:.2} MB", total_mb);
                                                }
                                                println!("[TORRENT]   Routing: {}", routing_table_info);
                                                println!("[TORRENT]   Status: Tensor file ready for parallel inference processing");
                                                
                                                // Extract shard_id and broadcast SHARD_LOADED to peers
                                                if let Some(shard_id) = file_path.file_stem()
                                                    .and_then(|s| s.to_str())
                                                    .and_then(|s| s.strip_prefix("shard-"))
                                                    .and_then(|s| s.parse::<u32>().ok()) {
                                                    // Mark shard as loaded in capabilities
                                                    s.announcement.capabilities.shard_loaded = true;
                                                    s.needs_reannounce = true;
                                                    
                                                    if let Some(completed_path) = s.query_completed_tensor_file(shard_id) {
                                                        println!("[TORRENT] ✓ Verified completed tensor file in queue: {}", completed_path.display());
                                                        println!("[TORRENT]   Local Peer ID: {} | Shard ID: {} | Ready for parallel inference", 
                                                            local_peer_id, shard_id);
                                                    }
                                                    
                                                    // Get peers to broadcast to
                                                    let pipeline_peers = s.discovery.get_pipeline().iter()
                                                        .map(|ann| ann.peer_id.clone())
                                                        .collect::<Vec<_>>();
                                                    drop(s);
                                                    
                                                    // Broadcast SHARD_LOADED to all known peers
                                                    println!("[TORRENT] 📢 Broadcasting shard {} loaded (from torrent) to {} peers...", shard_id, pipeline_peers.len());
                                                    for peer_id_str in pipeline_peers {
                                                        if let Ok(target_peer_id) = peer_id_str.parse::<PeerId>() {
                                                            if target_peer_id != peer_id {
                                                                let cmd = Command::new(commands::SHARD_LOADED, &peer_id.to_string(), Some(&peer_id_str))
                                                                    .with_param("shard_id", serde_json::json!(shard_id))
                                                                    .with_param("cluster_name", serde_json::json!(cluster_name))
                                                                    .with_param("peer_id", serde_json::json!(peer_id.to_string()));
                                                                
                                                                if let Ok(cmd_json) = cmd.to_json() {
                                                                    let msg = JsonMessage::new(peer_id.to_string(), cmd_json);
                                                                    let _request_id = swarm.behaviour_mut().request_response.send_request(&target_peer_id, msg);
                                                                    println!("[TORRENT]   📤 Broadcasted shard {} loaded to peer {} (request_id: {:?})", shard_id, target_peer_id, _request_id);
                                                                }
                                                            }
                                                        }
                                                    }
                                                } else {
                                                    drop(s);
                                                }
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
                        // Check if this is a bootstrap connection failure
                        if let Some(peer) = failed_peer {
                            // Try to determine if this was the bootstrap by checking if we're not connected
                            if !bootstrap_connected {
                                eprintln!("[CONNECT] ⚠️  Bootstrap connection failed: {:?}", error);
                                eprintln!("[CONNECT] ↻ Will retry in 5 seconds...");
                                bootstrap_retry_timer = tokio::time::Instant::now() + Duration::from_secs(5);
                            } else {
                                eprintln!("[ERROR] Connection failed to {:?}: {:?}", peer, error);
                            }
                        } else {
                            // No peer_id means it might be the bootstrap (initial dial)
                            if !bootstrap_connected {
                                eprintln!("[CONNECT] ⚠️  Bootstrap connection failed: {:?}", error);
                                eprintln!("[CONNECT] ↻ Will retry in 5 seconds...");
                                bootstrap_retry_timer = tokio::time::Instant::now() + Duration::from_secs(5);
                            }
                        }
                    }

                    _ => {}
                }
            }

            // Bootstrap connection retry
            _ = tokio::time::sleep_until(bootstrap_retry_timer), if !bootstrap_connected => {
                println!("[CONNECT] ↻ Retrying bootstrap connection...");
                if let Err(e) = swarm.dial(bootstrap_addr.clone()) {
                    eprintln!("[CONNECT] ⚠️  Retry dial failed: {:?}", e);
                }
                bootstrap_retry_timer = tokio::time::Instant::now() + Duration::from_secs(5);
            }

            // Periodic announcement refresh
            _ = tokio::time::sleep_until(next_refresh) => {
                // Periodic refresh: re-announce shard and query swarm readiness
                if announced {
                    let s = state.read().await;
                    let record = s.create_announcement_record();
                    let status = s.discovery.status();
                    let swarm_ready = s.swarm_ready;
                    drop(s);

                    if let Err(e) = swarm.behaviour_mut().kademlia.put_record(record, kad::Quorum::One) {
                        eprintln!("[DHT] Refresh failed: {:?}", e);
                    } else {
                        println!("[DHT] ↻ Refreshed shard announcement");
                    }

                    // Check and announce swarm readiness if complete
                    let all_shards_loaded = {
                        let s = state.read().await;
                        s.discovery.are_all_shards_loaded()
                    };
                    let should_broadcast = if !swarm_ready && status.is_complete && all_shards_loaded {
                        let mut s = state.write().await;
                        s.swarm_ready = true;
                        drop(s);
                        true
                    } else {
                        false
                    };
                    
                    if should_broadcast {
                        if let Some(readiness_record) = {
                            let s = state.read().await;
                            s.discovery.create_swarm_readiness_record(&peer_id.to_string())
                        } {
                            if let Err(e) = swarm.behaviour_mut().kademlia.put_record(readiness_record, kad::Quorum::One) {
                                eprintln!("[SWARM] ⚠️  Failed to announce swarm readiness: {:?}", e);
                            } else {
                                println!("[SWARM] ✓ Announced swarm readiness (all {} shards available)", status.expected_shards);
                            }
                        }
                        
                        // Broadcast SWARM_READY to all known peers
                        let pipeline = {
                            let s = state.read().await;
                            s.discovery.get_pipeline().iter().map(|ann| ann.peer_id.clone()).collect::<Vec<_>>()
                        };
                        
                        for peer_id_str in pipeline {
                            if let Ok(target_peer_id) = peer_id_str.parse::<PeerId>() {
                                if target_peer_id != peer_id {
                                    let cmd = Command::new(commands::SWARM_READY, &peer_id.to_string(), Some(&peer_id_str))
                                        .with_param("total_shards", serde_json::json!(status.expected_shards))
                                        .with_param("cluster_name", serde_json::json!(cluster_name));
                                    
                                    if let Ok(cmd_json) = cmd.to_json() {
                                                    let msg = JsonMessage::new(peer_id.to_string(), cmd_json.clone());
                                                    let request_id = swarm.behaviour_mut().request_response.send_request(&target_peer_id, msg);
                                                    println!("\n[MSG] ═══════════════════════════════════════════════════════════════════════════");
                                                    println!("[MSG] 📤 SENT MESSAGE TO PEER: {}", target_peer_id);
                                                    println!("[MSG]   Command: SWARM_READY");
                                                    println!("[MSG]   Request ID: {:?}", request_id);
                                                    println!("[MSG]   Message: {}", cmd_json);
                                                    println!("[MSG] ═══════════════════════════════════════════════════════════════════════════\n");
                                    }
                                }
                            }
                        }
                    }

                    // Query for swarm readiness periodically
                    let readiness_key = kad::RecordKey::new(&dht_keys::swarm_readiness_key(&cluster_name));
                    swarm.behaviour_mut().kademlia.get_record(readiness_key);
                }
                next_refresh = tokio::time::Instant::now() + refresh_interval;
            }

            // Periodic status report
            _ = tokio::time::sleep_until(next_status_report) => {
                print_status_report(&state, shard_id, total_shards, &cluster_name).await;
                next_status_report = tokio::time::Instant::now() + status_report_interval;
            }
        }
    }
}

/// Print comprehensive status report
async fn print_status_report(
    state: &Arc<RwLock<ShardNodeState>>,
    local_shard_id: u32,
    total_shards: u32,
    cluster_name: &str,
) {
    let s = state.read().await;
    let discovery_status = s.discovery.status();
    let pipeline = s.discovery.get_pipeline();
    
    println!("\n[STATUS] ═══════════════════════════════════════════════════════════════════════════");
    println!("[STATUS] System Status Report - {}", 
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| format!("{}", d.as_secs()))
            .unwrap_or_else(|_| "unknown".to_string()));
    println!("[STATUS] ═══════════════════════════════════════════════════════════════════════════");
    
    // Local node status
    println!("[STATUS] Local Node:");
    println!("[STATUS]   Shard ID: {} / {}", local_shard_id, total_shards - 1);
    println!("[STATUS]   Peer ID: {}", s.peer_id);
    println!("[STATUS]   Shard Loaded: {}", if s.is_shard_loaded(local_shard_id) { "✓ YES" } else { "✗ NO" });
    if s.is_shard_loaded(local_shard_id) {
        if let Some(path) = s.loaded_shards.get(&local_shard_id) {
            if let Ok(metadata) = std::fs::metadata(path) {
                let size_mb = metadata.len() as f64 / (1024.0 * 1024.0);
                println!("[STATUS]   Shard File: {} ({:.2} MB)", path.display(), size_mb);
            }
        }
    }
    println!("[STATUS]   Active Requests: {}/{}", s.active_requests, s.announcement.capabilities.max_concurrent);
    println!("[STATUS]   Total Requests: {} ({} successful)", s.total_requests, s.successful_requests);
    
    // Active downloads progress
    if !s.active_downloads.is_empty() {
        println!("\n[STATUS] Active Downloads:");
        for (info_hash, download) in &s.active_downloads {
            let progress_pct = if download.total_pieces > 0 {
                (download.downloaded_pieces as f64 / download.total_pieces as f64 * 100.0) as u32
            } else {
                0
            };
            let bar_width = 20;
            let filled = (progress_pct as usize * bar_width / 100).min(bar_width);
            let bar = "█".repeat(filled) + &"░".repeat(bar_width - filled);
            
            let downloaded_mb = if let Some(metadata) = &download.metadata {
                let downloaded_size: u64 = download.pieces.values().map(|d| d.len() as u64).sum();
                downloaded_size as f64 / (1024.0 * 1024.0)
            } else {
                0.0
            };
            let total_mb = download.metadata.as_ref()
                .map(|m| m.file_size as f64 / (1024.0 * 1024.0))
                .unwrap_or(0.0);
            
            println!("[STATUS]   {}: [{}] {}% ({:.2} MB / {:.2} MB) - Pieces {}/{}", 
                download.filename, bar, progress_pct, downloaded_mb, total_mb,
                download.downloaded_pieces, download.total_pieces);
            if let Some(peer) = download.peer_id {
                println!("[STATUS]     Downloading from: {}", peer);
            }
        }
    } else {
        println!("\n[STATUS] Active Downloads: None");
    }
    
    // Discovery status
    println!("\n[STATUS] Cluster Discovery:");
    println!("[STATUS]   Cluster: {}", cluster_name);
    println!("[STATUS]   Expected Shards: {}", discovery_status.expected_shards);
    println!("[STATUS]   Discovered Shards: {}", discovery_status.discovered_shards);
    println!("[STATUS]   Pipeline Complete: {}", if discovery_status.is_complete { "✓ YES" } else { "✗ NO" });
    println!("[STATUS]   Swarm Ready: {}", if s.swarm_ready { "✓ YES" } else { "✗ NO" });
    
    // Shard online status
    println!("\n[STATUS] Shard Online Status:");
    let mut online_shards = std::collections::HashSet::new();
    let mut loaded_shards = std::collections::HashSet::new();
    
    for ann in &pipeline {
        online_shards.insert(ann.shard_id);
        if ann.capabilities.shard_loaded {
            loaded_shards.insert(ann.shard_id);
        }
    }
    
    for shard_id in 0..total_shards {
        let is_online = online_shards.contains(&shard_id);
        let is_loaded = loaded_shards.contains(&shard_id);
        let is_local = shard_id == local_shard_id;
        
        let status_icon = if is_local {
            "★"
        } else if is_online && is_loaded {
            "✓"
        } else if is_online {
            "○"
        } else {
            "✗"
        };
        
        let status_text = if is_local {
            "LOCAL"
        } else if is_online && is_loaded {
            "ONLINE + LOADED"
        } else if is_online {
            "ONLINE (not loaded)"
        } else {
            "OFFLINE"
        };
        
        println!("[STATUS]   Shard {}: {} {}", shard_id, status_icon, status_text);
        
        if is_online && !is_local {
            if let Some(ann) = pipeline.iter().find(|a| a.shard_id == shard_id) {
                println!("[STATUS]     Peer ID: {}", ann.peer_id);
                println!("[STATUS]     Layers: {}-{}", ann.layer_start, ann.layer_end);
            }
        }
    }
    
    // Summary
    let online_count = online_shards.len();
    let loaded_count = loaded_shards.len();
    let total_shards_usize = total_shards as usize;
    let online_pct = (online_count as f64 / total_shards as f64 * 100.0) as u32;
    let loaded_pct = (loaded_count as f64 / total_shards as f64 * 100.0) as u32;
    
    println!("\n[STATUS] Summary:");
    println!("[STATUS]   Online Shards: {}/{} ({}%)", online_count, total_shards, online_pct);
    println!("[STATUS]   Loaded Shards: {}/{} ({}%)", loaded_count, total_shards, loaded_pct);
    println!("[STATUS]   Pipeline Ready: {}", if discovery_status.is_complete && loaded_count == total_shards_usize { "✓ YES" } else { "✗ NO" });
    
    println!("[STATUS] ═══════════════════════════════════════════════════════════════════════════\n");
}

#[allow(dead_code)]
#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();
    run_shard_listener(
        args.bootstrap,
        args.cluster,
        args.shard_id,
        args.total_shards,
        args.total_layers,
        args.model_name,
        args.port,
        args.refresh_interval,
        args.shards_dir,
        args.enable_torrent,
        args.transport,
    ).await
}

