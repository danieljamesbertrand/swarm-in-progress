//! Simple Kademlia Listener - Joins DHT and waits for connections
//! Usage: cargo run --bin listener [--bootstrap ADDR] [--namespace NAMESPACE]
//! 
//! Also available via unified node binary:
//!   cargo run --bin node -- listener --bootstrap ADDR --namespace NAMESPACE

use punch_simple::{JsonMessage, JsonCodec};
use punch_simple::metrics::{MetricsCodec, PeerMetrics, MetricsResponse};
use punch_simple::ai_inference_handler::{AIInferenceRequest, process_ai_inference};
use punch_simple::command_protocol::{Command, CommandResponse, commands};

use clap::Parser;
use serde_json;
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
use std::sync::Arc;
use tokio::sync::RwLock;
use rand::Rng;
use punch_simple::quic_transport::{create_transport, get_dual_listen_addresses, get_listen_address, TransportType};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct PeerDiscoveryRecord {
    peer_id: String,
    addrs: Vec<String>,
}

#[derive(Parser, Debug)]
#[command(name = "listener")]
#[command(about = "Simple Kademlia Listener - Joins DHT and waits for connections")]
struct Args {
    /// Bootstrap node address (Multiaddr format)
    #[arg(long, default_value = "/ip4/127.0.0.1/tcp/51820")]
    bootstrap: String,

    /// Namespace for peer discovery
    #[arg(long, default_value = "simple-chat")]
    namespace: String,

    /// Transport: quic|tcp|dual (default: dual)
    #[arg(long, default_value = "dual")]
    transport: TransportType,
}

#[derive(NetworkBehaviour)]
struct Behaviour {
    kademlia: kad::Behaviour<kad::store::MemoryStore>,
    identify: libp2p::identify::Behaviour,
    ping: ping::Behaviour,
    request_response: request_response::Behaviour<JsonCodec>,
    metrics_response: request_response::Behaviour<MetricsCodec>,
    relay: relay::Behaviour,
}

