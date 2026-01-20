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
    swarm::{NetworkBehaviour, Swarm, SwarmEvent},
    PeerId, Multiaddr,
};
use libp2p::swarm::Config as SwarmConfig;
use libp2p::futures::StreamExt;
use std::error::Error;
use std::time::Duration;
use punch_simple::quic_transport::{create_transport, get_dual_listen_addresses, get_listen_address, TransportType};

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
}

#[derive(NetworkBehaviour)]
struct Behaviour {
    kademlia: kad::Behaviour<kad::store::MemoryStore>,
    identify: libp2p::identify::Behaviour,
    ping: ping::Behaviour,
    relay: relay::Behaviour,
}

/// Run bootstrap server with a specified transport.
pub async fn run_bootstrap_with_transport(
    listen_addr: String,
    port: u16,
    transport_type: TransportType,
) -> Result<(), Box<dyn Error>> {
    println!("=== Simple Kademlia Bootstrap Node ===\n");
    println!("Configuration:");
    println!("  Listen Address: {}:{}", listen_addr, port);
    println!();

    // Generate local key and PeerId
    let local_key = identity::Keypair::generate_ed25519();
    let local_peer_id = PeerId::from(local_key.public());
    println!("Local peer id: {}\n", local_peer_id);

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

    let behaviour = Behaviour { kademlia, identify, ping, relay };
    
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
    println!("\nPress Ctrl+C to stop the bootstrap node.\n");

    // Main event loop
    loop {
        match swarm.select_next_some().await {
            SwarmEvent::NewListenAddr { address, .. } => {
                println!("[SERVER] Listening on {}", address);
            }
            SwarmEvent::ConnectionEstablished { peer_id, endpoint, .. } => {
                println!("[SERVER] ✓ Connection established from peer: {}", peer_id);
                if endpoint.is_dialer() {
                    println!("[SERVER]   (Outbound connection)");
                } else {
                    println!("[SERVER]   (Inbound connection)");
                }
            }
            SwarmEvent::ConnectionClosed { peer_id, cause, .. } => {
                println!("[SERVER] ✗ Connection closed: peer {}, cause: {:?}", peer_id, cause);
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
                    _ => {}
                }
            }
            SwarmEvent::OutgoingConnectionError { error, peer_id, .. } => {
                println!("[SERVER] ✗ Outgoing connection error to {:?}: {:?}", peer_id, error);
            }
            SwarmEvent::IncomingConnectionError { error, .. } => {
                println!("[SERVER] ✗ Incoming connection error: {:?}", error);
            }
            _ => {}
        }
    }
}

/// Run bootstrap server (extracted for unified binary).
///
/// Backwards-compatible wrapper that defaults to dual-stack transport.
pub async fn run_bootstrap(listen_addr: String, port: u16) -> Result<(), Box<dyn Error>> {
    run_bootstrap_with_transport(listen_addr, port, TransportType::DualStack).await
}

#[allow(dead_code)]
#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();
    run_bootstrap_with_transport(args.listen_addr, args.port, args.transport).await
}

