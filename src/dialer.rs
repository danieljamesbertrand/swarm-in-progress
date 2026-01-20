//! Simple Kademlia Dialer - Discovers peers via DHT and connects
//! Usage: cargo run --bin dialer [--bootstrap ADDR] [--namespace NAMESPACE]
//! 
//! Also available via unified node binary:
//!   cargo run --bin node -- dialer --bootstrap ADDR --namespace NAMESPACE

use punch_simple::{JsonMessage, JsonCodec};
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
use rand::Rng;
use punch_simple::quic_transport::{create_transport, get_dual_listen_addresses, get_listen_address, TransportType};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct PeerDiscoveryRecord {
    peer_id: String,
    addrs: Vec<String>,
}

#[derive(Parser, Debug)]
#[command(name = "dialer")]
#[command(about = "Simple Kademlia Dialer - Discovers peers via DHT and connects")]
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

    /// Send one AI question (EXECUTE_TASK/ai_inference) once a peer is found.
    /// Example: --ask "Why is the sky blue?"
    #[arg(long)]
    ask: Option<String>,
}

#[derive(NetworkBehaviour)]
struct Behaviour {
    kademlia: kad::Behaviour<kad::store::MemoryStore>,
    identify: libp2p::identify::Behaviour,
    ping: ping::Behaviour,
    request_response: request_response::Behaviour<JsonCodec>,
    relay: relay::Behaviour,
}

