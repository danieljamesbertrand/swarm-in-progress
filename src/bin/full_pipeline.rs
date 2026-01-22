//! Full AI Pipeline Executable
//!
//! This executable guarantees a complete AI inference pipeline by:
//! 1. Discovering shards via Kademlia DHT
//! 2. Loading actual .gguf model files
//! 3. Executing inference through the distributed pipeline
//! 4. Assembling and returning results
//!
//! Usage:
//!   # As coordinator (discovers and orchestrates):
//!   cargo run --bin full_pipeline -- coordinator --prompt "What is AI?"
//!
//!   # As shard node (processes shards):
//!   cargo run --bin full_pipeline -- shard --shard-id 0 --total-shards 4
//!
//!   # As both (for testing):
//!   cargo run --bin full_pipeline -- both --shard-id 0 --prompt "What is AI?"

#![allow(warnings)]
#![allow(dead_code)]
#![allow(unused_variables)]
#![allow(unused_imports)]
#![allow(unused_mut)]
#![allow(unused_assignments)]
#![allow(unused_must_use)]
#![allow(clippy::all)]

use clap::{Parser, Subcommand};
use libp2p::{
    identity,
    kad,
    ping,
    request_response::{self, ProtocolSupport},
    swarm::{NetworkBehaviour, Swarm, SwarmEvent},
    PeerId, Multiaddr, StreamProtocol,
};
use libp2p::swarm::Config as SwarmConfig;
use libp2p::futures::StreamExt;
use punch_simple::quic_transport::{create_transport, TransportType, get_dual_listen_addresses};
use punch_simple::kademlia_shard_discovery::{KademliaShardDiscovery, ShardAnnouncement, dht_keys};
use punch_simple::command_protocol::{Command, CommandResponse, commands};
use punch_simple::message::{JsonCodec, JsonMessage};
use punch_simple::pipeline_coordinator::{PipelineCoordinator, InferenceRequest, PipelineStrategy, NodeSpawner, PipelineError};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{RwLock, Mutex, oneshot};
use serde_json::json;
use std::collections::HashMap;
use std::path::PathBuf;
use uuid::Uuid;
use std::net::ToSocketAddrs;

#[derive(Parser)]
#[command(name = "full_pipeline")]
#[command(about = "Full AI Pipeline - Guaranteed end-to-end distributed inference")]
struct Args {
    #[command(subcommand)]
    mode: Mode,
    
    /// Bootstrap server address (QUIC format for eagleoneonline.ca)
    #[arg(long, default_value = "/ip4/eagleoneonline.ca/udp/51820/quic-v1")]
    bootstrap: String,
}

#[derive(Subcommand)]
enum Mode {
    /// Run as coordinator (discovers shards and orchestrates inference)
    Coordinator {
        /// Prompt to process
        #[arg(long)]
        prompt: String,
        
        /// Maximum tokens to generate
        #[arg(long, default_value = "256")]
        max_tokens: u32,
        
        /// Temperature for sampling
        #[arg(long, default_value = "0.7")]
        temperature: f64,
    },
    
    /// Run as shard node (processes assigned shard)
    Shard {
        /// Shard ID (0, 1, 2, 3)
        #[arg(long)]
        shard_id: u32,
        
        /// Total number of shards
        #[arg(long, default_value = "4")]
        total_shards: u32,
        
        /// Cluster name
        #[arg(long, default_value = "llama-8b-cluster")]
        cluster: String,
        
        /// Shards directory
        #[arg(long, default_value = "models_cache/shards")]
        shards_dir: String,
    },
    
    /// Run as both coordinator and shard (for testing)
    Both {
        /// Shard ID for this node
        #[arg(long)]
        shard_id: u32,
        
        /// Total number of shards
        #[arg(long, default_value = "4")]
        total_shards: u32,
        
        /// Prompt to process
        #[arg(long)]
        prompt: String,
        
        /// Cluster name
        #[arg(long, default_value = "llama-8b-cluster")]
        cluster: String,
        
        /// Shards directory
        #[arg(long, default_value = "models_cache/shards")]
        shards_dir: String,
    },
}

#[derive(NetworkBehaviour)]
struct Behaviour {
    kademlia: kad::Behaviour<kad::store::MemoryStore>,
    request_response: request_response::Behaviour<JsonCodec>,
    ping: ping::Behaviour,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    
    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║  FULL AI PIPELINE - GUARANTEED END-TO-END                   ║");
    println!("╚══════════════════════════════════════════════════════════════╝\n");
    
