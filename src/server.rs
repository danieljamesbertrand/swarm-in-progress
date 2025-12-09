//! Simple Rendezvous Server - Accepts peer registrations and serves discovery requests
//! Usage: cargo run --bin server [--listen-addr ADDR] [--port PORT]

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

#[derive(Parser, Debug)]
#[command(name = "server")]
#[command(about = "Simple Rendezvous Server - Accepts peer registrations and serves discovery requests")]
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
    rendezvous: rendezvous::server::Behaviour,
    identify: libp2p::identify::Behaviour,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();
    
    println!("=== Simple Rendezvous Server ===\n");
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

    // Rendezvous server behaviour
    let rendezvous = rendezvous::server::Behaviour::new(
        rendezvous::server::Config::default()
    );

    // Identify so clients can learn our addresses/peer id
    let identify = libp2p::identify::Behaviour::new(
        libp2p::identify::Config::new(
            "punch-simple-server/1.0.0".to_string(),
            local_key.public(),
        )
    );

    let behaviour = Behaviour { rendezvous, identify };
    
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

    println!("\n✅ Server started! Waiting for connections...\n");
    println!("Clients can connect to this server using:");
    println!("  --server {} --port {}", args.listen_addr, args.port);
    println!("\nPress Ctrl+C to stop the server.\n");

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
                    BehaviourEvent::Rendezvous(rendezvous::server::Event::PeerRegistered { peer, registration }) => {
                        println!("[SERVER] 📝 Peer {} registered", peer);
                        println!("[SERVER]   Namespace: {}", registration.namespace);
                        println!("[SERVER]   TTL: {} seconds", registration.ttl);
                        if let Some(record) = registration.record.addresses().first() {
                            println!("[SERVER]   Address: {}", record);
                        }
                    }
                    BehaviourEvent::Rendezvous(rendezvous::server::Event::PeerNotRegistered { peer, error, namespace }) => {
                        println!("[SERVER] ✗ Peer {} registration failed for namespace {}: {:?}", peer, namespace, error);
                    }
                    BehaviourEvent::Rendezvous(rendezvous::server::Event::DiscoverServed { enquirer, registrations }) => {
                        println!("[SERVER] 🔍 Discovery request from peer: {}", enquirer);
                        println!("[SERVER]   Serving {} registration(s)", registrations.len());
                    }
                    BehaviourEvent::Rendezvous(rendezvous::server::Event::DiscoverNotServed { enquirer, error }) => {
                        println!("[SERVER] ✗ Discovery request from peer {} failed: {:?}", enquirer, error);
                    }
                    BehaviourEvent::Rendezvous(e) => {
                        println!("[SERVER] [Rendezvous Event] {:?}", e);
                    }
                    BehaviourEvent::Identify(libp2p::identify::Event::Received { peer_id, info }) => {
                        println!("[SERVER] [Identify] Received from peer: {}", peer_id);
                        println!("[SERVER]   Protocol: {:?}", info.protocol_version);
                        println!("[SERVER]   Agent: {:?}", info.agent_version);
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

