//! Network Monitor - Web-based monitoring and management system for Kademlia P2P network
//! Usage: cargo run --bin monitor [--listen-addr ADDR] [--port PORT] [--web-port WEB_PORT]
//! 
//! Also available via unified node binary:
//!   cargo run --bin node -- monitor --listen-addr ADDR --port PORT --web-port WEB_PORT

use clap::Parser;
use libp2p::{
    identity,
    tcp,
    noise,
    yamux,
    kad,
    ping,
    relay,
    swarm::{NetworkBehaviour, Swarm, SwarmEvent},
    core::transport::Transport,
    PeerId, Multiaddr,
};
use libp2p::swarm::Config as SwarmConfig;
use libp2p::futures::StreamExt;
use std::error::Error;
use std::time::Duration;
use std::sync::Arc;
use tokio::sync::{RwLock, broadcast};
use serde::{Serialize, Deserialize};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Parser, Debug)]
#[command(name = "monitor")]
#[command(about = "Network Monitor - Web-based monitoring for Kademlia P2P network")]
struct Args {
    /// Listen address for P2P (default: 0.0.0.0)
    #[arg(long, default_value = "0.0.0.0")]
    listen_addr: String,

    /// Listen port for P2P (default: 51820)
    #[arg(long, default_value = "51820")]
    port: u16,

    /// Web server port (default: 8080)
    #[arg(long, default_value = "8080")]
    web_port: u16,
}

#[derive(NetworkBehaviour)]
struct Behaviour {
    kademlia: kad::Behaviour<kad::store::MemoryStore>,
    identify: libp2p::identify::Behaviour,
    ping: ping::Behaviour,
    relay: relay::Behaviour,
}

// Network state for monitoring
#[derive(Clone, Serialize, Deserialize, Debug)]
struct NodeInfo {
    peer_id: String,
    first_seen: u64,
    last_seen: u64,
    connection_count: u32,
    addresses: Vec<String>,
    agent: Option<String>,
    protocol: Option<String>,
    namespace: Option<String>, // Track namespace for each node
}

#[derive(Clone, Serialize, Deserialize, Debug)]
struct ConnectionEvent {
    timestamp: u64,
    event_type: String, // "connected" | "disconnected"
    peer_id: String,
    direction: String, // "inbound" | "outbound"
}

