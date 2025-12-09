//! Simple Rendezvous Dialer - Discovers peers via rndz and connects
//! Usage: cargo run --bin dialer [--server HOST] [--port PORT] [--namespace NAMESPACE]

use clap::Parser;
use libp2p::{
    identity,
    tcp,
    noise,
    yamux,
    rendezvous,
    swarm::{NetworkBehaviour, Swarm, SwarmEvent},
    core::transport::Transport,
    PeerId, Multiaddr,
};
use libp2p::swarm::Config as SwarmConfig;
use libp2p::futures::StreamExt;
use std::error::Error;
use std::time::Duration;
use tokio::time::sleep;

#[derive(Parser, Debug)]
#[command(name = "dialer")]
#[command(about = "Simple Rendezvous Dialer - Discovers peers via rndz and connects")]
struct Args {
    /// Rendezvous server hostname or IP address
    #[arg(long, default_value = "162.221.207.169")]
    server: String,

    /// Rendezvous server port
    #[arg(long, default_value = "51820")]
    port: u16,

    /// Namespace for peer discovery
    #[arg(long, default_value = "simple-chat")]
    namespace: String,
}

#[derive(NetworkBehaviour)]
struct Behaviour {
    rendezvous: rendezvous::client::Behaviour,
    identify: libp2p::identify::Behaviour,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();
    
    println!("=== Simple Rendezvous Dialer ===\n");
    println!("Configuration:");
    println!("  Server: {}:{}", args.server, args.port);
    println!("  Namespace: {}\n", args.namespace);

    let key = identity::Keypair::generate_ed25519();
    let peer_id = PeerId::from(key.public());
    println!("Local Peer ID: {}\n", peer_id);

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
        libp2p::identify::Config::new("simple-dialer/1.0".to_string(), key.public())
    );
    
    let behaviour = Behaviour { rendezvous, identify };
    
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
    println!("[1] Connecting to rendezvous server: {}:{}\n", args.server, args.port);
    println!("[VERBOSE] Make sure the rendezvous server is running!");
    println!("[VERBOSE] IMPORTANT: Server should listen on 0.0.0.0 (all interfaces), not the specific IP!");
    println!("[VERBOSE] Correct server command:");
    println!("[VERBOSE]   ~/.cargo/bin/rndz server --listen-addr 0.0.0.0:51820");
    println!("[VERBOSE] NOT: ~/.cargo/bin/rndz server --listen-addr {}:{}\n", args.server, args.port);
    swarm.listen_on("/ip4/0.0.0.0/tcp/0".parse()?)?;
    
    let mut rendezvous_peer_id: Option<PeerId> = None;
    let mut discovered_peers: Vec<PeerId> = Vec::new();
    let mut connection_retry_count = 0u32;
    const MAX_RETRIES: u32 = 5;
    const INITIAL_RETRY_DELAY: u64 = 2; // seconds
    
    // Initial connection attempt
    println!("[VERBOSE] Attempting initial connection...");
    println!("[VERBOSE] Testing connectivity to {}:{}...", args.server, args.port);
    swarm.dial(addr.clone())?;
    
    // Create a channel for retry signals
    let (retry_tx, mut retry_rx) = tokio::sync::mpsc::unbounded_channel::<()>();
    
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
                
                if rendezvous_peer_id.is_none() {
                    rendezvous_peer_id = Some(peer_id);
                    connection_retry_count = 0; // Reset retry count on success
                    println!("✓ Connected to rendezvous server!");
                    // Discover peers in namespace
                    let namespace = rendezvous::Namespace::new(args.namespace.clone())?;
                    println!("\n[2] Discovering peers in namespace: {}", args.namespace);
                    swarm.behaviour_mut().rendezvous.discover(
                        Some(namespace),
                        None, // Cookie
                        None, // Limit
                        peer_id,
                    );
                } else if !discovered_peers.contains(&peer_id) {
                    println!("[VERBOSE] ✓✓✓ Connected to discovered peer: {}", peer_id);
                    println!("✓✓✓ CONNECTED to peer: {}", peer_id);
                }
            }
            SwarmEvent::ConnectionClosed { peer_id, cause, .. } => {
                println!("[VERBOSE] ✗ Connection closed");
                println!("[VERBOSE]   Peer: {}", peer_id);
                println!("[VERBOSE]   Cause: {:?}", cause);
                
                // If rendezvous server connection closed, try to reconnect
                if Some(peer_id) == rendezvous_peer_id {
                    println!("[RECONNECT] Rendezvous server connection closed, attempting to reconnect...");
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
                    BehaviourEvent::Rendezvous(rendezvous::client::Event::Discovered { registrations, .. }) => {
                        println!("[VERBOSE] ✓ Discovered {} peer(s)", registrations.len());
                        for reg in registrations {
                            let discovered_peer = reg.record.peer_id();
                            if !discovered_peers.contains(&discovered_peer) && discovered_peer != peer_id {
                                discovered_peers.push(discovered_peer);
                                println!("[VERBOSE]   Found peer: {}", discovered_peer);
                                
                                // Get addresses for this peer
                                let addrs: Vec<Multiaddr> = reg.record.addresses().iter().cloned().collect();
                                
                                println!("[VERBOSE]   Addresses: {:?}", addrs);
                                
                                // Try to connect to discovered peer
                                if let Some(addr) = addrs.first() {
                                    println!("\n[3] Connecting to discovered peer: {}", discovered_peer);
                                    println!("[VERBOSE]   Address: {}", addr);
                                    swarm.dial(addr.clone())?;
                                } else {
                                    println!("[VERBOSE]   No addresses found for peer");
                                }
                            }
                        }
                    }
                    BehaviourEvent::Rendezvous(e) => {
                        println!("[VERBOSE] [Rendezvous Event] {:?}", e);
                    }
                    BehaviourEvent::Identify(libp2p::identify::Event::Received { peer_id, info }) => {
                        println!("[VERBOSE] [Identify] Received from peer: {}", peer_id);
                        println!("[VERBOSE]   Protocol: {:?}", info.protocol_version);
                        println!("[VERBOSE]   Agent: {:?}", info.agent_version);
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
        }
    }
}
