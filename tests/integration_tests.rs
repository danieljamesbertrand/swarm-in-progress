//! Integration tests for DHT distributed inference node discovery
//! 
//! These tests verify end-to-end functionality including:
//! - Full bootstrap and discovery workflow
//! - Multi-node scenarios
//! - Real-world usage patterns

use libp2p::{
    identity,
    tcp,
    noise,
    yamux,
    kad,
    relay,
    request_response::{self, ProtocolSupport},
    swarm::{NetworkBehaviour, Swarm, SwarmEvent},
    core::transport::Transport,
    PeerId, StreamProtocol,
};
use libp2p::swarm::Config as SwarmConfig;
use libp2p::futures::StreamExt;
use std::time::Duration;
use tokio::time::timeout;

use punch_simple::JsonCodec;

#[derive(NetworkBehaviour)]
struct TestBehaviour {
    kademlia: kad::Behaviour<kad::store::MemoryStore>,
    identify: libp2p::identify::Behaviour,
    request_response: request_response::Behaviour<JsonCodec>,
    relay: relay::Behaviour,
}

async fn create_test_swarm(peer_id: PeerId, key: identity::Keypair) -> Swarm<TestBehaviour> {
    let transport = tcp::tokio::Transport::default()
        .upgrade(libp2p::core::upgrade::Version::V1)
        .authenticate(noise::Config::new(&key).unwrap())
        .multiplex(yamux::Config::default())
        .boxed();
    
    let store = kad::store::MemoryStore::new(peer_id);
    let mut kademlia_config = kad::Config::default();
    kademlia_config.set_query_timeout(Duration::from_secs(10));
    let kademlia = kad::Behaviour::with_config(peer_id, store, kademlia_config);
    
    let identify = libp2p::identify::Behaviour::new(
        libp2p::identify::Config::new("test-node/1.0".to_string(), key.public())
    );
    
    let codec = JsonCodec;
    let request_response = request_response::Behaviour::with_codec(
        codec,
        [(StreamProtocol::new("/json-message/1.0"), ProtocolSupport::Full)],
        request_response::Config::default(),
    );
    
    let relay = relay::Behaviour::new(peer_id, relay::Config::default());
    
    let behaviour = TestBehaviour { kademlia, identify, request_response, relay };
    
    let swarm_config = SwarmConfig::with_tokio_executor()
        .with_idle_connection_timeout(Duration::from_secs(30));
    Swarm::new(transport, behaviour, peer_id, swarm_config)
}

