//! Node Internal State Monitor
//! Continuously probes nodes and rendezvous server to show internal state

use libp2p::kad;
use libp2p::swarm::{NetworkBehaviour, Swarm, SwarmEvent};
use libp2p::futures::StreamExt;
use libp2p::{identity, Multiaddr, PeerId};
use std::collections::HashMap;
use std::time::Duration;
use tokio::time;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║         NODE INTERNAL STATE MONITOR                         ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!();

    // Parse command line args
    let args: Vec<String> = std::env::args().collect();
    let bootstrap = args.get(1)
        .map(|s| s.as_str())
        .unwrap_or("eagleoneonline.ca:51820");
    
    let (host, port) = if let Some(colon) = bootstrap.find(':') {
        let h = &bootstrap[..colon];
        let p = bootstrap[colon+1..].parse::<u16>().unwrap_or(51820);
        (h, p)
    } else {
        (bootstrap, 51820)
    };

    println!("Configuration:");
    println!("  Bootstrap: {}:{}", host, port);
    println!("  Cluster: llama-cluster");
    println!("  Update Interval: 5 seconds");
    println!();

    // Generate keys
    let key = identity::Keypair::generate_ed25519();
    let peer_id = PeerId::from(key.public());
    println!("Monitor Peer ID: {}", peer_id);
    println!();

    // Create transport (QUIC) using existing helper
    let transport = punch_simple::quic_transport::create_quic_transport(&key)
        .map_err(|e| format!("Failed to create transport: {}", e))?;
    
    // Create Kademlia for DHT queries
    let store = kad::store::MemoryStore::new(peer_id);
    let mut kademlia_config = kad::Config::default();
    kademlia_config.set_query_timeout(Duration::from_secs(10));
    let kademlia = kad::Behaviour::with_config(peer_id, store, kademlia_config);

    // Simple behaviour wrapper for Kademlia
    #[derive(NetworkBehaviour)]
    #[behaviour(out_event = "monitor_behaviour::Event")]
    struct MonitorBehaviour {
        kademlia: kad::Behaviour<kad::store::MemoryStore>,
    }

    mod monitor_behaviour {
        use libp2p::kad;
        use libp2p::swarm::NetworkBehaviour;

        #[derive(NetworkBehaviour)]
        #[behaviour(out_event = "Event")]
        pub struct MonitorBehaviour {
            pub kademlia: kad::Behaviour<kad::store::MemoryStore>,
        }

        #[derive(Debug)]
        pub enum Event {
            Kademlia(kad::Event),
        }

        impl From<kad::Event> for Event {
            fn from(event: kad::Event) -> Self {
                Event::Kademlia(event)
            }
        }
    }

    let behaviour = monitor_behaviour::MonitorBehaviour { kademlia };

    // Create swarm
    let swarm_config = libp2p::swarm::Config::with_tokio_executor();
    let mut swarm = Swarm::new(transport, behaviour, peer_id, swarm_config);

    // Resolve bootstrap address
    println!("[1/3] Resolving bootstrap server...");
    let bootstrap_ip = tokio::net::lookup_host(format!("{}:{}", host, port))
        .await?
        .next()
        .ok_or("Failed to resolve bootstrap host")?;
    
    let bootstrap_addr = format!("/ip4/{}/udp/{}/quic-v1", 
        bootstrap_ip.ip(), port);
    println!("  Resolved to: {}", bootstrap_addr);
    println!();

    // Connect to bootstrap
    println!("[2/3] Connecting to bootstrap server...");
    let bootstrap_addr: Multiaddr = bootstrap_addr.parse()?;
    swarm.dial(bootstrap_addr.clone())?;
    
    // Wait for connection
    let mut connected = false;
    let mut start_time = time::Instant::now();
    while !connected && start_time.elapsed() < Duration::from_secs(10) {
        match swarm.select_next_some().await {
            SwarmEvent::ConnectionEstablished { peer_id: _, endpoint: _, .. } => {
                println!("  [OK] Connected to bootstrap server");
                connected = true;
            }
            SwarmEvent::OutgoingConnectionError { error, .. } => {
                eprintln!("  [ERROR] Connection failed: {}", error);
                return Err(format!("Failed to connect: {}", error).into());
            }
            _ => {}
        }
    }

    if !connected {
        return Err("Failed to connect to bootstrap within timeout".into());
    }

    println!();
    println!("[3/3] Starting monitoring loop...");
    println!();
    println!("═══════════════════════════════════════════════════════════════");
    println!();

    // Query DHT for all shards
    let cluster_name = "llama-cluster";
    let mut discovered_nodes: HashMap<u32, (PeerId, bool)> = HashMap::new(); // shard_id -> (peer_id, shard_loaded)
    
    let mut query_interval = time::interval(Duration::from_secs(5));
    let mut status_interval = time::interval(Duration::from_secs(10));
    
    loop {
        tokio::select! {
            // Handle swarm events
            event = swarm.select_next_some() => {
                match event {
                    SwarmEvent::ConnectionEstablished { peer_id, endpoint, .. } => {
                        println!("[CONNECTION] Connected to peer: {}", peer_id);
                    }
                    SwarmEvent::ConnectionClosed { peer_id, cause, .. } => {
                        println!("[CONNECTION] Disconnected from peer: {} (cause: {:?})", peer_id, cause);
                    }
                    SwarmEvent::Behaviour(monitor_behaviour::Event::Kademlia(kad::Event::RoutingUpdated { .. })) => {
                        // DHT routing table updated
                    }
                    SwarmEvent::Behaviour(monitor_behaviour::Event::Kademlia(kad::Event::OutboundQueryProgressed { id: _, result, .. })) => {
                        // DHT query result
                        match result {
                            kad::QueryResult::GetRecord(Ok(kad::GetRecordOk::FoundRecord(peer_record))) => {
                                // Parse shard announcement
                                if let Ok(announcement) = punch_simple::kademlia_shard_discovery::ShardAnnouncement::from_bytes(&peer_record.record.value) {
                                    let shard_id = announcement.shard_id;
                                    let announcement_peer_id = announcement.peer_id.parse::<PeerId>()
                                        .unwrap_or_else(|_| peer_id);
                                    let shard_loaded = announcement.capabilities.shard_loaded;
                                    
                                    discovered_nodes.insert(shard_id, (announcement_peer_id, shard_loaded));
                                    println!("[DISCOVERY] Found shard {} (peer: {}, loaded: {})", 
                                        shard_id, 
                                        announcement_peer_id.to_string().chars().take(12).collect::<String>(),
                                        shard_loaded
                                    );
                                }
                            }
                            _ => {}
                        }
                    }
                    _ => {}
                }
            }
            
            // Periodic DHT queries
            _ = query_interval.tick() => {
                // Query DHT for all shards 0-7
                for shard_id in 0..8 {
                    let key = libp2p::kad::RecordKey::new(
                        &format!("/llama-cluster/{}/shard/{}", cluster_name, shard_id)
                    );
                    swarm.behaviour_mut().kademlia.get_record(key);
                }
            }
            
            // Periodic status display
            _ = status_interval.tick() => {
                println!();
                println!("═══════════════════════════════════════════════════════════════");
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs();
                let hours = (now / 3600) % 24;
                let minutes = (now / 60) % 60;
                let seconds = now % 60;
                println!("  STATUS REPORT - {:02}:{:02}:{:02}", hours, minutes, seconds);
                println!("═══════════════════════════════════════════════════════════════");
                println!();
                
                println!("Discovered Nodes: {}/8", discovered_nodes.len());
                println!();
                
                if discovered_nodes.is_empty() {
                    println!("  [INFO] No nodes discovered yet");
                    println!("  [TIP] Nodes may still be starting up");
                } else {
                    println!("  Shard Status:");
                    for shard_id in 0..8 {
                        if let Some((peer_id, shard_loaded)) = discovered_nodes.get(&shard_id) {
                            let status = if *shard_loaded { "[LOADED]" } else { "[NOT LOADED]" };
                            let color = if *shard_loaded { "✓" } else { "✗" };
                            println!("    Shard {}: {} {} (peer: {}...)", 
                                shard_id, 
                                color,
                                status,
                                peer_id.to_string().chars().take(12).collect::<String>()
                            );
                        } else {
                            println!("    Shard {}: [MISSING]", shard_id);
                        }
                    }
                    
                    // Check swarm readiness
                    let all_discovered = discovered_nodes.len() == 8;
                    let all_loaded = discovered_nodes.values().all(|(_, loaded)| *loaded);
                    
                    println!();
                    if all_discovered && all_loaded {
                        println!("  [SWARM] ✓✓✓ SWARM READY FOR INFERENCE ✓✓✓");
                    } else {
                        println!("  [SWARM] ⚠️  Swarm not ready:");
                        if !all_discovered {
                            println!("    - Missing {} shard(s)", 8 - discovered_nodes.len());
                        }
                        if !all_loaded {
                            let not_loaded: Vec<u32> = discovered_nodes.iter()
                                .filter(|(_, (_, loaded))| !*loaded)
                                .map(|(shard_id, _)| *shard_id)
                                .collect();
                            println!("    - Shard(s) not loaded: {:?}", not_loaded);
                        }
                    }
                }
                
                println!();
                println!("═══════════════════════════════════════════════════════════════");
                println!();
            }
        }
    }
}
