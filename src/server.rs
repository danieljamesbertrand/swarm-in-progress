//! Simple Kademlia Bootstrap Node - Acts as a bootstrap node for the DHT network
//! Usage: cargo run --bin server [--listen-addr ADDR] [--port PORT]
//! 
//! Also available via unified node binary:
//!   cargo run --bin node -- bootstrap --listen-addr ADDR --port PORT

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
use std::error::Error;
use std::time::Duration;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;
use serde::{Serialize, Deserialize};
use sha2::{Sha256, Digest};
use punch_simple::quic_transport::{create_transport, get_dual_listen_addresses, get_listen_address, TransportType};
use punch_simple::quic_diagnostics::{QuicDiagnosticsManager, QuicHandshakeStage};
use axum::{
    extract::{Path as AxumPath, Query, State},
    http::StatusCode,
    response::Json,
    routing::get,
    Router,
};

/// Extract IP address from Multiaddr for fail2ban monitoring
fn extract_ip_from_multiaddr(addr: &Multiaddr) -> Option<String> {
    // Parse multiaddr to extract IP address
    for component in addr.iter() {
        match component {
            libp2p::multiaddr::Protocol::Ip4(ip) => {
                return Some(ip.to_string());
            }
            libp2p::multiaddr::Protocol::Ip6(ip) => {
                return Some(ip.to_string());
            }
            _ => {}
        }
    }
    None
}

#[derive(Parser, Debug)]
#[command(name = "server")]
#[command(about = "Simple Kademlia Bootstrap Node - Acts as a bootstrap node for the DHT network")]
struct Args {
    /// Listen address (default: 0.0.0.0)
    #[arg(long, default_value = "0.0.0.0")]
    listen_addr: String,

    /// Listen port (default: 51820)
    #[arg(long, default_value = "51820")]
    port: u16,

    /// Transport: quic|tcp|dual (default: dual)
    #[arg(long, default_value = "dual")]
    transport: TransportType,

    /// Directory to seed shard files from (optional)
    #[arg(long, default_value = "")]
    seed_dir: String,
}

// Torrent message types (same as torrent_server)
#[derive(Clone, Serialize, Deserialize, Debug)]
enum TorrentMessage {
    RequestPiece { info_hash: String, piece_index: u64 },
    PieceData { info_hash: String, piece_index: u64, data: Vec<u8> },
    RequestMetadata { info_hash: String },
    Metadata { metadata: TorrentMetadata },
    ListFiles,
    FileList { files: Vec<TorrentFileInfo> },
}

#[derive(Clone, Serialize, Deserialize, Debug)]
struct TorrentMetadata {
    info_hash: String,
    filename: String,
    file_size: u64,
    piece_size: u64,
    pieces: Vec<String>,
    announce: Vec<String>,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
struct TorrentFileInfo {
    info_hash: String,
    filename: String,
    size: u64,
}

#[derive(Clone)]
struct TorrentCodec;

#[async_trait::async_trait]
impl request_response::Codec for TorrentCodec {
    type Request = TorrentMessage;
    type Response = TorrentMessage;
    type Protocol = StreamProtocol;

    async fn read_request<T>(&mut self, _: &Self::Protocol, io: &mut T) -> std::io::Result<Self::Request>
    where T: libp2p::futures::AsyncRead + Unpin + Send,
    {
        use libp2p::futures::AsyncReadExt;
        let mut buffer = Vec::new();
        io.read_to_end(&mut buffer).await?;
        serde_json::from_slice(&buffer).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
    }

    async fn read_response<T>(&mut self, _: &Self::Protocol, io: &mut T) -> std::io::Result<Self::Response>
    where T: libp2p::futures::AsyncRead + Unpin + Send,
    {
        use libp2p::futures::AsyncReadExt;
        let mut buffer = Vec::new();
        io.read_to_end(&mut buffer).await?;
        serde_json::from_slice(&buffer).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
    }