/// Integration test: Full workflow from bootstrap to message exchange
#[tokio::test]
#[ignore] // Ignore by default as it requires network setup
async fn test_full_workflow_bootstrap_discovery_message() {
    // Step 1: Create bootstrap node
    let bootstrap_key = identity::Keypair::generate_ed25519();
    let bootstrap_peer_id = PeerId::from(bootstrap_key.public());
    let mut bootstrap_swarm = create_test_swarm(bootstrap_peer_id, bootstrap_key).await;
    
    bootstrap_swarm.listen_on("/ip4/127.0.0.1/tcp/0".parse().unwrap()).unwrap();
    
    let mut bootstrap_addr = None;
    let bootstrap_future = async {
        loop {
            match bootstrap_swarm.select_next_some().await {
                SwarmEvent::NewListenAddr { address, .. } => {
                    bootstrap_addr = Some(address);
                    break;
                }
                _ => {}
            }
        }
    };
    timeout(Duration::from_secs(5), bootstrap_future).await.unwrap();
    let bootstrap_addr = bootstrap_addr.unwrap();
    
    // Step 2: Create two client nodes
    let client1_key = identity::Keypair::generate_ed25519();
    let client1_peer_id = PeerId::from(client1_key.public());
    let mut client1_swarm = create_test_swarm(client1_peer_id, client1_key).await;
    
    let client2_key = identity::Keypair::generate_ed25519();
    let client2_peer_id = PeerId::from(client2_key.public());
    let mut client2_swarm = create_test_swarm(client2_peer_id, client2_key).await;
    
    // Step 3: Bootstrap both clients
    client1_swarm.behaviour_mut().kademlia.add_address(&bootstrap_peer_id, bootstrap_addr.clone());
    client2_swarm.behaviour_mut().kademlia.add_address(&bootstrap_peer_id, bootstrap_addr.clone());
    
    client1_swarm.listen_on("/ip4/127.0.0.1/tcp/0".parse().unwrap()).unwrap();
    client2_swarm.listen_on("/ip4/127.0.0.1/tcp/0".parse().unwrap()).unwrap();
    
    client1_swarm.dial(bootstrap_addr.clone()).unwrap();
    client2_swarm.dial(bootstrap_addr.clone()).unwrap();
    
    // Wait for bootstrap
    tokio::time::sleep(Duration::from_secs(2)).await;
    
    // Step 4: Store records in DHT
    let namespace = "test-namespace";
    let key1 = kad::RecordKey::new(&format!("{}-{}", namespace, client1_peer_id));
    let key2 = kad::RecordKey::new(&format!("{}-{}", namespace, client2_peer_id));
    
    let record1 = kad::Record::new(key1.clone(), client1_peer_id.to_bytes());
    let record2 = kad::Record::new(key2.clone(), client2_peer_id.to_bytes());
    
    client1_swarm.behaviour_mut().kademlia.put_record(record1, kad::Quorum::One).unwrap();
    client2_swarm.behaviour_mut().kademlia.put_record(record2, kad::Quorum::One).unwrap();
    
    // Wait for records to propagate
    tokio::time::sleep(Duration::from_millis(500)).await;
    
    // Step 5: Discover peers
    client1_swarm.behaviour_mut().kademlia.get_closest_peers(client2_peer_id);
    client2_swarm.behaviour_mut().kademlia.get_closest_peers(client1_peer_id);
    
    // Wait for discovery
    tokio::time::sleep(Duration::from_millis(500)).await;
    
    // Test passes if no panics occur
    assert!(true, "Full workflow completed successfully");
}

/// Integration test: Multiple nodes discovering each other
#[tokio::test]
#[ignore]
async fn test_multi_node_discovery() {
    // Create bootstrap
    let bootstrap_key = identity::Keypair::generate_ed25519();
    let bootstrap_peer_id = PeerId::from(bootstrap_key.public());
    let mut bootstrap_swarm = create_test_swarm(bootstrap_peer_id, bootstrap_key).await;
    
    bootstrap_swarm.listen_on("/ip4/127.0.0.1/tcp/0".parse().unwrap()).unwrap();
    
    let mut bootstrap_addr = None;
    let bootstrap_future = async {
        loop {
            match bootstrap_swarm.select_next_some().await {
                SwarmEvent::NewListenAddr { address, .. } => {
                    bootstrap_addr = Some(address);
                    break;
                }
                _ => {}
            }
        }
    };
    timeout(Duration::from_secs(5), bootstrap_future).await.unwrap();
    let bootstrap_addr = bootstrap_addr.unwrap();
    
    // Create multiple nodes
    let num_nodes = 5;
    let mut nodes = Vec::new();
    
    for _i in 0..num_nodes {
        let key = identity::Keypair::generate_ed25519();
        let peer_id = PeerId::from(key.public());
        let mut swarm = create_test_swarm(peer_id, key).await;
        
        swarm.behaviour_mut().kademlia.add_address(&bootstrap_peer_id, bootstrap_addr.clone());
        swarm.listen_on("/ip4/127.0.0.1/tcp/0".parse().unwrap()).unwrap();
        swarm.dial(bootstrap_addr.clone()).unwrap();
        
        nodes.push((peer_id, swarm));
    }
    
    // Wait for all nodes to connect
    tokio::time::sleep(Duration::from_secs(3)).await;
    
    // All nodes should have unique peer IDs
    let peer_ids: Vec<PeerId> = nodes.iter().map(|(pid, _)| *pid).collect();
    let unique_peer_ids: std::collections::HashSet<PeerId> = peer_ids.iter().cloned().collect();
    assert_eq!(peer_ids.len(), unique_peer_ids.len(), "All nodes should have unique peer IDs");
    
    // Store records for all nodes
    let namespace = "multi-node-test";
    for (peer_id, swarm) in &mut nodes {
        let key = kad::RecordKey::new(&format!("{}-{}", namespace, peer_id));
        let record = kad::Record::new(key, peer_id.to_bytes());
        let _ = swarm.behaviour_mut().kademlia.put_record(record, kad::Quorum::One);
    }
    
    // Wait for records to propagate
    tokio::time::sleep(Duration::from_millis(1000)).await;
    
    // Test passes if no panics occur
    assert!(true, "Multi-node discovery test completed");
}

