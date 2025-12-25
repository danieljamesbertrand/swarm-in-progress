//! Simple Kademlia Bootstrap Node - Acts as a bootstrap node for the DHT network
//! Usage: cargo run --bin server [--listen-addr ADDR] [--port PORT]

use clap::Parser;
use libp2p::{
    identity,
    tcp,
    noise,
    yamux,
    kad,
    swarm::{NetworkBehaviour, Swarm, SwarmEvent},
    core::transport::Transport,
    PeerId, Multiaddr,
};
use libp2p::swarm::Config as SwarmConfig;
use libp2p::futures::StreamExt;
use std::error::Error;
use std::time::Duration;

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
}

#[derive(NetworkBehaviour)]
struct Behaviour {
    kademlia: kad::Behaviour<kad::store::MemoryStore>,
    identify: libp2p::identify::Behaviour,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();
    
    println!("=== Simple Kademlia Bootstrap Node ===\n");
    println!("Configuration:");
    println!("  Listen Address: {}:{}", args.listen_addr, args.port);
    println!();

    // Generate local key and PeerId
    let local_key = identity::Keypair::generate_ed25519();
    let local_peer_id = PeerId::from(local_key.public());
    println!("Local peer id: {}\n", local_peer_id);

    // Transport: TCP + Noise + Yamux
    let transport = tcp::tokio::Transport::default()
        .upgrade(libp2p::core::upgrade::Version::V1)
        .authenticate(noise::Config::new(&local_key)?)
        .multiplex(yamux::Config::default())
        .boxed();

    // Kademlia DHT behaviour (bootstrap node)
    let store = kad::store::MemoryStore::new(local_peer_id);
    let kademlia_config = kad::Config::default();
    let kademlia = kad::Behaviour::with_config(local_peer_id, store, kademlia_config);

    // Identify so clients can learn our addresses/peer id
    let identify = libp2p::identify::Behaviour::new(
        libp2p::identify::Config::new(
            "punch-simple-bootstrap/1.0.0".to_string(),
            local_key.public(),
        )
    );

    let behaviour = Behaviour { kademlia, identify };
    
    // Swarm
    let swarm_config = SwarmConfig::with_tokio_executor()
        .with_idle_connection_timeout(Duration::from_secs(60));
    let mut swarm = Swarm::new(
        transport,
        behaviour,
        local_peer_id,
        swarm_config,
    );

    // Listen on specified address and port
    let addr: Multiaddr = format!("/ip4/{}/tcp/{}", args.listen_addr, args.port).parse()?;
    println!("Starting server...");
    swarm.listen_on(addr)?;

    println!("\n✅ Bootstrap node started! Waiting for connections...\n");
    println!("Clients can bootstrap to this node using:");
    println!("  --bootstrap /ip4/{}/tcp/{}", args.listen_addr, args.port);
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