    async fn write_request<T>(&mut self, _: &Self::Protocol, io: &mut T, req: Self::Request) -> std::io::Result<()>
    where T: libp2p::futures::AsyncWrite + Unpin + Send,
    {
        use libp2p::futures::AsyncWriteExt;
        let json = serde_json::to_vec(&req).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        io.write_all(&json).await?;
        Ok(())
    }

    async fn write_response<T>(&mut self, _: &Self::Protocol, io: &mut T, res: Self::Response) -> std::io::Result<()>
    where T: libp2p::futures::AsyncWrite + Unpin + Send,
    {
        use libp2p::futures::AsyncWriteExt;
        let json = serde_json::to_vec(&res).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        io.write_all(&json).await?;
        Ok(())
    }
}

struct TorrentFile {
    path: PathBuf,
    metadata: TorrentMetadata,
    pieces: Vec<Vec<u8>>,
}

struct TorrentServer {
    share_dir: PathBuf,
    files: HashMap<String, TorrentFile>,
}

impl TorrentServer {
    fn new(share_dir: &str) -> Result<Self, Box<dyn Error>> {
        let share_path = if share_dir.is_empty() {
            PathBuf::from("models_cache/shards")
        } else {
            PathBuf::from(share_dir)
        };
        std::fs::create_dir_all(&share_path)?;
        
        let mut server = Self {
            share_dir: share_path,
            files: HashMap::new(),
        };
        
        server.scan_files()?;
        Ok(server)
    }

    fn scan_files(&mut self) -> Result<(), Box<dyn Error>> {
        self.files.clear();
        
        if !self.share_dir.exists() {
            println!("[TORRENT] Share directory does not exist: {}", self.share_dir.display());
            return Ok(());
        }

        println!("[TORRENT] Scanning directory: {}", self.share_dir.display());
        let mut scanned = 0;
        let mut loaded = 0;

        for entry in std::fs::read_dir(&self.share_dir)? {
            let entry = entry?;
            let path = entry.path();
            
            if path.is_file() {
                scanned += 1;
                let ext = path.extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("");
                
                // Accept both .gguf and .safetensors files
                if ext == "gguf" || ext == "safetensors" {
                    match Self::load_file(&path) {
                        Ok(Some(file)) => {
                            let info_hash = file.metadata.info_hash.clone();
                            let filename = file.metadata.filename.clone();
                            let size_mb = file.metadata.file_size as f64 / 1_048_576.0;
                            let hash_preview = info_hash[..16].to_string();
                            self.files.insert(info_hash, file);
                            println!("[TORRENT]   ✓ Loaded: {} ({:.2} MB, hash: {})", 
                                filename, size_mb, hash_preview);
                            loaded += 1;
                        }
                        Ok(None) => {
                            // File skipped (empty or invalid)
                        }
                        Err(e) => {
                            eprintln!("[TORRENT]   ✗ Failed to load {}: {}", path.display(), e);
                        }
                    }
                }
            }
        }

        println!("[TORRENT] Scanned {} file(s), loaded {} shard file(s) for sharing", scanned, loaded);
        if loaded > 0 {
            println!("[TORRENT] ✓ Torrent seeding ready - {} file(s) available", loaded);
        }
        Ok(())
    }

