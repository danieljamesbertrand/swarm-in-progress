//! Torrent File Server - Serves files via BitTorrent-like protocol over libp2p
//! Usage: cargo run --bin torrent_server [--bootstrap ADDR] [--share-dir DIR] [--port PORT]

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
use std::path::{Path, PathBuf};
use serde::{Serialize, Deserialize};
use sha2::{Sha256, Digest};

#[derive(Parser, Debug)]
#[command(name = "torrent_server")]
#[command(about = "Torrent File Server - Serves files via P2P network")]
struct Args {
    /// Bootstrap node address (Multiaddr format)
    #[arg(long, default_value = "/ip4/127.0.0.1/tcp/51820")]
    bootstrap: String,

    /// Directory to share files from
    #[arg(long, default_value = "./shared")]
    share_dir: String,

    /// Listen port (0 = random)
    #[arg(long, default_value = "0")]
    port: u16,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
struct TorrentMetadata {
    info_hash: String,
    filename: String,
    file_size: u64,
    piece_size: u64,
    pieces: Vec<String>, // SHA256 hashes of pieces
    announce: Vec<String>, // Peer addresses
}

#[derive(Clone, Serialize, Deserialize, Debug)]
enum TorrentMessage {
    // Request a piece of a file
    RequestPiece {
        info_hash: String,
        piece_index: u64,
    },
    // Response with piece data
    PieceData {
        info_hash: String,
        piece_index: u64,
        data: Vec<u8>,
    },
    // Request file metadata
    RequestMetadata {
        info_hash: String,
    },
    // Response with metadata
    Metadata {
        metadata: TorrentMetadata,
    },
    // List available files
    ListFiles,
    // Response with file list
    FileList {
        files: Vec<TorrentFileInfo>,
    },
}

#[derive(Clone, Serialize, Deserialize, Debug)]
struct TorrentFileInfo {
    info_hash: String,
    filename: String,
    size: u64,
}

#[derive(NetworkBehaviour)]
struct Behaviour {
    kademlia: kad::Behaviour<kad::store::MemoryStore>,
    identify: libp2p::identify::Behaviour,
    request_response: request_response::Behaviour<TorrentCodec>,
    relay: relay::Behaviour,
}

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

struct TorrentServer {
    share_dir: PathBuf,
    files: HashMap<String, TorrentFile>, // info_hash -> file
}

struct TorrentFile {
    path: PathBuf,
    metadata: TorrentMetadata,
    pieces: Vec<Vec<u8>>,
}

impl TorrentServer {
    fn new(share_dir: &str) -> Result<Self, Box<dyn Error>> {
        let share_path = PathBuf::from(share_dir);
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
            return Ok(());
        }

        for entry in std::fs::read_dir(&self.share_dir)? {
            let entry = entry?;
            let path = entry.path();
            
            if path.is_file() {
                if let Some(file) = Self::load_file(&path)? {
                    let info_hash = file.metadata.info_hash.clone();
                    self.files.insert(info_hash, file);
                }
            }
        }

        println!("[TORRENT] Loaded {} file(s) for sharing", self.files.len());
        Ok(())
    }