/// Run dialer node with a specified transport and optional question.
pub async fn run_dialer_with_transport(
    bootstrap: String,
    namespace: String,
    transport_type: TransportType,
    ask: Option<String>,
) -> Result<(), Box<dyn Error>> {
    println!("=== Simple Kademlia Dialer ===\n");
    println!("Configuration:");
    println!("  Bootstrap: {}", bootstrap);
    println!("  Namespace: {}\n", namespace);

    let key = identity::Keypair::generate_ed25519();
    let peer_id = PeerId::from(key.public());
    println!("Local Peer ID: {}\n", peer_id);

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
        libp2p::identify::Config::new("simple-dialer/1.0".to_string(), key.public())
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
    
    // Relay protocol for NAT traversal (client mode)
    let relay = relay::Behaviour::new(
        peer_id,
        relay::Config::default(),
    );
    
    let behaviour = Behaviour { kademlia, identify, ping, request_response, relay };
    
    // Swarm - Increased idle timeout since ping keeps connections alive
    let swarm_config = SwarmConfig::with_tokio_executor()
        .with_idle_connection_timeout(Duration::from_secs(90));
    let mut swarm = Swarm::new(
        transport,
        behaviour,
        peer_id,
        swarm_config,
    );

    // Bootstrap to DHT: listen on requested transport(s)
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
    println!("[1] Bootstrapping to DHT via: {}\n", bootstrap);
    
    // Connect to bootstrap node
    swarm.dial(bootstrap_addr.clone())?;
    
    let mut bootstrapped = false;
    let mut discovered_peers: Vec<PeerId> = Vec::new();
    let mut connected_peers: HashMap<PeerId, ()> = HashMap::new();
    let mut message_counter = 0u32;
    let mut pending_ai_request: Option<String> = None; // request_id
    
    // Start bootstrap process
    println!("[VERBOSE] Starting Kademlia bootstrap...");
    
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
            }
            SwarmEvent::Dialing { .. } => {
                println!("[VERBOSE] → Dialing...");
            }
            SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                println!("[VERBOSE] ✓ Connection established");
                println!("[VERBOSE]   Peer ID: {}", peer_id);
                
                if !bootstrapped {
                    // Start bootstrap after first connection
                    if let Err(e) = swarm.behaviour_mut().kademlia.bootstrap() {
                        eprintln!("[WARN] Bootstrap start failed: {:?}", e);
                    } else {
                        println!("✓ Started Kademlia bootstrap!");
                    }
                } else if !discovered_peers.contains(&peer_id) && peer_id != *swarm.local_peer_id() {
                    println!("[VERBOSE] ✓✓✓ Connected to discovered peer: {}", peer_id);
                    println!("✓✓✓ CONNECTED to peer: {}", peer_id);
                    connected_peers.insert(peer_id, ());
                    
                    // Send initial JSON message
                    message_counter += 1;
                    let json_msg = JsonMessage::new(
                        format!("dialer-{}", peer_id.to_string().chars().take(8).collect::<String>()),
                        format!("Hello from dialer! Message #{}", message_counter),
                    );
                    let _request_id = swarm.behaviour_mut().request_response.send_request(&peer_id, json_msg.clone());
                    println!("\n[📤 SENT JSON MESSAGE] to peer {}", peer_id);
                    println!("  From: {}", json_msg.from);
                    println!("  Message: {}", json_msg.message);
                    println!("  Timestamp: {}", json_msg.timestamp);

                    // If user asked a question, send an AI inference command (once).
                    if pending_ai_request.is_none() {
                        if let Some(ref question) = ask {
                            let mut cmd = Command::new(
                                commands::EXECUTE_TASK,
                                &swarm.local_peer_id().to_string(),
                                Some(&peer_id.to_string()),
                            );
                            cmd.params
                                .insert("task_type".to_string(), serde_json::json!("ai_inference"));
                            cmd.params
                                .insert("model_name".to_string(), serde_json::json!("mock"));
                            cmd.params
                                .insert("input_data".to_string(), serde_json::json!(question));

                            pending_ai_request = Some(cmd.request_id.clone());
                            let msg = JsonMessage::new(
                                swarm.local_peer_id().to_string(),
                                cmd.to_json().unwrap_or_else(|_| "{}".to_string()),
                            );
                            swarm.behaviour_mut()
                                .request_response
                                .send_request(&peer_id, msg);
                            println!("[AI] Sent question with request_id={}", pending_ai_request.as_ref().unwrap());
                        }
                    }
                }
            }
            SwarmEvent::ConnectionClosed { peer_id, cause, .. } => {
                println!("[VERBOSE] ✗ Connection closed");
                println!("[VERBOSE]   Peer: {}", peer_id);
                println!("[VERBOSE]   Cause: {:?}", cause);
                
                connected_peers.remove(&peer_id);
                discovered_peers.retain(|&p| p != peer_id);
            }
            SwarmEvent::Behaviour(event) => {
                match event {
                    BehaviourEvent::Kademlia(kad::Event::RoutingUpdated { .. }) => {
                        if !bootstrapped {
                            bootstrapped = true;
                            println!("✓ DHT bootstrapped! Discovering peers...");
                            
                            // Store our peer info in DHT
                            let key = kad::RecordKey::new(&namespace);
                            let record_value = PeerDiscoveryRecord {
                                peer_id: peer_id.to_string(),
                                addrs: Vec::new(),
                            };
                            let value = serde_json::to_vec(&record_value).unwrap_or_else(|_| peer_id.to_bytes());
                            let record = kad::Record::new(key.clone(), value);
                            if let Err(e) = swarm.behaviour_mut().kademlia.put_record(record, kad::Quorum::One) {
                                eprintln!("[WARN] Failed to put record: {:?}", e);
                            }
                            
                            // Query for peers in namespace
                            swarm.behaviour_mut().kademlia.get_record(key);
                            swarm.behaviour_mut().kademlia.get_closest_peers(peer_id);
                        }
                    }
                    BehaviourEvent::Kademlia(kad::Event::OutboundQueryProgressed { result, .. }) => {
                        match result {
                            kad::QueryResult::GetClosestPeers(Ok(ok)) => {
                                println!("[VERBOSE] ✓ Found {} peer(s) in DHT", ok.peers.len());
                                for discovered_peer in ok.peers {
                                    if !discovered_peers.contains(&discovered_peer) && discovered_peer != peer_id {
                                        discovered_peers.push(discovered_peer);
                                        println!("[VERBOSE]   Found peer: {}", discovered_peer);
                                        // Kademlia will handle connection automatically
                                    }
                                }
                            }
                            kad::QueryResult::GetRecord(Ok(kad::GetRecordOk::FoundRecord(_record))) => {
                                println!("[VERBOSE] ✓ Found record in DHT");
                                // Record contains peer info - connection will be established automatically
                            }
                            kad::QueryResult::Bootstrap(Ok(kad::BootstrapOk { .. })) => {
                                if !bootstrapped {
                                    bootstrapped = true;
                                    println!("✓ DHT bootstrapped!");
                                }
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
                                
                                println!("\n[📨 RECEIVED JSON MESSAGE] (latency: {:.2}ms)", latency_ms);
                                println!("  From: {}", request.from);
                                println!("  Message: {}", request.message);
                                println!("  Timestamp: {}", request.timestamp);
                                
                                // Send a response with latency info
                                message_counter += 1;
                                let response_msg = JsonMessage::new(
                                    format!("dialer-{}", peer_id.to_string().chars().take(8).collect::<String>()),
                                    format!("Echo from dialer: {} (latency: {:.2}ms, response #{})", request.message, latency_ms, message_counter),
                                );
                                
                                if let Err(e) = swarm.behaviour_mut().request_response.send_response(channel, response_msg.clone()) {
                                    eprintln!("[ERROR] Failed to send response: {:?}", e);
                                } else {
                                    println!("\n[📤 SENT JSON RESPONSE]");
                                    println!("  From: {}", response_msg.from);
                                    println!("  Message: {}", response_msg.message);
                                    println!("  Timestamp: {}", response_msg.timestamp);
                                }
                            }
                            request_response::Message::Response { response, .. } => {
                                // Received a response to our request (already deserialized)
                                println!("\n[📥 RECEIVED JSON RESPONSE]");
                                println!("  From: {}", response.from);
                                println!("  Message: {}", response.message);
                                println!("  Timestamp: {}", response.timestamp);
                                // Show full JSON
                                if let Ok(json_str) = serde_json::to_string_pretty(&response) {
                                    println!("  Full JSON:\n{}", json_str);
                                }

                                // If this looks like a CommandResponse, print AI output and verify request_id match.
                                if let Ok(cmd_resp) = CommandResponse::from_json(&response.message) {
                                    if let Some(ref pending) = pending_ai_request {
                                        if &cmd_resp.request_id == pending {
                                            if let Some(result) = cmd_resp.result {
                                                if let Some(output) = result.get("output").and_then(|v| v.as_str()) {
                                                    println!("\n[AI ANSWER]\n{}\n", output);
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    BehaviourEvent::RequestResponse(request_response::Event::OutboundFailure { error, .. }) => {
                        eprintln!("[ERROR] Outbound request failed: {:?}", error);
                    }
                    BehaviourEvent::RequestResponse(request_response::Event::InboundFailure { error, .. }) => {
                        eprintln!("[ERROR] Inbound request failed: {:?}", error);
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
            }
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
                                format!("dialer-{}", local_peer_id_str.chars().take(8).collect::<String>()),
                                msg_text,
                            );
                            let _request_id = swarm.behaviour_mut().request_response.send_request(peer_id, json_msg.clone());
                            println!("\n[📤 SENT RANDOM MESSAGE] to peer {} (#{})", peer_id, message_counter);
                        }
                    }
                }
            }
        }
    }
}

/// Run dialer node (extracted for unified binary).
///
/// Backwards-compatible wrapper that defaults to dual-stack transport.
pub async fn run_dialer(bootstrap: String, namespace: String) -> Result<(), Box<dyn Error>> {
    run_dialer_with_transport(bootstrap, namespace, TransportType::DualStack, None).await
}

#[allow(dead_code)]
#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();
    run_dialer_with_transport(args.bootstrap, args.namespace, args.transport, args.ask).await
}