/// Run listener node with specified transport.
pub async fn run_listener_with_transport(
    bootstrap: String,
    namespace: String,
    transport_type: TransportType,
) -> Result<(), Box<dyn Error>> {
    println!("=== Simple Kademlia Listener ===\n");
    println!("Configuration:");
    println!("  Bootstrap: {}", bootstrap);
    println!("  Namespace: {}\n", namespace);

    let key = identity::Keypair::generate_ed25519();
    let peer_id = PeerId::from(key.public());
    println!("Peer ID: {}\n", peer_id);

    // Transport: QUIC/TCP selectable (default dual-stack)
    let transport = create_transport(&key, transport_type)?;
    
    // Kademlia DHT - Large timeout for reliable discovery
    let store = kad::store::MemoryStore::new(peer_id);
    let mut kademlia_config = kad::Config::default();
    kademlia_config.set_query_timeout(Duration::from_secs(120)); // Large timeout for reliable DHT operations
    let kademlia = kad::Behaviour::with_config(peer_id, store, kademlia_config);
    
    let bootstrap_addr: Multiaddr = bootstrap.parse()?;
    
    // Identify
    let identify = libp2p::identify::Behaviour::new(
        libp2p::identify::Config::new("simple-listener/1.0".to_string(), key.public())
    );
    
    // Ping protocol for connection keepalive (sends pings every 25 seconds)
    let ping = ping::Behaviour::new(
        ping::Config::new()
            .with_interval(Duration::from_secs(25)) // Ping every 25 seconds
            .with_timeout(Duration::from_secs(10)), // 10 second timeout
    );
    
    // Request-Response for JSON messaging using custom JSON codec
    let codec = JsonCodec;
    let request_response = request_response::Behaviour::with_codec(
        codec,
        [(StreamProtocol::new("/json-message/1.0"), ProtocolSupport::Full)],
        request_response::Config::default(),
    );
    
    // Request-Response for metrics reporting
    let metrics_codec = MetricsCodec;
    let metrics_response = request_response::Behaviour::with_codec(
        metrics_codec,
        [(StreamProtocol::new("/metrics/1.0"), ProtocolSupport::Full)],
        request_response::Config::default(),
    );
    
    // Relay protocol for NAT traversal (client mode)
    let relay = relay::Behaviour::new(
        peer_id,
        relay::Config::default(),
    );
    
    let behaviour = Behaviour { kademlia, identify, ping, request_response, metrics_response, relay };
    
    // Swarm - Increased idle timeout since ping keeps connections alive
    let swarm_config = SwarmConfig::with_tokio_executor()
        .with_idle_connection_timeout(Duration::from_secs(90));
    let mut swarm = Swarm::new(
        transport,
        behaviour,
        peer_id,
        swarm_config,
    );

    // Bootstrap to DHT
    println!("Bootstrapping to DHT via: {}\n", bootstrap);
    
    // Listen on requested transport(s)
    match transport_type {
        TransportType::DualStack => {
            let (quic, tcp) = get_dual_listen_addresses(0);
            swarm.listen_on(quic.parse()?)?;
            swarm.listen_on(tcp.parse()?)?;
        }
        other => {
            swarm.listen_on(get_listen_address(other, 0).parse()?)?;
        }
    }
    
    // Connect to bootstrap node
    swarm.dial(bootstrap_addr.clone())?;
    
    let mut bootstrapped = false;
    let mut registered = false;
    let mut connected_peers: HashMap<PeerId, ()> = HashMap::new();
    let mut message_counter = 0u32;
    let mut listen_addrs: Vec<Multiaddr> = Vec::new();
    
    // Metrics tracking
    let metrics = Arc::new(RwLock::new(PeerMetrics {
        peer_id: peer_id.to_string(),
        namespace: namespace.clone(),
        messages_sent: 0,
        messages_received: 0,
        latency_samples: Vec::new(),
        bytes_sent: 0,
        bytes_received: 0,
        message_errors: 0,
        timeout_errors: 0,
    }));
    
    // Track monitor peer ID (bootstrap node)
    let _monitor_peer_id = peer_id; // Will be updated when we connect to bootstrap
    
    // Create a channel for random message sending
    let (msg_tx, mut msg_rx) = tokio::sync::mpsc::unbounded_channel::<()>();
    let msg_tx_clone = msg_tx.clone();
    tokio::spawn(async move {
        loop {
            // Random interval between 100ms and 2000ms (0.1-2 seconds)
            let delay_ms = rand::thread_rng().gen_range(100..=2000);
            tokio::time::sleep(Duration::from_millis(delay_ms)).await;
            // Send random number of messages (1-5)
            let msg_count = rand::thread_rng().gen_range(1..=5);
            for _ in 0..msg_count {
                let _ = msg_tx_clone.send(());
            }
        }
    });
    
    loop {
        tokio::select! {
            // Handle swarm events
            event = swarm.select_next_some() => {
                match event {
            SwarmEvent::NewListenAddr { address, .. } => {
                println!("[VERBOSE] Listening on: {}", address);
                swarm.add_external_address(address.clone());
                listen_addrs.push(address);
            }
            SwarmEvent::Dialing { .. } => {
                println!("[VERBOSE] → Dialing...");
            }
            SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                println!("[VERBOSE] ✓ Connection established");
                println!("[VERBOSE]   Peer ID: {}", peer_id);
                
                if !bootstrapped {
                    // First connection is to bootstrap node (monitor)
                    // Start bootstrap after first connection
                    if let Err(e) = swarm.behaviour_mut().kademlia.bootstrap() {
                        eprintln!("[WARN] Bootstrap start failed: {:?}", e);
                    } else {
                        println!("✓ Started Kademlia bootstrap!");
                    }
                } else if !registered {
                    // Register our peer info in DHT
                    let key = kad::RecordKey::new(&namespace);
                    let record_value = PeerDiscoveryRecord {
                        peer_id: swarm.local_peer_id().to_string(),
                        addrs: listen_addrs.iter().map(|a| a.to_string()).collect(),
                    };
                    let value = serde_json::to_vec(&record_value).unwrap_or_else(|_| peer_id.to_bytes());
                    let record = kad::Record::new(key.clone(), value);
                    if let Err(e) = swarm.behaviour_mut().kademlia.put_record(record, kad::Quorum::One) {
                        eprintln!("[ERROR] Failed to put record: {:?}", e);
                    } else {
                        println!("✓ Registered in DHT! Waiting for connections...\n");
                        println!("Your Peer ID: {}", peer_id);
                        registered = true;
                    }
                } else if peer_id != *swarm.local_peer_id() && !connected_peers.contains_key(&peer_id) {
                    // This is a peer connection
                    println!("✓✓✓ Peer connected: {}", peer_id);
                    connected_peers.insert(peer_id, ());
                    println!("[MESSAGE] Ready to exchange JSON messages with peer {}", peer_id);
                }
            }
            SwarmEvent::ConnectionClosed { peer_id, cause, .. } => {
                println!("[VERBOSE] ✗ Connection closed");
                println!("[VERBOSE]   Peer: {}", peer_id);
                println!("[VERBOSE]   Cause: {:?}", cause);
                
                connected_peers.remove(&peer_id);
            }
            SwarmEvent::Behaviour(event) => {
                match event {
                    BehaviourEvent::Kademlia(kad::Event::RoutingUpdated { .. }) => {
                        if !bootstrapped {
                            bootstrapped = true;
                            println!("✓ DHT bootstrapped!");
                            // After bootstrapping, query for peers to discover them
                            if registered {
                                let key = kad::RecordKey::new(&namespace);
                                let local_peer_id = *swarm.local_peer_id();
                                swarm.behaviour_mut().kademlia.get_record(key);
                                swarm.behaviour_mut().kademlia.get_closest_peers(local_peer_id);
                            }
                        }
                    }
                    BehaviourEvent::Kademlia(kad::Event::OutboundQueryProgressed { result, .. }) => {
                        match result {
                            kad::QueryResult::Bootstrap(Ok(kad::BootstrapOk { .. })) => {
                                if !bootstrapped {
                                    bootstrapped = true;
                                    println!("✓ DHT bootstrapped!");
                                }
                            }
                            kad::QueryResult::GetClosestPeers(Ok(ok)) => {
                                if ok.peers.len() > 0 {
                                    println!("[VERBOSE] ✓ Found {} peer(s) in DHT", ok.peers.len());
                                    for discovered_peer in ok.peers {
                                        if !connected_peers.contains_key(&discovered_peer) && discovered_peer != *swarm.local_peer_id() {
                                            println!("[VERBOSE]   Discovered peer: {}", discovered_peer);
                                            // Kademlia will automatically maintain connections to closest peers
                                            // We can also query for their addresses if needed
                                            let key = kad::RecordKey::new(&namespace);
                                            swarm.behaviour_mut().kademlia.get_record(key);
                                        }
                                    }
                                }
                            }
                            kad::QueryResult::GetRecord(Ok(kad::GetRecordOk::FoundRecord(_record))) => {
                                println!("[VERBOSE] ✓ Found record in DHT");
                            }
                            _ => {}
                        }
                    }
                    BehaviourEvent::Kademlia(e) => {
                        println!("[VERBOSE] [Kademlia Event] {:?}", e);
                    }
                    BehaviourEvent::Identify(libp2p::identify::Event::Received { peer_id, info }) => {
                        println!("[VERBOSE] [Identify] Received from peer: {}", peer_id);
                        println!("[VERBOSE]   Protocol: {:?}", info.protocol_version);
                        println!("[VERBOSE]   Agent: {:?}", info.agent_version);
                    }
                    BehaviourEvent::RequestResponse(request_response::Event::Message { message, .. }) => {
                        match message {
                            request_response::Message::Request { request, channel, .. } => {
                                // Calculate latency if send_time_ms is present
                                let latency_ms = if let Some(send_time) = request.send_time_ms {
                                    let now_ms = std::time::SystemTime::now()
                                        .duration_since(std::time::UNIX_EPOCH)
                                        .unwrap()
                                        .as_millis() as u64;
                                    if now_ms > send_time {
                                        (now_ms - send_time) as f64
                                    } else {
                                        0.0
                                    }
                                } else {
                                    0.0
                                };
                                
                                // Update metrics
                                {
                                    let mut m = metrics.write().await;
                                    m.messages_received += 1;
                                    if latency_ms > 0.0 {
                                        m.latency_samples.push(latency_ms);
                                        // Keep only last 100 samples
                                        if m.latency_samples.len() > 100 {
                                            m.latency_samples.remove(0);
                                        }
                                    }
                                    if let Ok(json_bytes) = serde_json::to_vec(&request) {
                                        m.bytes_received += json_bytes.len() as u64;
                                    }
                                }
                                
                                println!("\n[📨 RECEIVED JSON MESSAGE] (latency: {:.2}ms)", latency_ms);
                                println!("  From: {}", request.from);
                                println!("  Message: {}", request.message);
                                println!("  Timestamp: {}", request.timestamp);

                                // Build exactly one response and send exactly once.
                                // (Prevents accidental "use of moved value: channel" regressions.)
                                let response_msg = if let Ok(cmd) = Command::from_json(&request.message) {
                                    if cmd.command == commands::EXECUTE_TASK {
                                        if let Ok(ai_req) = AIInferenceRequest::from_command(&cmd) {
                                            let resp = match process_ai_inference(&ai_req).await {
                                                Ok(result) => {
                                                    let mut response_data = HashMap::new();
                                                    if let Some(output) = result.get("output") {
                                                        response_data.insert("output".to_string(), output.clone());
                                                    }
                                                    if let Some(model) = result.get("model") {
                                                        response_data.insert("model".to_string(), model.clone());
                                                    }
                                                    CommandResponse::success(
                                                        &cmd.command,
                                                        &cmd.request_id,
                                                        &swarm.local_peer_id().to_string(),
                                                        &cmd.from,
                                                        response_data,
                                                    )
                                                }
                                                Err(e) => CommandResponse::error(
                                                    &cmd.command,
                                                    &cmd.request_id,
                                                    &swarm.local_peer_id().to_string(),
                                                    &cmd.from,
                                                    &e,
                                                ),
                                            };

                                            JsonMessage::new(
                                                swarm.local_peer_id().to_string(),
                                                resp.to_json().unwrap_or_default(),
                                            )
                                        } else {
                                            JsonMessage::new(
                                                format!(
                                                    "listener-{}",
                                                    peer_id.to_string().chars().take(8).collect::<String>()
                                                ),
                                                format!("Echo: {}", request.message),
                                            )
                                        }
                                    } else {
                                        JsonMessage::new(
                                            format!(
                                                "listener-{}",
                                                peer_id.to_string().chars().take(8).collect::<String>()
                                            ),
                                            format!("Echo: {}", request.message),
                                        )
                                    }
                                } else {
                                    JsonMessage::new(
                                        format!(
                                            "listener-{}",
                                            peer_id.to_string().chars().take(8).collect::<String>()
                                        ),
                                        format!("Echo: {}", request.message),
                                    )
                                };

                                // Update metrics for outgoing response
                                {
                                    let mut m = metrics.write().await;
                                    m.messages_sent += 1;
                                    if let Ok(json_bytes) = serde_json::to_vec(&response_msg) {
                                        m.bytes_sent += json_bytes.len() as u64;
                                    }
                                }

                                // (clone is only for logging after send)
                                let response_msg_for_log = response_msg.clone();
                                let send_result = swarm
                                    .behaviour_mut()
                                    .request_response
                                    .send_response(channel, response_msg);

                                if let Err(e) = send_result {
                                    eprintln!("[ERROR] Failed to send response: {:?}", e);
                                    let mut m = metrics.write().await;
                                    m.message_errors += 1;
                                } else {
                                    println!("\n[📤 SENT JSON RESPONSE]");
                                    println!("  From: {}", response_msg_for_log.from);
                                    println!("  Message: {}", response_msg_for_log.message);
                                    println!("  Timestamp: {}", response_msg_for_log.timestamp);
                                }
                            }
                            request_response::Message::Response { response, .. } => {
                                // Update metrics
                                {
                                    let mut m = metrics.write().await;
                                    m.messages_received += 1;
                                    if let Ok(json_bytes) = serde_json::to_vec(&response) {
                                        m.bytes_received += json_bytes.len() as u64;
                                    }
                                }
                                
                                // Received a response to our request
                                println!("\n[📥 RECEIVED JSON RESPONSE]");
                                println!("  From: {}", response.from);
                                println!("  Message: {}", response.message);
                                println!("  Timestamp: {}", response.timestamp);
                                // Show full JSON
                                if let Ok(json_str) = serde_json::to_string_pretty(&response) {
                                    println!("  Full JSON:\n{}", json_str);
                                }
                            }
                        }
                    }
                    BehaviourEvent::MetricsResponse(request_response::Event::Message { message, .. }) => {
                        match message {
                            request_response::Message::Request { channel, .. } => {
                                // Monitor is requesting metrics
                                let m = metrics.read().await;
                                let peer_metrics = PeerMetrics {
                                    peer_id: m.peer_id.clone(),
                                    namespace: m.namespace.clone(),
                                    messages_sent: m.messages_sent,
                                    messages_received: m.messages_received,
                                    latency_samples: m.latency_samples.clone(),
                                    bytes_sent: m.bytes_sent,
                                    bytes_received: m.bytes_received,
                                    message_errors: m.message_errors,
                                    timeout_errors: m.timeout_errors,
                                };
                                drop(m);
                                
                                let response = MetricsResponse {
                                    success: true,
                                    message: "Metrics sent".to_string(),
                                    metrics: Some(peer_metrics),
                                };
                                
                                if let Err(e) = swarm.behaviour_mut().metrics_response.send_response(channel, response) {
                                    eprintln!("[ERROR] Failed to send metrics: {:?}", e);
                                }
                            }
                            _ => {}
                        }
                    }
                    BehaviourEvent::MetricsResponse(request_response::Event::OutboundFailure { error, .. }) => {
                        eprintln!("[ERROR] Metrics request failed: {:?}", error);
                        let mut m = metrics.write().await;
                        m.timeout_errors += 1;
                    }
                    BehaviourEvent::MetricsResponse(request_response::Event::InboundFailure { error, .. }) => {
                        eprintln!("[ERROR] Metrics inbound failed: {:?}", error);
                    }
                    BehaviourEvent::RequestResponse(request_response::Event::OutboundFailure { error, .. }) => {
                        eprintln!("[ERROR] Outbound request failed: {:?}", error);
                        let mut m = metrics.write().await;
                        m.message_errors += 1;
                    }
                    BehaviourEvent::RequestResponse(request_response::Event::InboundFailure { error, .. }) => {
                        eprintln!("[ERROR] Inbound request failed: {:?}", error);
                        let mut m = metrics.write().await;
                        m.message_errors += 1;
                    }
                    _ => {}
                }
            }
            SwarmEvent::OutgoingConnectionError { error, peer_id, .. } => {
                println!("[VERBOSE] ✗ Outgoing connection error");
                println!("[VERBOSE]   Peer: {:?}", peer_id);
                println!("[VERBOSE]   Error: {:?}", error);
            }
            SwarmEvent::IncomingConnectionError { error, .. } => {
                println!("[VERBOSE] ✗ Incoming connection error: {:?}", error);
            }
            _ => {}
                }
            },
            _ = msg_rx.recv() => {
                // Send random messages to random connected peers
                if !connected_peers.is_empty() {
                    let mut rng = rand::thread_rng();
                    message_counter += 1;
                    
                    // Pick random peer(s) to send to
                    let peer_ids: Vec<_> = connected_peers.keys().cloned().collect();
                    let target_count = rng.gen_range(1..=peer_ids.len().min(3)); // Send to 1-3 random peers
                    let local_peer_id_str = swarm.local_peer_id().to_string();
                    
                    for _ in 0..target_count {
                        if let Some(peer_id) = peer_ids.get(rng.gen_range(0..peer_ids.len())) {
                            let msg_texts = vec![
                                format!("Load test message #{}", message_counter),
                                format!("Random payload {}", rng.gen_range(1000..9999)),
                                format!("Test data: {}", rng.gen_range(0..100)),
                                format!("Message batch {}", message_counter),
                            ];
                            let msg_text = msg_texts[rng.gen_range(0..msg_texts.len())].clone();
                            
                            let json_msg = JsonMessage::new(
                                format!("listener-{}", local_peer_id_str.chars().take(8).collect::<String>()),
                                msg_text,
                            );
                            
                            // Update metrics
                            {
                                let mut m = metrics.write().await;
                                m.messages_sent += 1;
                                if let Ok(json_bytes) = serde_json::to_vec(&json_msg) {
                                    m.bytes_sent += json_bytes.len() as u64;
                                }
                            }
                            
                            let _request_id = swarm.behaviour_mut().request_response.send_request(peer_id, json_msg.clone());
                            println!("\n[📤 SENT RANDOM MESSAGE] to peer {} (#{})", peer_id, message_counter);
                        }
                    }
                }
            }
        }
    }
}

/// Run listener node (extracted for unified binary).
///
/// Backwards-compatible wrapper that defaults to dual-stack transport.
pub async fn run_listener(bootstrap: String, namespace: String) -> Result<(), Box<dyn Error>> {
    run_listener_with_transport(bootstrap, namespace, TransportType::DualStack).await
}

#[allow(dead_code)]
#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();
    run_listener_with_transport(args.bootstrap, args.namespace, args.transport).await
}