    fn load_file(path: &Path) -> Result<Option<TorrentFile>, Box<dyn Error>> {
        let file_data = std::fs::read(path)?;
        let file_size = file_data.len() as u64;
        let piece_size_bytes = 64 * 1024; // 64 KB pieces (usize for chunks)
        let piece_size = piece_size_bytes as u64; // u64 for metadata
        
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

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();
    
    println!("=== Torrent File Server ===\n");
    println!("Share Directory: {}", args.share_dir);
    println!("Bootstrap: {}\n", args.bootstrap);

    // Load files to share
    let server = std::sync::Arc::new(std::sync::Mutex::new(TorrentServer::new(&args.share_dir)?));
    
    // Generate peer identity
    let key = identity::Keypair::generate_ed25519();
    let peer_id = PeerId::from(key.public());
    println!("Peer ID: {}\n", peer_id);

    // Setup transport
    let transport = tcp::tokio::Transport::default()
        .upgrade(libp2p::core::upgrade::Version::V1)
        .authenticate(noise::Config::new(&key)?)
        .multiplex(yamux::Config::default())
        .boxed();

    // Setup Kademlia DHT
    let store = kad::store::MemoryStore::new(peer_id);
    let mut kademlia_config = kad::Config::default();
    kademlia_config.set_query_timeout(Duration::from_secs(60));
    let mut kademlia = kad::Behaviour::with_config(peer_id, store, kademlia_config);

    // Add bootstrap node
    let bootstrap_addr: Multiaddr = args.bootstrap.parse()?;
    let bootstrap_peer_id = PeerId::from(key.public()); // In real scenario, you'd get this from bootstrap
    kademlia.add_address(&bootstrap_peer_id, bootstrap_addr.clone());

    // Identify
    let identify = libp2p::identify::Behaviour::new(
        libp2p::identify::Config::new("torrent-server/1.0".to_string(), key.public())
    );

    // Request-Response for torrent protocol
    let codec = TorrentCodec;
    let request_response = request_response::Behaviour::with_codec(
        codec,
        [(StreamProtocol::new("/torrent/1.0"), ProtocolSupport::Full)],
        request_response::Config::default(),
    );

    // Relay for NAT traversal
    let relay = relay::Behaviour::new(peer_id, relay::Config::default());

    let behaviour = Behaviour { kademlia, identify, request_response, relay };

    // Swarm
    let swarm_config = SwarmConfig::with_tokio_executor()
        .with_idle_connection_timeout(Duration::from_secs(60));
    let mut swarm = Swarm::new(transport, behaviour, peer_id, swarm_config);

    // Listen
    let listen_addr = if args.port == 0 {
        "/ip4/0.0.0.0/tcp/0".parse()?
    } else {
        format!("/ip4/0.0.0.0/tcp/{}", args.port).parse()?
    };
    swarm.listen_on(listen_addr)?;

    // Connect to bootstrap
    swarm.dial(bootstrap_addr)?;

    let mut bootstrapped = false;
    let mut registered = false;

    println!("✅ Torrent server started!");
    println!("   Share directory: {}", args.share_dir);
    println!("   Files available: {}\n", server.lock().unwrap().files.len());

    // Register files in DHT
    let server_clone = server.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(5)).await;
        let files = server_clone.lock().unwrap().get_file_list();
        for file in files {
            println!("[TORRENT] Sharing: {} (hash: {})", file.filename, &file.info_hash[..16]);
        }
    });

    loop {
        match swarm.select_next_some().await {
            SwarmEvent::NewListenAddr { address, .. } => {
                println!("[TORRENT] Listening on: {}", address);
                swarm.add_external_address(address);
            }
            SwarmEvent::ConnectionEstablished { .. } => {
                if !bootstrapped {
                    if let Err(e) = swarm.behaviour_mut().kademlia.bootstrap() {
                        eprintln!("[WARN] Bootstrap failed: {:?}", e);
                    } else {
                        println!("✓ Started Kademlia bootstrap!");
                    }
                }
            }
            SwarmEvent::Behaviour(behaviour_event) => {
                match behaviour_event {
                    BehaviourEvent::Kademlia(kad::Event::RoutingUpdated { .. }) => {
                        if !bootstrapped {
                            bootstrapped = true;
                            println!("✓ DHT bootstrapped!");
                            
                            // Register our files in DHT
                            let files = server.lock().unwrap().get_file_list();
                            let file_count = files.len();
                            for file in &files {
                                let key = kad::RecordKey::new(&file.info_hash);
                                match serde_json::to_vec(file) {
                                    Ok(value) => {
                                        let record = kad::Record::new(key, value);
                                        if let Err(e) = swarm.behaviour_mut().kademlia.put_record(record, kad::Quorum::One) {
                                            eprintln!("[WARN] Failed to register file {}: {:?}", file.filename, e);
                                        }
                                    }
                                    Err(e) => {
                                        eprintln!("[WARN] Failed to serialize file {}: {:?}", file.filename, e);
                                    }
                                }
                            }
                            println!("✓ Registered {} file(s) in DHT", file_count);
                        }
                    }
                    BehaviourEvent::RequestResponse(request_response::Event::Message { message, .. }) => {
                        match message {
                            request_response::Message::Request { request, channel, .. } => {
                                let server_guard = server.lock().unwrap();
                                let response = match request {
                                    TorrentMessage::ListFiles => {
                                        println!("[TORRENT] 📥 File list requested");
                                        TorrentMessage::FileList {
                                            files: server_guard.get_file_list(),
                                        }
                                    }
                                    TorrentMessage::RequestMetadata { info_hash } => {
                                        println!("[TORRENT] 📥 Metadata requested for: {}", &info_hash[..16]);
                                        if let Some(metadata) = server_guard.get_metadata(&info_hash) {
                                            TorrentMessage::Metadata {
                                                metadata: metadata.clone(),
                                            }
                                        } else {
                                            continue; // File not found
                                        }
                                    }
                                    TorrentMessage::RequestPiece { info_hash, piece_index } => {
                                        println!("[TORRENT] 📥 Piece {} requested for: {}", piece_index, &info_hash[..16]);
                                        if let Some(piece_data) = server_guard.get_piece(&info_hash, piece_index) {
                                            TorrentMessage::PieceData {
                                                info_hash,
                                                piece_index,
                                                data: piece_data,
                                            }
                                        } else {
                                            continue; // Piece not found
                                        }
                                    }
                                    _ => continue,
                                };
                                drop(server_guard);
                                
                                if let Err(e) = swarm.behaviour_mut().request_response.send_response(channel, response) {
                                    eprintln!("[ERROR] Failed to send response: {:?}", e);
                                } else {
                                    println!("[TORRENT] ✓ Response sent");
                                }
                            }
                            _ => {}
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
}