    fn load_file(path: &Path) -> Result<Option<TorrentFile>, Box<dyn Error>> {
        let file_data = std::fs::read(path)?;
        let file_size = file_data.len() as u64;
        let piece_size_bytes = 64 * 1024; // 64 KB pieces
        let piece_size = piece_size_bytes as u64;
        
        // Calculate info hash (SHA256 of filename + size)
        let mut hasher = Sha256::new();
        hasher.update(path.file_name().unwrap().to_string_lossy().as_bytes());
        hasher.update(&file_size.to_le_bytes());
        let info_hash = format!("{:x}", hasher.finalize());

        // Split into pieces
        let mut pieces = Vec::new();
        let mut piece_hashes = Vec::new();
        
        for chunk in file_data.chunks(piece_size_bytes) {
            let piece = chunk.to_vec();
            let mut piece_hasher = Sha256::new();
            piece_hasher.update(&piece);
            piece_hashes.push(format!("{:x}", piece_hasher.finalize()));
            pieces.push(piece);
        }

        let metadata = TorrentMetadata {
            info_hash: info_hash.clone(),
            filename: path.file_name().unwrap().to_string_lossy().to_string(),
            file_size,
            piece_size,
            pieces: piece_hashes,
            announce: vec![],
        };

        Ok(Some(TorrentFile {
            path: path.to_path_buf(),
            metadata,
            pieces,
        }))
    }

    fn get_file_list(&self) -> Vec<TorrentFileInfo> {
        self.files.values()
            .map(|f| TorrentFileInfo {
                info_hash: f.metadata.info_hash.clone(),
                filename: f.metadata.filename.clone(),
                size: f.metadata.file_size,
            })
            .collect()
    }

    fn get_metadata(&self, info_hash: &str) -> Option<&TorrentMetadata> {
        self.files.get(info_hash).map(|f| &f.metadata)
    }

