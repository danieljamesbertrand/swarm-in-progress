//! Torrent File Client - Downloads files via BitTorrent-like protocol over libp2p
//! Usage: cargo run --bin torrent_client [--bootstrap ADDR] [--download-dir DIR] [--info-hash HASH]

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
use std::path::PathBuf;
use serde::{Serialize, Deserialize};
use sha2::{Sha256, Digest};

#[derive(Parser, Debug)]
#[command(name = "torrent_client")]
#[command(about = "Torrent File Client - Downloads files via P2P network")]
struct Args {
    /// Bootstrap node address (Multiaddr format)
    #[arg(long, default_value = "/ip4/127.0.0.1/tcp/51820")]
    bootstrap: String,

    /// Directory to download files to
    #[arg(long, default_value = "./downloads")]
    download_dir: String,

    /// Info hash of file to download (optional - will list files if not provided)
    #[arg(long)]
    info_hash: Option<String>,
}

// Same message types as torrent_server
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

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();
    
    println!("=== Torrent File Client ===\n");
    println!("Download Directory: {}", args.download_dir);
    println!("Bootstrap: {}\n", args.bootstrap);

    // Create download directory
    std::fs::create_dir_all(&args.download_dir)?;

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

    // Setup Kademlia DHT - Large timeout for reliable discovery
    let store = kad::store::MemoryStore::new(peer_id);
    let mut kademlia_config = kad::Config::default();
    kademlia_config.set_query_timeout(Duration::from_secs(120)); // Large timeout for reliable DHT operations
    let mut kademlia = kad::Behaviour::with_config(peer_id, store, kademlia_config);

    // Add bootstrap node
    let bootstrap_addr: Multiaddr = args.bootstrap.parse()?;
    let bootstrap_peer_id = PeerId::from(key.public());
    kademlia.add_address(&bootstrap_peer_id, bootstrap_addr.clone());

    // Identify
    let identify = libp2p::identify::Behaviour::new(
        libp2p::identify::Config::new("torrent-client/1.0".to_string(), key.public())
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
    swarm.listen_on("/ip4/0.0.0.0/tcp/0".parse()?)?;

    // Connect to bootstrap
    swarm.dial(bootstrap_addr)?;

    let mut bootstrapped = false;
    let mut connected_peers: HashMap<PeerId, ()> = HashMap::new();

    println!("✅ Torrent client started!\n");

    loop {
        match swarm.select_next_some().await {
            SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                if !bootstrapped {
                    if let Err(e) = swarm.behaviour_mut().kademlia.bootstrap() {
                        eprintln!("[WARN] Bootstrap failed: {:?}", e);
                    } else {
                        println!("✓ Started Kademlia bootstrap!");
                    }
                } else {
                    connected_peers.insert(peer_id, ());
                    println!("✓ Connected to peer: {}", peer_id);
                    
                    // If we have an info_hash, start downloading
                    if let Some(ref info_hash) = args.info_hash {
                        if !connected_peers.is_empty() {
                            let target_peer = *connected_peers.keys().next().unwrap();
                            println!("\n[DOWNLOAD] Requesting metadata for: {}", &info_hash[..16]);
                            let _ = swarm.behaviour_mut().request_response.send_request(
                                &target_peer,
                                TorrentMessage::RequestMetadata {
                                    info_hash: info_hash.clone(),
                                }
                            );
                        }
                    }
                }
            }
            SwarmEvent::Behaviour(behaviour_event) => {
                match behaviour_event {
                    BehaviourEvent::Kademlia(kad::Event::RoutingUpdated { .. }) => {
                        if !bootstrapped {
                            bootstrapped = true;
                            println!("✓ DHT bootstrapped!");
                            
                            // Query DHT for available files
                            if args.info_hash.is_none() {
                                println!("\n[TORRENT] Discovering files in DHT...");
                                // Query for file records
                                let key = kad::RecordKey::new("torrent-files");
                                swarm.behaviour_mut().kademlia.get_record(key);
                            }
                        }
                    }
                    BehaviourEvent::Kademlia(kad::Event::OutboundQueryProgressed { result, .. }) => {
                        match result {
                            kad::QueryResult::GetRecord(Ok(kad::GetRecordOk::FoundRecord(record))) => {
                                // Found file record in DHT
                                if let Ok(file_info) = serde_json::from_slice::<TorrentFileInfo>(record.value.as_ref()) {
                                    println!("[TORRENT] 📁 Found file in DHT: {} ({})", file_info.filename, &file_info.info_hash[..16]);
                                }
                            }
                            kad::QueryResult::GetClosestPeers(Ok(ok)) => {
                                // Found peers - connect to them
                                for peer in ok.peers {
                                    if !connected_peers.contains_key(&peer) && peer != peer_id {
                                        if let Err(e) = swarm.dial(format!("/p2p/{}", peer).parse()?) {
                                            eprintln!("[WARN] Failed to dial peer {}: {:?}", peer, e);
                                        }
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                    BehaviourEvent::RequestResponse(request_response::Event::Message { message, .. }) => {
                        match message {
                            request_response::Message::Response { response, .. } => {
                                match response {
                                    TorrentMessage::FileList { files } => {
                                        println!("\n[TORRENT] 📁 Available files:");
                                        for file in files {
                                            println!("  - {} ({} bytes, hash: {})", 
                                                file.filename, file.size, &file.info_hash[..16]);
                                        }
                                    }
                                    TorrentMessage::Metadata { metadata } => {
                                        println!("\n[DOWNLOAD] 📥 Starting download: {}", metadata.filename);
                                        println!("  Size: {} bytes", metadata.file_size);
                                        println!("  Pieces: {}", metadata.pieces.len());
                                        
                                        // Download all pieces
                                        let download_dir = PathBuf::from(&args.download_dir);
                                        let file_path = download_dir.join(&metadata.filename);
                                        let mut file_data = Vec::new();
                                        
                                        for (i, _) in metadata.pieces.iter().enumerate() {
                                            if let Some(peer) = connected_peers.keys().next() {
                                                let request = TorrentMessage::RequestPiece {
                                                    info_hash: metadata.info_hash.clone(),
                                                    piece_index: i as u64,
                                                };
                                                let _ = swarm.behaviour_mut().request_response.send_request(peer, request);
                                            }
                                        }
                                        
                                        // Wait for pieces and assemble file
                                        // (In production, this would be more sophisticated)
                                    }
                                    TorrentMessage::PieceData { info_hash, piece_index, data } => {
                                        println!("[DOWNLOAD] ✓ Received piece {} ({} bytes)", piece_index, data.len());
                                        // Assemble file from pieces
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
            _ => {}
        }
    }
}













