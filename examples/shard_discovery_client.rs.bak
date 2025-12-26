//! Shard Discovery Client Example
//!
//! Demonstrates how to discover all Llama shards in a cluster using Kademlia DHT.
//! This client joins the DHT, queries for all shards, and builds the inference pipeline.
//!
//! Usage:
//!   cargo run --example shard_discovery_client -- \
//!     --bootstrap /ip4/SERVER/tcp/51820 \
//!     --cluster llama-8b-cluster \
//!     --total-shards 4
//!
//! The client will:
//! 1. Connect to the Kademlia DHT via bootstrap node
//! 2. Query for all shard records (0 to total_shards-1)
//! 3. Build a sorted pipeline of discovered shards
//! 4. Display pipeline status and send a test inference request

use clap::Parser;
use libp2p::{
    identity,
    tcp,
    noise,
    yamux,
    kad,
    request_response::{self, ProtocolSupport},
    swarm::{NetworkBehaviour, Swarm, SwarmEvent},
    core::transport::Transport,
    PeerId, Multiaddr, StreamProtocol,
};
use libp2p::swarm::Config as SwarmConfig;
use libp2p::futures::StreamExt;
use punch_simple::{
    KademliaShardDiscovery, dht_keys,
    JsonMessage, JsonCodec, Command, commands,
};
use serde_json::json;
use std::error::Error;
use std::time::Duration;
use std::collections::HashMap;

#[derive(Parser, Debug)]
#[command(name = "shard_discovery_client")]
#[command(about = "Discover Llama shards via Kademlia DHT and build inference pipeline")]
struct Args {
    /// Bootstrap node address
    #[arg(long, default_value = "/ip4/127.0.0.1/tcp/51820")]
    bootstrap: String,

    /// Cluster name
    #[arg(long, default_value = "llama-cluster")]
    cluster: String,

    /// Expected total shards
    #[arg(long, default_value = "4")]
    total_shards: u32,

    /// Discovery timeout in seconds
    #[arg(long, default_value = "30")]
    timeout: u64,

    /// Run test inference after discovery
    #[arg(long)]
    test_inference: bool,
}

#[derive(NetworkBehaviour)]
struct ClientBehaviour {
    kademlia: kad::Behaviour<kad::store::MemoryStore>,
    identify: libp2p::identify::Behaviour,
    request_response: request_response::Behaviour<JsonCodec>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();

    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║         Shard Discovery Client                               ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!();
    println!("Configuration:");
    println!("  Cluster: {}", args.cluster);
    println!("  Expected Shards: {}", args.total_shards);
    println!("  Bootstrap: {}", args.bootstrap);
    println!("  Timeout: {}s", args.timeout);
    println!();

    // Generate keys
    let key = identity::Keypair::generate_ed25519();
    let peer_id = PeerId::from(key.public());
    println!("Client Peer ID: {}", peer_id);

    // Transport
    let transport = tcp::tokio::Transport::default()
        .upgrade(libp2p::core::upgrade::Version::V1)
        .authenticate(noise::Config::new(&key)?)
        .multiplex(yamux::Config::default())
        .boxed();

    // Kademlia
    let store = kad::store::MemoryStore::new(peer_id);
    let mut kademlia_config = kad::Config::default();
    kademlia_config.set_query_timeout(Duration::from_secs(30));
    let kademlia = kad::Behaviour::with_config(peer_id, store, kademlia_config);

    // Identify
    let identify = libp2p::identify::Behaviour::new(
        libp2p::identify::Config::new("shard-discovery-client/1.0".to_string(), key.public())
    );

    // Request-Response
    let request_response = request_response::Behaviour::with_codec(
        JsonCodec,
        [(StreamProtocol::new("/json-message/1.0"), ProtocolSupport::Full)],
        request_response::Config::default(),
    );

    let behaviour = ClientBehaviour {
        kademlia,
        identify,
        request_response,
    };

    // Swarm
    let swarm_config = SwarmConfig::with_tokio_executor()
        .with_idle_connection_timeout(Duration::from_secs(60));
    let mut swarm = Swarm::new(transport, behaviour, peer_id, swarm_config);

    // Listen on ephemeral port
    swarm.listen_on("/ip4/0.0.0.0/tcp/0".parse()?)?;

    // Connect to bootstrap
    let bootstrap_addr: Multiaddr = args.bootstrap.parse()?;
    println!("\n🔗 Connecting to bootstrap node...");
    swarm.dial(bootstrap_addr)?;

    // Discovery state
    let mut discovery = KademliaShardDiscovery::with_expected_shards(&args.cluster, args.total_shards);
    let mut bootstrapped = false;
    let mut queries_sent = false;
    let mut connected_peers: HashMap<PeerId, Multiaddr> = HashMap::new();

    let timeout_instant = tokio::time::Instant::now() + Duration::from_secs(args.timeout);

    println!("⏳ Discovering shards (timeout: {}s)...\n", args.timeout);