    fn get_piece(&self, info_hash: &str, piece_index: u64) -> Option<Vec<u8>> {
        self.files.get(info_hash)
            .and_then(|f| f.pieces.get(piece_index as usize))
            .cloned()
    }
}

#[derive(NetworkBehaviour)]
struct Behaviour {
    kademlia: kad::Behaviour<kad::store::MemoryStore>,
    identify: libp2p::identify::Behaviour,
    ping: ping::Behaviour,
    relay: relay::Behaviour,
    torrent_response: request_response::Behaviour<TorrentCodec>,
}

/// Run bootstrap server with a specified transport.
pub async fn run_bootstrap_with_transport(
    listen_addr: String,
    port: u16,
    transport_type: TransportType,
    seed_dir: String,
) -> Result<(), Box<dyn Error>> {
    println!("=== Simple Kademlia Bootstrap Node ===\n");
    println!("Configuration:");
    println!("  Listen Address: {}:{}", listen_addr, port);
    if !seed_dir.is_empty() {
        println!("  Seed Directory: {}", seed_dir);
    }
    println!();

    // Generate local key and PeerId
    let local_key = identity::Keypair::generate_ed25519();
    let local_peer_id = PeerId::from(local_key.public());
    println!("Local peer id: {}\n", local_peer_id);

    // Initialize QUIC diagnostics manager
    let diagnostics = Arc::new(QuicDiagnosticsManager::new());
    let diagnostics_clone = diagnostics.clone();

    // Transport: QUIC/TCP selectable (default dual-stack)
    let transport = create_transport(&local_key, transport_type)?;

    // Kademlia DHT behaviour (bootstrap node) - Large timeout for reliable discovery
    let store = kad::store::MemoryStore::new(local_peer_id);
    let mut kademlia_config = kad::Config::default();
    kademlia_config.set_query_timeout(Duration::from_secs(120)); // Large timeout for reliable DHT operations
    let kademlia = kad::Behaviour::with_config(local_peer_id, store, kademlia_config);

    // Identify so clients can learn our addresses/peer id
    let identify = libp2p::identify::Behaviour::new(
        libp2p::identify::Config::new(
            "punch-simple-bootstrap/1.0.0".to_string(),
            local_key.public(),
        )
    );

    // Ping protocol for connection keepalive (sends pings every 25 seconds)
    let ping = ping::Behaviour::new(
        ping::Config::new()
            .with_interval(Duration::from_secs(25)) // Ping every 25 seconds
            .with_timeout(Duration::from_secs(10)), // 10 second timeout
    );

    // Relay protocol for NAT traversal
    // Server acts as a relay to help peers behind NAT connect
    let relay = relay::Behaviour::new(
        local_peer_id,
        relay::Config::default(),
    );

    // Initialize torrent server if seed directory is provided
    let torrent_server = if !seed_dir.is_empty() {
        match TorrentServer::new(&seed_dir) {
            Ok(server) => {
                println!("[TORRENT] ✓ Torrent seeding enabled");
                Some(Arc::new(RwLock::new(server)))
            }
            Err(e) => {
                eprintln!("[TORRENT] ⚠️  Failed to initialize torrent server: {}", e);
                None
            }
        }
    } else {
        None
    };

    // Torrent protocol for file seeding
    let torrent_response = request_response::Behaviour::with_codec(
        TorrentCodec,
        [(StreamProtocol::new("/torrent/1.0"), ProtocolSupport::Full)],
        request_response::Config::default(),
    );

    let behaviour = Behaviour { 
        kademlia, 
        identify, 
        ping, 
        relay,
        torrent_response,
    };
    
    // Swarm - Increased idle timeout since ping keeps connections alive
    let swarm_config = SwarmConfig::with_tokio_executor()
        .with_idle_connection_timeout(Duration::from_secs(90));
    let mut swarm = Swarm::new(
        transport,
        behaviour,
        local_peer_id,
        swarm_config,
    );

    // Listen on specified address and port
    println!("Starting server...");
    match transport_type {
        TransportType::DualStack => {
            let (quic, tcp) = get_dual_listen_addresses(port);
            let quic_addr: Multiaddr = quic.replace("0.0.0.0", &listen_addr).parse()?;
            let tcp_addr: Multiaddr = tcp.replace("0.0.0.0", &listen_addr).parse()?;
            swarm.listen_on(quic_addr)?;
            swarm.listen_on(tcp_addr)?;
        }
        other => {
            let addr: Multiaddr =
                get_listen_address(other, port).replace("0.0.0.0", &listen_addr).parse()?;
            swarm.listen_on(addr)?;
        }
    }

    println!("\n✅ Bootstrap node started! Waiting for connections...\n");
    println!("Clients can bootstrap to this node using:");
    println!("  --bootstrap /ip4/{}/udp/{}/quic-v1  (QUIC)", listen_addr, port);
    println!("  --bootstrap /ip4/{}/tcp/{}          (TCP)", listen_addr, port);
    println!("\nQUIC Diagnostics available at: http://{}:{}/diagnostics", listen_addr, port);
    println!("\nPress Ctrl+C to stop the bootstrap node.\n");

    // Start HTTP server for diagnostics in background
    let diagnostics_http = diagnostics.clone();
    let http_port = port + 1; // Use port + 1 for HTTP diagnostics
    tokio::spawn(async move {
        start_diagnostics_server(diagnostics_http, http_port).await;
    });

    // Main event loop
    loop {
        match swarm.select_next_some().await {
            SwarmEvent::NewListenAddr { address, .. } => {
                println!("[SERVER] Listening on {}", address);
            }
            SwarmEvent::ConnectionEstablished { peer_id, endpoint, .. } => {
                let addr_str = endpoint.get_remote_address().to_string();
                let remote_addr = endpoint.get_remote_address();
                // ConnectedPoint doesn't have get_local_address(), we'll use None
                let local_addr: Option<&Multiaddr> = None;
                
                println!("[SERVER] ✓ Connection established from peer: {}", peer_id);
                if endpoint.is_dialer() {
                    println!("[SERVER]   (Outbound connection)");
                } else {
                    println!("[SERVER]   (Inbound connection from {})", addr_str);
                    // Record connection attempt and establishment
                    diagnostics.record_connection_attempt(Some(peer_id), Some(remote_addr), local_addr).await;
                }
                
                // Determine if this is QUIC (UDP) or TCP
                let is_quic = addr_str.contains("/udp/") || addr_str.contains("/quic-v1");
                if is_quic {
                    diagnostics.record_connection_established(peer_id, remote_addr, local_addr, None).await;
                    diagnostics.record_handshake_stage(Some(peer_id), Some(remote_addr), QuicHandshakeStage::Completed).await;
                }
            }
            SwarmEvent::ConnectionClosed { peer_id, cause, .. } => {
                println!("[SERVER] ✗ Connection closed: peer {}, cause: {:?}", peer_id, cause);
                // Try to get remote address from cause or use a placeholder
                // Note: ConnectionClosed doesn't provide endpoint, so we'll use peer_id lookup
                let cause_str = format!("{:?}", cause);
                diagnostics.record_connection_closed(peer_id, &Multiaddr::empty(), Some(&cause_str)).await;
            }
            SwarmEvent::Behaviour(behaviour_event) => {
                match behaviour_event {
                    BehaviourEvent::Kademlia(kad::Event::RoutingUpdated { .. }) => {
                        println!("[BOOTSTRAP] DHT routing table updated");
                    }
                    BehaviourEvent::Kademlia(e) => {
                        println!("[BOOTSTRAP] [Kademlia Event] {:?}", e);
                    }
                    BehaviourEvent::Identify(libp2p::identify::Event::Received { peer_id, info }) => {
                        println!("[BOOTSTRAP] [Identify] Received from peer: {}", peer_id);
                        println!("[BOOTSTRAP]   Protocol: {:?}", info.protocol_version);
                        println!("[BOOTSTRAP]   Agent: {:?}", info.agent_version);
                    }
                    BehaviourEvent::TorrentResponse(request_response::Event::Message { message, .. }) => {
                        if let Some(ref torrent) = torrent_server {
                            match message {
                                request_response::Message::Request { request, channel, .. } => {
                                    let torrent_guard = torrent.read().await;
                                    let response = match request {
                                        TorrentMessage::ListFiles => {
                                            let files = torrent_guard.get_file_list();
                                            TorrentMessage::FileList { files }
                                        }
                                        TorrentMessage::RequestMetadata { info_hash } => {
                                            let info_hash_clone = info_hash.clone();
                                            if let Some(metadata) = torrent_guard.get_metadata(&info_hash_clone) {
                                                TorrentMessage::Metadata { metadata: metadata.clone() }
                                            } else {
                                                TorrentMessage::FileList { files: vec![] }
                                            }
                                        }
                                        TorrentMessage::RequestPiece { info_hash, piece_index } => {
                                            let info_hash_clone = info_hash.clone();
                                            if let Some(piece_data) = torrent_guard.get_piece(&info_hash_clone, piece_index) {
                                                TorrentMessage::PieceData { info_hash: info_hash_clone, piece_index, data: piece_data }
                                            } else {
                                                TorrentMessage::FileList { files: vec![] }
                                            }
                                        }
                                        _ => TorrentMessage::FileList { files: vec![] },
                                    };
                                    drop(torrent_guard);
                                    if let Err(e) = swarm.behaviour_mut().torrent_response.send_response(channel, response) {
                                        eprintln!("[TORRENT] Failed to send response: {:?}", e);
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                    _ => {}
                }
            }
            SwarmEvent::OutgoingConnectionError { error, peer_id, .. } => {
                println!("[SERVER] ✗ Outgoing connection error to {:?}: {:?}", peer_id, error);
            }
            SwarmEvent::IncomingConnectionError { send_back_addr, error, .. } => {
                // Extract IP address from send_back_addr for fail2ban monitoring
                let ip_addr = extract_ip_from_multiaddr(&send_back_addr);
                let error_str = format!("{:?}", error);
                eprintln!("[SECURITY] Incoming connection error from {}: {:?}", 
                    ip_addr.as_deref().unwrap_or("unknown"), error);
                // Log in fail2ban-friendly format: IP address in standard format
                if let Some(ip) = ip_addr {
                    eprintln!("[FAIL2BAN] Connection attempt failed from IP: {}", ip);
                }
                
                // Determine handshake stage from error
                let handshake_stage = if error_str.contains("HandshakeTimedOut") || error_str.contains("timeout") {
                    Some(QuicHandshakeStage::Failed)
                } else if error_str.contains("Initial") {
                    Some(QuicHandshakeStage::Initial)
                } else if error_str.contains("Handshake") {
                    Some(QuicHandshakeStage::Handshake)
                } else {
                    None
                };
                
                // Record in diagnostics
                diagnostics.record_connection_error(None, Some(&send_back_addr), &error_str, handshake_stage).await;
            }
            _ => {}
        }
    }
}

/// Start HTTP server for QUIC diagnostics
async fn start_diagnostics_server(diagnostics: Arc<QuicDiagnosticsManager>, port: u16) {
    // Serve static HTML file
    let diagnostics_html = include_str!("../diagnostics.html").to_string();
    
    let app = Router::new()
        .route("/", get(move || {
            let html = diagnostics_html.clone();
            async move { axum::response::Html(html) }
        }))
        .route("/diagnostics", get(get_diagnostics))
        .route("/diagnostics/events", get(get_recent_events))
        .route("/diagnostics/errors", get(get_error_log))
        .route("/diagnostics/connection/:peer_id/:addr", get(get_connection_stats))
        .route("/diagnostics/health", get(health_check))
        .with_state(diagnostics);

    let addr = format!("0.0.0.0:{}", port);
    let listener = match tokio::net::TcpListener::bind(&addr).await {
        Ok(l) => l,
        Err(e) => {
            eprintln!("[DIAGNOSTICS] Failed to bind HTTP server on {}: {}", addr, e);
            return;
        }
    };

    println!("[DIAGNOSTICS] HTTP server listening on http://{}", addr);
    if let Err(e) = axum::serve(listener, app).await {
        eprintln!("[DIAGNOSTICS] HTTP server error: {}", e);
    }
}

/// Get full diagnostics snapshot
async fn get_diagnostics(
    State(diagnostics): State<Arc<QuicDiagnosticsManager>>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let diag = diagnostics.get_diagnostics().await;
    Ok(Json(serde_json::to_value(diag).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?))
}

/// Get recent events
async fn get_recent_events(
    State(diagnostics): State<Arc<QuicDiagnosticsManager>>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let limit = params
        .get("limit")
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(100);
    let events = diagnostics.get_recent_events(limit).await;
    Ok(Json(serde_json::to_value(events).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?))
}

/// Get error log
async fn get_error_log(
    State(diagnostics): State<Arc<QuicDiagnosticsManager>>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let limit = params
        .get("limit")
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(100);
    let errors = diagnostics.get_error_log(limit).await;
    Ok(Json(serde_json::to_value(errors).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?))
}

/// Get connection stats for specific peer
async fn get_connection_stats(
    State(diagnostics): State<Arc<QuicDiagnosticsManager>>,
    AxumPath((peer_id, addr)): AxumPath<(String, String)>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    match diagnostics.get_connection_stats(&peer_id, &addr).await {
        Some(stats) => Ok(Json(serde_json::to_value(stats).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?)),
        None => Err(StatusCode::NOT_FOUND),
    }
}

/// Health check endpoint
async fn health_check() -> Result<Json<serde_json::Value>, StatusCode> {
    Ok(Json(serde_json::json!({
        "status": "ok",
        "service": "quic-diagnostics"
    })))
}

/// Run bootstrap server (extracted for unified binary).
///
/// Backwards-compatible wrapper that defaults to dual-stack transport.
pub async fn run_bootstrap(listen_addr: String, port: u16) -> Result<(), Box<dyn Error>> {
    run_bootstrap_with_transport(listen_addr, port, TransportType::DualStack, String::new()).await
}

#[allow(dead_code)]
#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();
    run_bootstrap_with_transport(args.listen_addr, args.port, args.transport, args.seed_dir).await
}