#[derive(Clone, Serialize, Deserialize, Debug)]
struct NetworkMetrics {
    total_nodes: usize,
    active_connections: usize,
    total_connections: u64,
    dht_records: usize,
    uptime_seconds: u64,
    messages_sent: u64,
    messages_received: u64,
    // Latency metrics (in milliseconds)
    latency_min_ms: f64,
    latency_max_ms: f64,
    latency_avg_ms: f64,
    latency_p50_ms: f64,
    latency_p95_ms: f64,
    latency_p99_ms: f64,
    // Throughput metrics
    messages_per_second: f64,
    bytes_sent: u64,
    bytes_received: u64,
    // Error metrics
    message_errors: u64,
    timeout_errors: u64,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
struct NetworkState {
    bootstrap_peer_id: String,
    nodes: HashMap<String, NodeInfo>,
    connections: Vec<ConnectionEvent>,
    metrics: NetworkMetrics,
    namespaces: HashMap<String, usize>, // namespace -> node count
    active_connection_count: usize, // Track actual connection count
    latency_samples: Vec<f64>, // Store latency samples for percentile calculation
    message_timestamps: Vec<u64>, // Store message timestamps for throughput calculation
}

impl NetworkState {
    fn new(bootstrap_peer_id: PeerId) -> Self {
        Self {
            bootstrap_peer_id: bootstrap_peer_id.to_string(),
            nodes: HashMap::new(),
            connections: Vec::new(),
            metrics: NetworkMetrics {
                total_nodes: 0,
                active_connections: 0,
                total_connections: 0,
                dht_records: 0,
                uptime_seconds: 0,
                messages_sent: 0,
                messages_received: 0,
                latency_min_ms: 0.0,
                latency_max_ms: 0.0,
                latency_avg_ms: 0.0,
                latency_p50_ms: 0.0,
                latency_p95_ms: 0.0,
                latency_p99_ms: 0.0,
                messages_per_second: 0.0,
                bytes_sent: 0,
                bytes_received: 0,
                message_errors: 0,
                timeout_errors: 0,
            },
            namespaces: HashMap::new(),
            active_connection_count: 0,
            latency_samples: Vec::new(),
            message_timestamps: Vec::new(),
        }
    }
}

/// Run monitor node (extracted for unified binary)
pub async fn run_monitor(listen_addr: String, port: u16, web_port: u16) -> Result<(), Box<dyn Error>> {
    println!("=== Network Monitor ===\n");
    println!("P2P Listen: {}:{}", listen_addr, port);
    println!("Web Server: http://localhost:{}\n", web_port);

    // Generate local key and PeerId
    let local_key = identity::Keypair::generate_ed25519();
    let local_peer_id = PeerId::from(local_key.public());
    println!("Bootstrap Peer ID: {}\n", local_peer_id);

    // Create shared network state
    let network_state = Arc::new(RwLock::new(NetworkState::new(local_peer_id)));
    let start_time = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();

    // Setup P2P network
    let transport = tcp::tokio::Transport::default()
        .upgrade(libp2p::core::upgrade::Version::V1)
        .authenticate(noise::Config::new(&local_key)?)
        .multiplex(yamux::Config::default())
        .boxed();

    // Kademlia DHT - Large timeout for reliable discovery
    let store = kad::store::MemoryStore::new(local_peer_id);
    let mut kademlia_config = kad::Config::default();
    kademlia_config.set_query_timeout(Duration::from_secs(120)); // Large timeout for reliable DHT operations
    let kademlia = kad::Behaviour::with_config(local_peer_id, store, kademlia_config);

    let identify = libp2p::identify::Behaviour::new(
        libp2p::identify::Config::new(
            "punch-simple-monitor/1.0.0".to_string(),
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
    // Monitor acts as a relay server to help peers behind NAT connect
    let relay = relay::Behaviour::new(
        local_peer_id,
        relay::Config::default(),
    );

    let behaviour = Behaviour { kademlia, identify, ping, relay };
    
    let swarm_config = SwarmConfig::with_tokio_executor()
        .with_idle_connection_timeout(Duration::from_secs(90)); // Increased since ping keeps connections alive
    let mut swarm = Swarm::new(
        transport,
        behaviour,
        local_peer_id,
        swarm_config,
    );

    let addr: Multiaddr = format!("/ip4/{}/tcp/{}", listen_addr, port).parse()?;
    swarm.listen_on(addr.clone())?;
    println!("[MONITOR] Listening on: {}", addr);

    // Start web server in background
    let web_state = network_state.clone();
    let web_port_clone = web_port;
    tokio::spawn(async move {
        if let Err(e) = start_web_server(web_port_clone, web_state, start_time).await {
            eprintln!("Web server error: {}", e);
        }
    });

    println!("✅ Monitor started!");
    println!("   P2P Network: {}:{}", listen_addr, port);
    println!("   Web Dashboard: http://localhost:{}", web_port);
    println!("\nPress Ctrl+C to stop.\n");

    // Main event loop - track network events
    loop {
        match swarm.select_next_some().await {
            SwarmEvent::ConnectionEstablished { peer_id, endpoint, .. } => {
                let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
                let mut state = network_state.write().await;
                
                let peer_id_str = peer_id.to_string();
                let direction = if endpoint.is_dialer() { "outbound" } else { "inbound" };
                
                // Update node info
                let is_new_node = !state.nodes.contains_key(&peer_id_str);
                state.nodes.entry(peer_id_str.clone()).and_modify(|node| {
                    node.last_seen = now;
                    node.connection_count += 1;
                }).or_insert_with(|| {
                    NodeInfo {
                        peer_id: peer_id_str.clone(),
                        first_seen: now,
                        last_seen: now,
                        connection_count: 1,
                        addresses: vec![],
                        agent: None,
                        protocol: None,
                        namespace: None,
                    }
                });
                
                // Add connection event
                state.connections.push(ConnectionEvent {
                    timestamp: now,
                    event_type: "connected".to_string(),
                    peer_id: peer_id_str.clone(),
                    direction: direction.to_string(),
                });
                
                // Update metrics
                state.metrics.total_connections += 1;
                state.active_connection_count += 1;
                state.metrics.active_connections = state.active_connection_count;
                if is_new_node {
                    state.metrics.total_nodes = state.nodes.len();
                }
                
                // Update namespace tracking
                // Since RecordKey is hashed, we can't extract namespace directly
                // For now, we'll infer from common test namespaces
                // TODO: Better solution - have nodes report namespace via custom protocol
                let node_count = state.nodes.len();
                if node_count > 0 {
                    // Check if this looks like the intensive test (all nodes in same namespace)
                    // We'll update this when we get more info from identify protocol
                    let test_namespace = "intensive-test";
                    state.namespaces.insert(test_namespace.to_string(), node_count);
                }
                
                println!("[MONITOR] ✓ Connection established: {} ({})", peer_id_str, direction);
            }
            SwarmEvent::ConnectionClosed { peer_id, .. } => {
                let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
                let mut state = network_state.write().await;
                
                let peer_id_str = peer_id.to_string();
                state.connections.push(ConnectionEvent {
                    timestamp: now,
                    event_type: "disconnected".to_string(),
                    peer_id: peer_id_str.clone(),
                    direction: "unknown".to_string(),
                });
                
                // Decrement active connection count
                if state.active_connection_count > 0 {
                    state.active_connection_count -= 1;
                }
                state.metrics.active_connections = state.active_connection_count;
                
                println!("[MONITOR] ✗ Connection closed: {}", peer_id_str);
            }
            SwarmEvent::NewListenAddr { address, .. } => {
                println!("[MONITOR] ✓ Now listening on: {}", address);
                swarm.add_external_address(address);
            }
            SwarmEvent::Dialing { peer_id, .. } => {
                if let Some(pid) = peer_id {
                    println!("[MONITOR] → Dialing peer: {}", pid);
                } else {
                    println!("[MONITOR] → Dialing unknown peer");
                }
            }
            SwarmEvent::Behaviour(behaviour_event) => {
                match behaviour_event {
                    BehaviourEvent::Kademlia(kad::Event::RoutingUpdated { .. }) => {
                        // DHT routing table updated - could query for records here
                    }
                    BehaviourEvent::Kademlia(kad::Event::OutboundQueryProgressed { result, .. }) => {
                        // Track DHT records to discover namespaces
                        match result {
                            kad::QueryResult::GetRecord(Ok(kad::GetRecordOk::FoundRecord(_record))) => {
                                let mut state = network_state.write().await;
                                state.metrics.dht_records += 1;
                                // Note: RecordKey is hashed, so we can't extract namespace directly
                                // We'll need to track namespaces differently
                            }
                            kad::QueryResult::PutRecord(Ok(_)) => {
                                // Record was stored
                                let mut state = network_state.write().await;
                                state.metrics.dht_records += 1;
                            }
                            _ => {}
                        }
                    }
                    BehaviourEvent::Identify(libp2p::identify::Event::Received { peer_id, info }) => {
                        let mut state = network_state.write().await;
                        let peer_id_str = peer_id.to_string();
                        
                        if let Some(node) = state.nodes.get_mut(&peer_id_str) {
                            node.agent = Some(info.agent_version.clone());
                            node.protocol = Some(info.protocol_version.clone());
                            if let Some(addr) = info.listen_addrs.first() {
                                node.addresses.push(addr.to_string());
                            }
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
}

async fn start_web_server(
    port: u16,
    state: Arc<RwLock<NetworkState>>,
    start_time: u64,
) -> Result<(), Box<dyn Error>> {
    use axum::{
        extract::ws::WebSocketUpgrade,
        response::{Html, Json},
        routing::get,
        Router,
    };
    
    // Broadcast channel for real-time updates
    let (tx, _rx) = broadcast::channel::<String>(100);
    let tx_clone = tx.clone();
    
    // Update metrics periodically
    let metrics_state = state.clone();
    let metrics_tx = tx.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(1));
        loop {
            interval.tick().await;
            let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
            let mut s = metrics_state.write().await;
            s.metrics.uptime_seconds = now - start_time;
            // Keep active_connections in sync
            s.metrics.active_connections = s.active_connection_count;
            s.metrics.total_nodes = s.nodes.len();
            
            // Calculate latency percentiles
            if !s.latency_samples.is_empty() {
                let mut sorted = s.latency_samples.clone();
                sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
                let len = sorted.len();
                s.metrics.latency_min_ms = sorted[0];
                s.metrics.latency_max_ms = sorted[len - 1];
                s.metrics.latency_avg_ms = sorted.iter().sum::<f64>() / len as f64;
                s.metrics.latency_p50_ms = sorted[len * 50 / 100];
                s.metrics.latency_p95_ms = sorted[len * 95 / 100];
                s.metrics.latency_p99_ms = sorted[len * 99 / 100];
            }
            
            // Calculate messages per second (last 10 seconds)
            let cutoff = now.saturating_sub(10);
            s.message_timestamps.retain(|&t| t >= cutoff);
            s.metrics.messages_per_second = s.message_timestamps.len() as f64 / 10.0;
            
            // Keep only last 1000 latency samples
            let sample_count = s.latency_samples.len();
            if sample_count > 1000 {
                s.latency_samples.drain(0..sample_count - 1000);
            }
            
            // Broadcast update
            if let Ok(json) = serde_json::to_string(&*s) {
                let _ = metrics_tx.send(json);
            }
        }
    });
    
    // Read HTML file
    let html_content = std::fs::read_to_string("web/index.html")
        .unwrap_or_else(|_| "<h1>Error: web/index.html not found</h1>".to_string());
    let html_content = Arc::new(html_content);
    let html_clone = html_content.clone();
    
    let app = Router::new()
        .route("/", get(move || {
            let html = html_clone.clone();
            async move { Html((*html).clone()) }
        }))
        .route("/api/state", get({
            let state = state.clone();
            move || async move {
                let s = state.read().await;
                Json(s.clone())
            }
        }))
        .route("/ws", get({
            let tx = tx_clone;
            move |ws: WebSocketUpgrade| async move {
                ws.on_upgrade(|socket| handle_socket(socket, tx))
            }
        }))
        .route("/api/metrics", get({
            let state = state.clone();
            move || async move {
                let s = state.read().await;
                Json(s.metrics.clone())
            }
        }))
        .route("/api/nodes", get({
            let state = state.clone();
            move || async move {
                let s = state.read().await;
                let nodes: Vec<_> = s.nodes.values().cloned().collect();
                Json(nodes)
            }
        }));
    
    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port)).await?;
    println!("Web server listening on http://0.0.0.0:{}", port);
    axum::serve(listener, app).await?;
    Ok(())
}

async fn handle_socket(socket: axum::extract::ws::WebSocket, tx: broadcast::Sender<String>) {
    use axum::extract::ws::Message;
    use futures_util::{SinkExt, StreamExt};
    
    let (mut sender, mut receiver) = socket.split();
    let mut rx = tx.subscribe();
    
    // Send updates to client
    let mut send_task = tokio::spawn(async move {
        while let Ok(msg) = rx.recv().await {
            if sender.send(Message::Text(msg)).await.is_err() {
                break;
            }
        }
    });
    
    // Receive messages from client (ping/pong) - just consume
    let mut recv_task = tokio::spawn(async move {
        while let Some(msg) = receiver.next().await {
            match msg {
                Ok(Message::Close(_)) => break,
                Err(_) => break,
                _ => {}
            }
        }
    });
    
    // Wait for either task to complete
    tokio::select! {
        _ = &mut send_task => {
            recv_task.abort();
        }
        _ = &mut recv_task => {
            send_task.abort();
        }
    };
}

#[allow(dead_code)]
#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();
    run_monitor(args.listen_addr, args.port, args.web_port).await
}