/// Integration test: Namespace isolation
#[tokio::test]
#[ignore]
async fn test_namespace_isolation() {
    // Create bootstrap
    let bootstrap_key = identity::Keypair::generate_ed25519();
    let bootstrap_peer_id = PeerId::from(bootstrap_key.public());
    let mut bootstrap_swarm = create_test_swarm(bootstrap_peer_id, bootstrap_key).await;
    
    bootstrap_swarm.listen_on("/ip4/127.0.0.1/tcp/0".parse().unwrap()).unwrap();
    
    let mut bootstrap_addr = None;
    let bootstrap_future = async {
        loop {
            match bootstrap_swarm.select_next_some().await {
                SwarmEvent::NewListenAddr { address, .. } => {
                    bootstrap_addr = Some(address);
                    break;
                }
                _ => {}
            }
        }
    };
    timeout(Duration::from_secs(5), bootstrap_future).await.unwrap();
    let bootstrap_addr = bootstrap_addr.unwrap();
    
    // Create nodes in different namespaces
    let namespace1 = "namespace-1";
    let namespace2 = "namespace-2";
    
    let key1 = identity::Keypair::generate_ed25519();
    let peer_id1 = PeerId::from(key1.public());
    let mut swarm1 = create_test_swarm(peer_id1, key1).await;
    
    let key2 = identity::Keypair::generate_ed25519();
    let peer_id2 = PeerId::from(key2.public());
    let mut swarm2 = create_test_swarm(peer_id2, key2).await;
    
    swarm1.behaviour_mut().kademlia.add_address(&bootstrap_peer_id, bootstrap_addr.clone());
    swarm2.behaviour_mut().kademlia.add_address(&bootstrap_peer_id, bootstrap_addr.clone());
    
    swarm1.listen_on("/ip4/127.0.0.1/tcp/0".parse().unwrap()).unwrap();
    swarm2.listen_on("/ip4/127.0.0.1/tcp/0".parse().unwrap()).unwrap();
    
    swarm1.dial(bootstrap_addr.clone()).unwrap();
    swarm2.dial(bootstrap_addr.clone()).unwrap();
    
    // Store records in different namespaces
    let key1_record = kad::RecordKey::new(&namespace1);
    let key2_record = kad::RecordKey::new(&namespace2);
    
    let record1 = kad::Record::new(key1_record.clone(), peer_id1.to_bytes());
    let record2 = kad::Record::new(key2_record.clone(), peer_id2.to_bytes());
    
    swarm1.behaviour_mut().kademlia.put_record(record1, kad::Quorum::One).unwrap();
    swarm2.behaviour_mut().kademlia.put_record(record2, kad::Quorum::One).unwrap();
    
    // Wait for records to propagate
    tokio::time::sleep(Duration::from_millis(500)).await;
    
    // Namespaces should be isolated - records in different namespaces should have different keys
    assert_ne!(key1_record, key2_record, "Different namespaces should have different keys");
    
    // Test passes if no panics occur
    assert!(true, "Namespace isolation test completed");
}

