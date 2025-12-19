//! Simple Rendezvous Listener - Registers with rndz server and waits for connections
//! Usage: cargo run --bin listener [--server HOST] [--port PORT] [--namespace NAMESPACE]

mod message;
use message::{JsonMessage, JsonCodec};

use clap::Parser;
use serde_json;
use libp2p::{
    identity,
    tcp,
    noise,
    yamux,
    rendezvous,
    request_response::{self, ProtocolSupport},
    swarm::{NetworkBehaviour, Swarm, SwarmEvent},
    core::transport::Transport,
    PeerId, Multiaddr, StreamProtocol,
};
use libp2p::swarm::Config as SwarmConfig;
use libp2p::futures::StreamExt;
use std::error::Error;
use std::time::Duration;
use tokio::time::sleep;
use std::collections::HashMap;

#[derive(Parser, Debug)]
#[command(name = "listener")]
#[command(about = "Simple Rendezvous Listener - Registers with rndz server and waits for connections")]
struct Args {
    /// Rendezvous server hostname or IP address
    #[arg(long, default_value = "162.221.207.169")]
    server: String,

    /// Rendezvous server port
    #[arg(long, default_value = "51820")]
    port: u16,

    /// Namespace for peer registration
    #[arg(long, default_value = "simple-chat")]
    namespace: String,
}

#[derive(NetworkBehaviour)]
struct Behaviour {
    rendezvous: rendezvous::client::Behaviour,
    identify: libp2p::identify::Behaviour,
    request_response: request_response::Behaviour<JsonCodec>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();
    
    println!("=== Simple Rendezvous Listener ===\n");
    println!("Configuration:");
    println!("  Server: {}:{}", args.server, args.port);
    println!("  Namespace: {}\n", args.namespace);

    let key = identity::Keypair::generate_ed25519();
    let peer_id = PeerId::from(key.public());
    println!("Peer ID: {}\n", peer_id);

    // TCP transport with noise encryption and yamux multiplexing
    let transport = tcp::tokio::Transport::default()
        .upgrade(libp2p::core::upgrade::Version::V1)
        .authenticate(noise::Config::new(&key)?)
        .multiplex(yamux::Config::default())
        .boxed();
    
    // Rendezvous client
    let rendezvous = rendezvous::client::Behaviour::new(key.clone());
    
    // Identify
    let identify = libp2p::identify::Behaviour::new(
        libp2p::identify::Config::new("simple-listener/1.0".to_string(), key.public())
    );
    
    // Request-Response for JSON messaging using custom JSON codec
    let codec = JsonCodec;
    let request_response = request_response::Behaviour::with_codec(
        codec,
        [(StreamProtocol::new("/json-message/1.0"), ProtocolSupport::Full)],
        request_response::Config::default(),
    );
    
    let behaviour = Behaviour { rendezvous, identify, request_response };
    
    // Swarm
    let swarm_config = SwarmConfig::with_tokio_executor()
        .with_idle_connection_timeout(Duration::from_secs(60));
    let mut swarm = Swarm::new(
        transport,
        behaviour,
        peer_id,
        swarm_config,
    );

    // Connect to rendezvous server
    let addr: Multiaddr = format!("/ip4/{}/tcp/{}", args.server, args.port).parse()?;
    println!("Connecting to rendezvous server: {}:{}\n", args.server, args.port);
    println!("[VERBOSE] Make sure the rendezvous server is running!");
    println!("[VERBOSE] IMPORTANT: Server should listen on 0.0.0.0 (all interfaces), not the specific IP!");
    println!("[VERBOSE] Correct server command:");
    println!("[VERBOSE]   ~/.cargo/bin/rndz server --listen-addr 0.0.0.0:51820");
    println!("[VERBOSE] NOT: ~/.cargo/bin/rndz server --listen-addr {}:{}\n", args.server, args.port);
    
    // Listen on all interfaces
    swarm.listen_on("/ip4/0.0.0.0/tcp/0".parse()?)?;
    
    // Wait for listen address and add it as external address
    // This is needed for rendezvous registration
    let mut listen_addr_ready = false;
    
    let mut registered = false;
    let mut rendezvous_peer_id: Option<PeerId> = None;
    let mut connection_retry_count = 0;
    let mut connected_peers: HashMap<PeerId, ()> = HashMap::new();
    let mut message_counter = 0u32;
    const MAX_RETRIES: u32 = 5;
    const INITIAL_RETRY_DELAY: u64 = 2; // seconds
    
    // Initial connection attempt
    println!("[VERBOSE] Attempting initial connection...");
    swarm.dial(addr.clone())?;
    
    // Create a channel for retry signals
    let (retry_tx, mut retry_rx) = tokio::sync::mpsc::unbounded_channel::<()>();
    