    loop {
        tokio::select! {
            event = swarm.select_next_some() => {
                match event {
                    SwarmEvent::NewListenAddr { address, .. } => {
                        println!("[LISTEN] Client listening on: {}", address);
                    }

                    SwarmEvent::ConnectionEstablished { peer_id: connected_peer, endpoint, .. } => {
                        println!("[CONNECT] ✓ Connected to: {}", connected_peer);
                        
                        let addr = endpoint.get_remote_address().clone();
                        connected_peers.insert(connected_peer, addr);

                        if !bootstrapped {
                            if let Err(e) = swarm.behaviour_mut().kademlia.bootstrap() {
                                eprintln!("[WARN] Bootstrap failed: {:?}", e);
                            } else {
                                println!("[DHT] ✓ Started Kademlia bootstrap");
                                bootstrapped = true;
                            }
                        }
                    }

                    SwarmEvent::Behaviour(behaviour_event) => {
                        match behaviour_event {
                            ClientBehaviourEvent::Kademlia(kad::Event::RoutingUpdated { .. }) => {
                                println!("[DHT] Routing table updated");

                                // Send queries for all shards
                                if !queries_sent && bootstrapped {
                                    println!("\n📡 Querying for {} shards...", args.total_shards);
                                    for shard_id in 0..args.total_shards {
                                        let key = kad::RecordKey::new(&dht_keys::shard_key(&args.cluster, shard_id));
                                        swarm.behaviour_mut().kademlia.get_record(key);
                                        println!("   → Querying shard {}", shard_id);
                                    }
                                    queries_sent = true;
                                }
                            }

                            ClientBehaviourEvent::Kademlia(kad::Event::OutboundQueryProgressed {
                                result: kad::QueryResult::GetRecord(Ok(kad::GetRecordOk::FoundRecord(peer_record))),
                                ..
                            }) => {
                                if let Some(ann) = discovery.process_shard_record(&peer_record.record) {
                                    println!("   ✓ Found shard {} at {} (layers {}-{})", 
                                        ann.shard_id, 
                                        ann.peer_id.chars().take(16).collect::<String>() + "...",
                                        ann.layer_start,
                                        ann.layer_end
                                    );
                                }

                                // Check if complete
                                if discovery.is_pipeline_complete() {
                                    println!("\n╔══════════════════════════════════════════════════════════════╗");
                                    println!("║  ✅ Pipeline Complete!                                        ║");
                                    println!("╚══════════════════════════════════════════════════════════════╝");
                                    
                                    display_pipeline(&discovery);

                                    if args.test_inference {
                                        // Test inference request
                                        if let Some(entry) = discovery.entry_node() {
                                            println!("\n🧠 Sending test inference request to entry node...");
                                            
                                            let cmd = Command::new(commands::EXECUTE_TASK, &peer_id.to_string(), Some(&entry.peer_id))
                                                .with_param("task_type", json!("ai_inference"))
                                                .with_param("input_data", json!("What is the capital of France?"))
                                                .with_param("max_tokens", json!(100));
                                            
                                            let cmd_json = serde_json::to_string(&cmd)?;
                                            let msg = JsonMessage::new(peer_id.to_string(), cmd_json);
                                            
                                            // Parse multiaddr to get PeerId
                                            if let Ok(target_peer) = entry.peer_id.parse::<PeerId>() {
                                                swarm.behaviour_mut().request_response.send_request(&target_peer, msg);
                                                println!("   → Request sent to {}", entry.peer_id);
                                            }
                                        }
                                    } else {
                                        println!("\n✓ Discovery complete. Use --test-inference to send a test request.");
                                        return Ok(());
                                    }
                                }
                            }

                            ClientBehaviourEvent::Kademlia(kad::Event::OutboundQueryProgressed {
                                result: kad::QueryResult::GetRecord(Err(err)),
                                ..
                            }) => {
                                println!("   ⚠ Query failed: {:?}", err);
                            }

                            ClientBehaviourEvent::RequestResponse(request_response::Event::Message {
                                message: request_response::Message::Response { response, .. },
                                ..
                            }) => {
                                println!("\n📨 Received response: {}", response.message);
                                println!("\n✓ Test inference complete!");
                                return Ok(());
                            }

                            _ => {}
                        }
                    }

                    _ => {}
                }
            }

            _ = tokio::time::sleep_until(timeout_instant) => {
                println!("\n⏰ Discovery timeout reached!");
                display_pipeline(&discovery);
                
                let status = discovery.status();
                if !status.missing_shards.is_empty() {
                    println!("\n⚠️ Missing shards: {:?}", status.missing_shards);
                    println!("   Make sure all shard_listener nodes are running.");
                }
                
                return Ok(());
            }
        }
    }
}

fn display_pipeline(discovery: &KademliaShardDiscovery) {
    let status = discovery.status();
    let pipeline = discovery.get_pipeline();

    println!("\n┌─────────────────────────────────────────────────────────────────┐");
    println!("│ Pipeline Status                                                  │");
    println!("├─────────────────────────────────────────────────────────────────┤");
    println!("│ Cluster: {:54} │", status.cluster_name);
    println!("│ Shards: {}/{} discovered, {} replicas{:23} │", 
        status.discovered_shards, status.expected_shards, status.total_replicas, "");
    println!("│ Complete: {:53} │", if status.is_complete { "✅ Yes" } else { "❌ No" });
    println!("├─────────────────────────────────────────────────────────────────┤");
    
    for shard in &pipeline {
        let role = if shard.has_embeddings { 
            "Entry (Embeddings)" 
        } else if shard.has_output { 
            "Exit (Output Head)" 
        } else { 
            "Middle" 
        };
        
        println!("│ Shard {:2}: Layers {:2}-{:2} | {:18} | {} │", 
            shard.shard_id,
            shard.layer_start,
            shard.layer_end,
            role,
            &shard.peer_id.chars().take(12).collect::<String>()
        );
    }
    
    println!("└─────────────────────────────────────────────────────────────────┘");

    // Show pipeline flow
    if !pipeline.is_empty() {
        println!("\n📊 Pipeline Flow:");
        let flow: Vec<String> = pipeline.iter()
            .map(|s| format!("[Shard {}]", s.shard_id))
            .collect();
        println!("   {}", flow.join(" → "));
    }
}