    match args.mode {
        Mode::Coordinator { prompt, max_tokens, temperature } => {
            run_coordinator(prompt, max_tokens, temperature, &args.bootstrap).await?;
        }
        Mode::Shard { shard_id, total_shards, cluster, shards_dir } => {
            run_shard_node(shard_id, total_shards, cluster, shards_dir, &args.bootstrap).await?;
        }
        Mode::Both { shard_id, total_shards, prompt, cluster, shards_dir } => {
            run_both(shard_id, total_shards, prompt, cluster, shards_dir, &args.bootstrap).await?;
        }
    }
    
    Ok(())
}

/// Run as coordinator - discovers shards and orchestrates inference
async fn run_coordinator(
    prompt: String,
    max_tokens: u32,
    temperature: f64,
    bootstrap: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("[MODE] Running as COORDINATOR");
    println!("[MODE] Prompt: \"{}\"", prompt);
    println!("[MODE] Bootstrap: {}\n", bootstrap);
    
    // Create identity
    let keypair = identity::Keypair::generate_ed25519();
    let peer_id = PeerId::from(keypair.public());
    
    // Create transport
    let transport = create_transport(&keypair, TransportType::DualStack)?;
    
    // Create Kademlia
    let store = kad::store::MemoryStore::new(peer_id);
    let mut kademlia_config = kad::Config::default();
    kademlia_config.set_query_timeout(Duration::from_secs(120));
    let kademlia = kad::Behaviour::with_config(peer_id, store, kademlia_config);
    
    // Create request-response
    let request_response = request_response::Behaviour::with_codec(
        JsonCodec,
        [(StreamProtocol::new("/json-message/1.0"), ProtocolSupport::Full)],
        request_response::Config::default(),
    );
    
    // Create ping
    let ping = ping::Behaviour::new(
        ping::Config::new()
            .with_interval(Duration::from_secs(25))
            .with_timeout(Duration::from_secs(10)),
    );
    
    let behaviour = Behaviour {
        kademlia,
        request_response,
        ping,
    };
    
    // Create swarm
    let mut swarm = Swarm::new(
        transport,
        behaviour,
        peer_id,
        SwarmConfig::with_tokio_executor()
            .with_idle_connection_timeout(Duration::from_secs(90)),
    );
    
    // Listen
    let (quic_addr, tcp_addr) = get_dual_listen_addresses(0);
    swarm.listen_on(quic_addr.parse()?)?;
    swarm.listen_on(tcp_addr.parse()?)?;
    
    println!("[NETWORK] Listening on QUIC: {}", quic_addr);
    println!("[NETWORK] Listening on TCP: {}\n", tcp_addr);
    
    // Connect to bootstrap - resolve hostname if needed
    let (quic_bootstrap, tcp_bootstrap) = resolve_bootstrap_address(bootstrap)?;
    println!("[BOOTSTRAP] Resolved bootstrap addresses:");
    println!("[BOOTSTRAP]   QUIC: {}", quic_bootstrap);
    println!("[BOOTSTRAP]   TCP:  {}", tcp_bootstrap);
    println!("[BOOTSTRAP] Dialing bootstrap node (trying QUIC first, then TCP)...\n");
    
    // Add bootstrap addresses to Kademlia before dialing
    // Extract peer ID from bootstrap connection (we'll get it after connection)
    // For now, try both QUIC and TCP
    swarm.dial(quic_bootstrap.clone())?;
    swarm.dial(tcp_bootstrap.clone())?;
    
    // Add external addresses so we can be discovered
    swarm.add_external_address(quic_bootstrap.clone());
    swarm.add_external_address(tcp_bootstrap.clone());
    
    // Create discovery
    let discovery = Arc::new(RwLock::new(
        KademliaShardDiscovery::with_expected_shards("llama-8b-cluster", 4)
    ));
    
    // Create command sender
    let swarm_arc = Arc::new(Mutex::new(swarm));
    let pending_responses: Arc<Mutex<HashMap<String, oneshot::Sender<CommandResponse>>>> = 
        Arc::new(Mutex::new(HashMap::new()));
    
    let command_sender = {
        let swarm_clone = Arc::clone(&swarm_arc);
        let pending_clone = Arc::clone(&pending_responses);
        let sender_peer_id = peer_id;
        
        move |peer_id_str: String, cmd: Command| -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<CommandResponse, PipelineError>> + Send>> {
            let swarm_clone = Arc::clone(&swarm_clone);
            let pending_clone = Arc::clone(&pending_clone);
            let sender_peer_id = sender_peer_id;
            
            Box::pin(async move {
                let target_peer: PeerId = peer_id_str.parse()
                    .map_err(|e| PipelineError::Internal { message: format!("Invalid peer ID: {}", e) })?;
                
                let cmd_json = serde_json::to_string(&cmd)
                    .map_err(|e| PipelineError::Internal { message: format!("Serialization error: {}", e) })?;
                
                let msg = JsonMessage::new(sender_peer_id.to_string(), cmd_json);
                
                let (tx, rx) = oneshot::channel();
                let cmd_request_id = cmd.request_id.clone();
                
                {
                    let mut swarm = swarm_clone.lock().await;
                    let _ = swarm.behaviour_mut().request_response.send_request(&target_peer, msg);
                }
                
                {
                    let mut pending = pending_clone.lock().await;
                    pending.insert(cmd_request_id, tx);
                }
                
                rx.await
                    .map_err(|_| PipelineError::Internal { message: "Response channel closed".to_string() })
            })
        }
    };
    
    // Start swarm event loop in background
    let discovery_clone = Arc::clone(&discovery);
    let swarm_clone = Arc::clone(&swarm_arc);
    let pending_clone = Arc::clone(&pending_responses);
    let connected_clone = Arc::new(Mutex::new(false));
    let bootstrapped_clone = Arc::new(Mutex::new(false));
    let connected_for_bootstrap = Arc::clone(&connected_clone);
    let bootstrapped_for_bootstrap = Arc::clone(&bootstrapped_clone);
    let swarm_for_bootstrap = Arc::clone(&swarm_clone);
    let quic_bootstrap_for_events = quic_bootstrap.clone();
    let tcp_bootstrap_for_events = tcp_bootstrap.clone();
    
    tokio::spawn(async move {
        loop {
            let event = {
                let mut swarm_guard = swarm_clone.lock().await;
                swarm_guard.select_next_some().await
            };
            
            match event {
                SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                    let mut conn = connected_for_bootstrap.lock().await;
                    if !*conn {
                        println!("[BOOTSTRAP] ✓ Connected to bootstrap node {}", peer_id);
                        // Add bootstrap node to Kademlia and start bootstrap
                        {
                            let mut swarm_guard = swarm_for_bootstrap.lock().await;
                            // Add both QUIC and TCP addresses to Kademlia
                            swarm_guard.behaviour_mut().kademlia.add_address(&peer_id, quic_bootstrap_for_events.clone());
                            swarm_guard.behaviour_mut().kademlia.add_address(&peer_id, tcp_bootstrap_for_events.clone());
                            if let Err(e) = swarm_guard.behaviour_mut().kademlia.bootstrap() {
                                eprintln!("[BOOTSTRAP] Failed to start bootstrap: {:?}", e);
                            } else {
                                println!("[BOOTSTRAP] ✓ Started Kademlia bootstrap");
                            }
                        }
                        *conn = true;
                    }
                }
                SwarmEvent::Behaviour(behaviour_event) => {
                    match behaviour_event {
                        BehaviourEvent::Kademlia(kad::Event::RoutingUpdated { .. }) => {
                            let mut boot = bootstrapped_for_bootstrap.lock().await;
                            if !*boot {
                                println!("[BOOTSTRAP] ✓ Kademlia bootstrap completed");
                                *boot = true;
                            }
                        }
                        BehaviourEvent::Kademlia(kad::Event::OutboundQueryProgressed { result, .. }) => {
                            match result {
                                kad::QueryResult::GetRecord(Ok(kad::GetRecordOk::FoundRecord(record))) => {
                                    let mut discovery = discovery_clone.write().await;
                                    if let Some(announcement) = discovery.process_shard_record(&record.record) {
                                        let shard_id = announcement.shard_id;
                                        discovery.add_shard(announcement);
                                        println!("[DISCOVERY] ✓ Found shard {}", shard_id);
                                    }
                                }
                                _ => {}
                            }
                        }
                        BehaviourEvent::RequestResponse(request_response::Event::Message { message, .. }) => {
                            match message {
                                request_response::Message::Response { response, .. } => {
                                    // Handle responses
                                    if let Ok(cmd_response) = serde_json::from_str::<CommandResponse>(&response.message) {
                                        let mut pending = pending_clone.lock().await;
                                        if let Some(tx) = pending.remove(&cmd_response.request_id) {
                                            let _ = tx.send(cmd_response);
                                        }
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
    });
    
    // Wait for bootstrap to complete
    println!("[BOOTSTRAP] Waiting for bootstrap to complete...");
    let mut attempts = 0;
    loop {
        tokio::time::sleep(Duration::from_millis(500)).await;
        let conn = connected_clone.lock().await;
        let boot = bootstrapped_clone.lock().await;
        if *boot {
            drop(conn);
            drop(boot);
            break;
        }
        if attempts % 10 == 0 && attempts > 0 {
            println!("[BOOTSTRAP] Still waiting... (connected: {}, bootstrapped: {})", *conn, *boot);
        }
        drop(conn);
        drop(boot);
        attempts += 1;
        if attempts > 120 { // 60 seconds total
            return Err("Timeout waiting for bootstrap".into());
        }
    }
    println!("[BOOTSTRAP] ✓ Bootstrap complete\n");
    
    // Start periodic DHT queries for shards
    let swarm_for_queries = Arc::clone(&swarm_arc);
    let bootstrapped_for_queries = Arc::clone(&bootstrapped_clone);
    tokio::spawn(async move {
        let mut next_query = tokio::time::Instant::now() + Duration::from_secs(2);
        loop {
            tokio::time::sleep_until(next_query).await;
            
            let is_bootstrapped = *bootstrapped_for_queries.lock().await;
            if is_bootstrapped {
                {
                    let mut swarm_guard = swarm_for_queries.lock().await;
                    for shard_id in 0..4 {
                        let key = kad::RecordKey::new(&punch_simple::kademlia_shard_discovery::dht_keys::shard_key("llama-8b-cluster", shard_id));
                        swarm_guard.behaviour_mut().kademlia.get_record(key);
                    }
                }
                next_query = tokio::time::Instant::now() + Duration::from_secs(5);
            } else {
                next_query = tokio::time::Instant::now() + Duration::from_millis(100);
            }
        }
    });
    
    // Create pipeline coordinator
    // PipelineCoordinator::new takes KademliaShardDiscovery directly, but we need to share it
    // So we create a new one and wrap it in Arc<RwLock<...>> inside the coordinator
    let discovery_for_coord = KademliaShardDiscovery::with_expected_shards("llama-8b-cluster", 4);
    let mut coordinator = PipelineCoordinator::new(discovery_for_coord);
    coordinator = coordinator.with_command_sender(Box::new(command_sender));
    let coordinator = Arc::new(coordinator);
    
    // Wait for shards to be discovered
    println!("[DISCOVERY] Waiting for shards to be discovered...\n");
    
    let mut attempts = 0;
    loop {
        tokio::time::sleep(Duration::from_secs(2)).await;
        
        let discovery_guard = discovery.read().await;
        let status = discovery_guard.status();
        let pipeline = discovery_guard.get_pipeline();
        
        println!("[STATUS] Discovered: {}/{} shards", status.discovered_shards, status.expected_shards);
        for shard in &pipeline {
            println!("[STATUS]   Shard {}: Peer {} (Loaded: {})", 
                shard.shard_id, shard.peer_id, shard.capabilities.shard_loaded);
        }
        
        if status.is_complete && discovery_guard.are_all_shards_loaded() {
            println!("\n[READY] ✓ All shards discovered and loaded!\n");
            break;
        }
        
        attempts += 1;
        if attempts > 60 {
            return Err("Timeout waiting for shards".into());
        }
    }
    
    // Execute inference
    println!("[INFERENCE] Starting inference...\n");
    
    let request = InferenceRequest {
        request_id: uuid::Uuid::new_v4().to_string(),
        prompt: prompt.clone(),
        max_tokens,
        temperature: temperature as f32,
        top_p: 0.9,
        context: None,
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
        priority: 0,
    };
    
    let start = Instant::now();
    let result = coordinator.submit_inference(request).await;
    let elapsed = start.elapsed();
    
    match result {
        Ok(response) => {
            println!("\n╔══════════════════════════════════════════════════════════════╗");
            println!("║  INFERENCE SUCCESS                                             ║");
            println!("╚══════════════════════════════════════════════════════════════╝\n");
            println!("[RESULT] Generated text:");
            println!("{}\n", response.text);
            println!("[STATS] Tokens: {}", response.tokens_generated);
            println!("[STATS] Latency: {} ms", elapsed.as_millis());
            println!("[STATS] Shard latencies:");
            for shard_latency in &response.shard_latencies {
                println!("[STATS]   Shard {}: {} ms", shard_latency.shard_id, shard_latency.latency_ms);
            }
        }
        Err(e) => {
            eprintln!("\n[ERROR] Inference failed: {}", e);
            return Err(e.into());
        }
    }
    
    Ok(())
}

/// Resolve bootstrap address, converting hostnames to IP addresses
/// Returns both QUIC and TCP addresses for dual-stack transport
fn resolve_bootstrap_address(addr_str: &str) -> Result<(Multiaddr, Multiaddr), Box<dyn std::error::Error>> {
    // Extract hostname and port from multiaddr format
    // Format: /ip4/HOSTNAME/tcp/PORT or /ip4/HOSTNAME/udp/PORT/quic-v1
    let parts: Vec<&str> = addr_str.split('/').collect();
    if parts.len() >= 4 && (parts[3] == "tcp" || parts[3] == "udp") {
        let hostname = parts[2];
        let port = parts.get(4).ok_or("Invalid multiaddr format")?;
        
        // Try to resolve hostname
        let socket_addr = format!("{}:{}", hostname, port)
            .to_socket_addrs()?
            .next()
            .ok_or("Failed to resolve hostname")?;
        
        // Reconstruct multiaddr with IP address
        let ip = socket_addr.ip();
        let quic_addr = format!("/ip4/{}/udp/{}/quic-v1", ip, port).parse()?;
        let tcp_addr = format!("/ip4/{}/tcp/{}", ip, port).parse()?;
        Ok((quic_addr, tcp_addr))
    } else {
        Err("Invalid multiaddr format".into())
    }
}

/// Run as shard node - processes assigned shard
async fn run_shard_node(
    shard_id: u32,
    total_shards: u32,
    cluster: String,
    shards_dir: String,
    bootstrap: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("[MODE] Running as SHARD NODE");
    println!("[MODE] Shard ID: {}", shard_id);
    println!("[MODE] Total shards: {}", total_shards);
    println!("[MODE] Cluster: {}", cluster);
    println!("[MODE] Shards directory: {}", shards_dir);
    println!("[MODE] Bootstrap: {}\n", bootstrap);
    
    // Resolve bootstrap address first (shard_listener expects IP, not hostname)
    let (quic_bootstrap, _tcp_bootstrap) = resolve_bootstrap_address(bootstrap)?;
    // Use QUIC bootstrap for shard_listener (server is QUIC-only)
    let bootstrap_addr_str = quic_bootstrap.to_string();
    
    // Use the existing shard_listener implementation
    // Use QUIC-only transport since bootstrap server is QUIC-only
    punch_simple::shard_listener::run_shard_listener(
        bootstrap_addr_str,
        cluster,
        Some(shard_id),
        total_shards,
        32, // total_layers
        "llama-8b".to_string(),
        0, // port (auto)
        30, // refresh_interval
        shards_dir,
        true, // enable_torrent
        "quic".to_string(), // transport - QUIC-only for eagleoneonline.ca
    ).await?;
    
    Ok(())
}

/// Run as both coordinator and shard (for testing)
async fn run_both(
    shard_id: u32,
    total_shards: u32,
    prompt: String,
    cluster: String,
    shards_dir: String,
    bootstrap: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("[MODE] Running as BOTH coordinator and shard node");
    println!("[MODE] Shard ID: {}", shard_id);
    println!("[MODE] Prompt: \"{}\"\n", prompt);
    
    // Start shard node in background
    let bootstrap_str = bootstrap.to_string();
    let cluster_clone = cluster.clone();
    let shards_dir_clone = shards_dir.clone();
    let shard_handle = tokio::spawn(async move {
        punch_simple::shard_listener::run_shard_listener(
            bootstrap_str,
            cluster_clone,
            Some(shard_id),
            total_shards,
            32, // total_layers
            "llama-8b".to_string(),
            0, // port (auto)
            30, // refresh_interval
            shards_dir_clone,
            true, // enable_torrent
            "dual".to_string(), // transport
        ).await.map_err(|e| format!("Shard listener error: {}", e))
    });
    
    // Wait a bit for shard to start
    tokio::time::sleep(Duration::from_secs(5)).await;
    
    // Run coordinator
    run_coordinator(prompt, 256, 0.7, bootstrap).await?;
    
    // Keep shard running (ignore errors for now)
    match shard_handle.await {
        Ok(Ok(())) => {}
        Ok(Err(e)) => eprintln!("[WARNING] Shard node error: {}", e),
        Err(e) => eprintln!("[WARNING] Shard task error: {}", e),
    }
    
    Ok(())
}