    // Create a channel for periodic message sending
    let (msg_tx, mut msg_rx) = tokio::sync::mpsc::unbounded_channel::<()>();
    let msg_tx_clone = msg_tx.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_millis(100)); // Send every 100ms instead of 5 seconds
        loop {
            interval.tick().await;
            // Send multiple triggers to send batches of messages
            for _ in 0..10 {
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
                // Add as external address for rendezvous registration
                swarm.add_external_address(address.clone());
                // Also add 127.0.0.1 version for local connections (Windows <-> Windows via WSL server)
                let addr_str = address.to_string();
                if addr_str.contains("/ip4/0.0.0.0/") {
                    let localhost_addr_str = addr_str.replace("/ip4/0.0.0.0/", "/ip4/127.0.0.1/");
                    if let Ok(localhost_addr) = localhost_addr_str.parse::<Multiaddr>() {
                        println!("[VERBOSE] Also adding localhost address: {}", localhost_addr);
                        swarm.add_external_address(localhost_addr);
                    }
                }
                listen_addr_ready = true;
            }
            SwarmEvent::Dialing { .. } => {
                println!("[VERBOSE] → Dialing...");
            }
            SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                println!("[VERBOSE] ✓ Connection established");
                println!("[VERBOSE]   Peer ID: {}", peer_id);
                
                if !registered && listen_addr_ready {
                    rendezvous_peer_id = Some(peer_id);
                    connection_retry_count = 0; // Reset retry count on success
                    println!("✓ Connected to rendezvous server!");
                    // Register with rendezvous server
                    let namespace = rendezvous::Namespace::new(args.namespace.clone())?;
                    println!("\n[1] Registering with rendezvous server...");
                    println!("    Namespace: {}", args.namespace);
                    match swarm.behaviour_mut().rendezvous.register(
                        namespace,
                        peer_id,
                        None, // Use default TTL (7200 seconds)
                    ) {
                        Ok(()) => {
                            println!("[VERBOSE] Registration request sent");
                        }
                        Err(e) => {
                            eprintln!("[ERROR] Failed to send registration: {:?}", e);
                            if e.to_string().contains("NoExternalAddresses") {
                                println!("[INFO] Waiting for external address...");
                            }
                        }
                    }
                } else if !listen_addr_ready {
                    println!("[VERBOSE] Waiting for listen address before registering...");
                } else if Some(peer_id) != rendezvous_peer_id {
                    // This is a peer connection (not the rendezvous server)
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
                
                // If rendezvous server connection closed, try to reconnect
                if Some(peer_id) == rendezvous_peer_id {
                    println!("[RECONNECT] Rendezvous server connection closed, attempting to reconnect...");
                    registered = false;
                    rendezvous_peer_id = None;
                    connection_retry_count = 0;
                    let retry_tx_clone = retry_tx.clone();
                    tokio::spawn(async move {
                        sleep(Duration::from_secs(INITIAL_RETRY_DELAY)).await;
                        let _ = retry_tx_clone.send(());
                    });
                }
            }
            SwarmEvent::Behaviour(event) => {
                match event {
                    BehaviourEvent::Rendezvous(rendezvous::client::Event::Registered { ttl, .. }) => {
                        println!("[VERBOSE] ✓ Registered with rendezvous server");
                        println!("[VERBOSE]   TTL: {:?}", ttl);
                        println!("✓ Registered! Waiting for connections...\n");
                        println!("Your Peer ID: {}", peer_id);
                        registered = true;
                    }
                    BehaviourEvent::Rendezvous(rendezvous::client::Event::RegisterFailed { error, .. }) => {
                        println!("[VERBOSE] ✗ Registration failed");
                        eprintln!("✗ Registration failed: {:?}", error);
                    }
                    BehaviourEvent::Rendezvous(e) => {
                        println!("[VERBOSE] [Rendezvous Event] {:?}", e);
                    }
                    BehaviourEvent::Identify(libp2p::identify::Event::Received { peer_id, info }) => {
                        println!("[VERBOSE] [Identify] Received from peer: {}", peer_id);
                        println!("[VERBOSE]   Protocol: {:?}", info.protocol_version);
                        println!("[VERBOSE]   Agent: {:?}", info.agent_version);
                    }
                    BehaviourEvent::RequestResponse(request_response::Event::Message { message, .. }) => {
                        match message {
                            request_response::Message::Request { request, channel, .. } => {
                                // Received a JSON message request
                                println!("\n[📨 RECEIVED JSON MESSAGE]");
                                println!("  From: {}", request.from);
                                println!("  Message: {}", request.message);
                                println!("  Timestamp: {}", request.timestamp);
                                // Show full JSON
                                if let Ok(json_str) = serde_json::to_string_pretty(&request) {
                                    println!("  Full JSON:\n{}", json_str);
                                }
                                
                                // Send a response
                                let response_msg = JsonMessage::new(
                                    format!("listener-{}", peer_id.to_string().chars().take(8).collect::<String>()),
                                    format!("Echo: {}", request.message),
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
                
                // Detailed error analysis
                let error_str = format!("{:?}", error);
                if error_str.contains("ConnectionReset") || error_str.contains("10054") {
                    println!("[DIAGNOSTIC] Connection Reset (10054) detected!");
                    println!("[DIAGNOSTIC] This usually means:");
                    println!("[DIAGNOSTIC]   - Server is running but rejecting libp2p connections");
                    println!("[DIAGNOSTIC]   - Wrong service running on port {} (not rndz server)", args.port);
                    println!("[DIAGNOSTIC]   - Protocol mismatch (server expects different protocol)");
                    println!("[DIAGNOSTIC]   - Server needs to be: ~/.cargo/bin/rndz server --listen-addr 0.0.0.0:{}", args.port);
                }
                
                // If this is the rendezvous server connection failing, retry
                if rendezvous_peer_id.is_none() || peer_id == rendezvous_peer_id {
                    connection_retry_count += 1;
                    if connection_retry_count <= MAX_RETRIES {
                        let delay_secs = INITIAL_RETRY_DELAY * connection_retry_count as u64;
                        eprintln!("✗ Connection error (attempt {}/{}): {:?}", connection_retry_count, MAX_RETRIES, error);
                        println!("[RETRY] Retrying connection in {} seconds...", delay_secs);
                        
                        // Schedule retry
                        let retry_tx_clone = retry_tx.clone();
                        tokio::spawn(async move {
                            sleep(Duration::from_secs(delay_secs)).await;
                            let _ = retry_tx_clone.send(());
                        });
                    } else {
                        eprintln!("\n✗ Connection failed after {} attempts. Please check:", MAX_RETRIES);
                        eprintln!("   1. Is the rendezvous server running?");
                        eprintln!("   2. Is the server address correct? ({})", args.server);
                        eprintln!("   3. Is port {} accessible? (firewall/network)", args.port);
                        eprintln!("   4. IMPORTANT: Server must listen on 0.0.0.0 (all interfaces):");
                        eprintln!("      ~/.cargo/bin/rndz server --listen-addr 0.0.0.0:{}", args.port);
                        eprintln!("      NOT: --listen-addr {}:{}", args.server, args.port);
                        eprintln!("   5. Verify server is running: ssh to server and check with 'ps aux | grep rndz'");
                        eprintln!("   6. Test connectivity: telnet {} {} (should connect)", args.server, args.port);
                        eprintln!("\n[INFO] Will continue trying periodically. Press Ctrl+C to exit.\n");
                        // Reset counter and continue trying with longer delays
                        connection_retry_count = 0;
                        let retry_tx_clone = retry_tx.clone();
                        tokio::spawn(async move {
                            sleep(Duration::from_secs(30)).await; // Wait 30 seconds before next attempt
                            let _ = retry_tx_clone.send(());
                        });
                    }
                } else {
                    eprintln!("✗ Connection error to peer {:?}: {:?}", peer_id, error);
                }
            }
            SwarmEvent::IncomingConnectionError { error, .. } => {
                println!("[VERBOSE] ✗ Incoming connection error: {:?}", error);
            }
            _ => {}
                }
            },
            // Handle retry signals
            _ = retry_rx.recv() => {
                if connection_retry_count <= MAX_RETRIES {
                    println!("[RETRY] Attempting to reconnect to rendezvous server...");
                    if let Err(e) = swarm.dial(addr.clone()) {
                        eprintln!("[RETRY] Failed to initiate retry: {:?}", e);
                    }
                }
            }
            _ = msg_rx.recv() => {
                // Send periodic messages to all connected peers
                if !connected_peers.is_empty() {
                    message_counter += 1;
                    for peer_id in connected_peers.keys() {
                        let json_msg = JsonMessage::new(
                            format!("listener-{}", peer_id.to_string().chars().take(8).collect::<String>()),
                            format!("Periodic message #{} from listener", message_counter),
                        );
                        let _request_id = swarm.behaviour_mut().request_response.send_request(peer_id, json_msg.clone());
                        println!("\n[📤 SENT PERIODIC JSON MESSAGE] to peer {} (#{})", peer_id, message_counter);
                        println!("  From: {}", json_msg.from);
                        println!("  Message: {}", json_msg.message);
                        println!("  Timestamp: {}", json_msg.timestamp);
                    }
                }
            }
        }
    }
}
