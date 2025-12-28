//! Comprehensive Transport Tests
//!
//! These tests verify that both TCP and QUIC transports work correctly.
//! Run with: cargo test --test transport_tests
//!
//! IMPORTANT: These tests are critical for ensuring the TCP->QUIC migration
//! doesn't corrupt the software. All tests must pass before deploying.

use libp2p::{
    identity::Keypair,
    kad,
    request_response::{self, ProtocolSupport},
    swarm::{NetworkBehaviour, Swarm, SwarmEvent},
    core::transport::Transport,
    PeerId, Multiaddr, StreamProtocol,
};
use libp2p::swarm::Config as SwarmConfig;
use libp2p::futures::StreamExt;
use std::time::Duration;
use tokio::time::timeout;

// Import the transport functions
use punch_simple::quic_transport::{
    create_quic_transport, create_tcp_transport, create_dual_transport,
    create_transport, TransportType, get_listen_address, get_dual_listen_addresses,
};
use punch_simple::{JsonMessage, JsonCodec};

// ============================================================================
// UNIT TESTS - Transport Creation
// ============================================================================

#[test]
fn test_transport_type_parsing() {
    assert_eq!("quic".parse::<TransportType>().unwrap(), TransportType::QuicOnly);
    assert_eq!("tcp".parse::<TransportType>().unwrap(), TransportType::TcpOnly);
    assert_eq!("dual".parse::<TransportType>().unwrap(), TransportType::DualStack);
    assert_eq!("QUIC-only".parse::<TransportType>().unwrap(), TransportType::QuicOnly);
    assert_eq!("TCP-only".parse::<TransportType>().unwrap(), TransportType::TcpOnly);
    assert_eq!("both".parse::<TransportType>().unwrap(), TransportType::DualStack);
    assert!("invalid".parse::<TransportType>().is_err());
}

#[test]
fn test_listen_address_generation() {
    let quic_addr = get_listen_address(TransportType::QuicOnly, 51820);
    assert!(quic_addr.contains("udp"));
    assert!(quic_addr.contains("quic-v1"));
    assert!(quic_addr.contains("51820"));
    
    let tcp_addr = get_listen_address(TransportType::TcpOnly, 51820);
    assert!(tcp_addr.contains("tcp"));
    assert!(tcp_addr.contains("51820"));
    
    let (dual_quic, dual_tcp) = get_dual_listen_addresses(8080);
    assert!(dual_quic.contains("udp"));
    assert!(dual_tcp.contains("tcp"));
}

#[tokio::test]
async fn test_quic_transport_creation() {
    let key = Keypair::generate_ed25519();
    let result = create_quic_transport(&key);
    assert!(result.is_ok(), "QUIC transport creation should succeed");
}

#[tokio::test]
async fn test_tcp_transport_creation() {
    let key = Keypair::generate_ed25519();
    let result = create_tcp_transport(&key);
    assert!(result.is_ok(), "TCP transport creation should succeed");
}

#[tokio::test]
async fn test_dual_transport_creation() {
    let key = Keypair::generate_ed25519();
    let result = create_dual_transport(&key);
    assert!(result.is_ok(), "Dual transport creation should succeed");
}

#[tokio::test]
async fn test_create_transport_all_types() {
    let key = Keypair::generate_ed25519();
    
    assert!(create_transport(&key, TransportType::QuicOnly).is_ok());
    assert!(create_transport(&key, TransportType::TcpOnly).is_ok());
    assert!(create_transport(&key, TransportType::DualStack).is_ok());
}

// ============================================================================
// INTEGRATION TESTS - TCP Swarm (Original Implementation)
// ============================================================================

#[derive(NetworkBehaviour)]
struct TestBehaviour {
    kademlia: kad::Behaviour<kad::store::MemoryStore>,
    request_response: request_response::Behaviour<JsonCodec>,
}

async fn create_tcp_swarm() -> (Swarm<TestBehaviour>, PeerId) {
    let key = Keypair::generate_ed25519();
    let peer_id = PeerId::from(key.public());
    
    let transport = create_tcp_transport(&key).unwrap();
    
    let store = kad::store::MemoryStore::new(peer_id);
    let kademlia = kad::Behaviour::new(peer_id, store);
    
    let codec = JsonCodec;
    let request_response = request_response::Behaviour::with_codec(
        codec,
        [(StreamProtocol::new("/json-message/1.0"), ProtocolSupport::Full)],
        request_response::Config::default(),
    );
    
    let behaviour = TestBehaviour { kademlia, request_response };
    
    let swarm_config = SwarmConfig::with_tokio_executor()
        .with_idle_connection_timeout(Duration::from_secs(30));
    
    let swarm = Swarm::new(transport, behaviour, peer_id, swarm_config);
    
    (swarm, peer_id)
}

#[tokio::test]
async fn test_tcp_swarm_listen() {
    let (mut swarm, _peer_id) = create_tcp_swarm().await;
    
    // Listen on random TCP port
    let addr: Multiaddr = "/ip4/127.0.0.1/tcp/0".parse().unwrap();
    let result = swarm.listen_on(addr);
    assert!(result.is_ok(), "TCP swarm should listen successfully");
    
    // Wait for listen event
    let listen_result = timeout(Duration::from_secs(5), async {
        loop {
            match swarm.select_next_some().await {
                SwarmEvent::NewListenAddr { address, .. } => {
                    return Some(address);
                }
                _ => continue,
            }
        }
    }).await;
    
    assert!(listen_result.is_ok(), "Should receive listen address within timeout");
    let addr = listen_result.unwrap();
    assert!(addr.is_some(), "Should have a listen address");
    let addr = addr.unwrap();
    assert!(addr.to_string().contains("tcp"), "Should be a TCP address");
}

// ============================================================================
// INTEGRATION TESTS - QUIC Swarm (New Implementation)
// ============================================================================

async fn create_quic_swarm() -> (Swarm<TestBehaviour>, PeerId) {
    let key = Keypair::generate_ed25519();
    let peer_id = PeerId::from(key.public());
    
    let transport = create_quic_transport(&key).unwrap();
    
    let store = kad::store::MemoryStore::new(peer_id);
    let kademlia = kad::Behaviour::new(peer_id, store);
    
    let codec = JsonCodec;
    let request_response = request_response::Behaviour::with_codec(
        codec,
        [(StreamProtocol::new("/json-message/1.0"), ProtocolSupport::Full)],
        request_response::Config::default(),
    );
    
    let behaviour = TestBehaviour { kademlia, request_response };
    
    let swarm_config = SwarmConfig::with_tokio_executor()
        .with_idle_connection_timeout(Duration::from_secs(30));
    
    let swarm = Swarm::new(transport, behaviour, peer_id, swarm_config);
    
    (swarm, peer_id)
}

#[tokio::test]
async fn test_quic_swarm_listen() {
    let (mut swarm, _peer_id) = create_quic_swarm().await;
    
    // Listen on random QUIC port
    let addr: Multiaddr = "/ip4/127.0.0.1/udp/0/quic-v1".parse().unwrap();
    let result = swarm.listen_on(addr);
    assert!(result.is_ok(), "QUIC swarm should listen successfully");
    
    // Wait for listen event
    let listen_result = timeout(Duration::from_secs(5), async {
        loop {
            match swarm.select_next_some().await {
                SwarmEvent::NewListenAddr { address, .. } => {
                    return Some(address);
                }
                _ => continue,
            }
        }
    }).await;
    
    assert!(listen_result.is_ok(), "Should receive listen address within timeout");
    let addr = listen_result.unwrap();
    assert!(addr.is_some(), "Should have a listen address");
    let addr = addr.unwrap();
    assert!(addr.to_string().contains("quic"), "Should be a QUIC address");
}

// ============================================================================
// INTEGRATION TESTS - Dual-Stack Swarm
// ============================================================================

async fn create_dual_swarm() -> (Swarm<TestBehaviour>, PeerId) {
    let key = Keypair::generate_ed25519();
    let peer_id = PeerId::from(key.public());
    
    let transport = create_dual_transport(&key).unwrap();
    
    let store = kad::store::MemoryStore::new(peer_id);
    let kademlia = kad::Behaviour::new(peer_id, store);
    
    let codec = JsonCodec;
    let request_response = request_response::Behaviour::with_codec(
        codec,
        [(StreamProtocol::new("/json-message/1.0"), ProtocolSupport::Full)],
        request_response::Config::default(),
    );
    
    let behaviour = TestBehaviour { kademlia, request_response };
    
    let swarm_config = SwarmConfig::with_tokio_executor()
        .with_idle_connection_timeout(Duration::from_secs(30));
    
    let swarm = Swarm::new(transport, behaviour, peer_id, swarm_config);
    
    (swarm, peer_id)
}

#[tokio::test]
async fn test_dual_swarm_listen_quic() {
    let (mut swarm, _peer_id) = create_dual_swarm().await;
    
    // Dual transport should be able to listen on QUIC
    let addr: Multiaddr = "/ip4/127.0.0.1/udp/0/quic-v1".parse().unwrap();
    let result = swarm.listen_on(addr);
    assert!(result.is_ok(), "Dual swarm should listen on QUIC");
    
    // Wait for listen event
    let listen_result = timeout(Duration::from_secs(5), async {
        loop {
            match swarm.select_next_some().await {
                SwarmEvent::NewListenAddr { address, .. } => {
                    return Some(address);
                }
                _ => continue,
            }
        }
    }).await;
    
    assert!(listen_result.is_ok());
    assert!(listen_result.unwrap().is_some());
}

#[tokio::test]
async fn test_dual_swarm_listen_tcp() {
    let (mut swarm, _peer_id) = create_dual_swarm().await;
    
    // Dual transport should be able to listen on TCP
    let addr: Multiaddr = "/ip4/127.0.0.1/tcp/0".parse().unwrap();
    let result = swarm.listen_on(addr);
    assert!(result.is_ok(), "Dual swarm should listen on TCP");
    
    // Wait for listen event
    let listen_result = timeout(Duration::from_secs(5), async {
        loop {
            match swarm.select_next_some().await {
                SwarmEvent::NewListenAddr { address, .. } => {
                    return Some(address);
                }
                _ => continue,
            }
        }
    }).await;
    
    assert!(listen_result.is_ok());
    assert!(listen_result.unwrap().is_some());
}

// ============================================================================
// INTEGRATION TESTS - Peer-to-Peer Connection
// ============================================================================

#[tokio::test]
async fn test_tcp_peer_connection() {
    let (mut swarm1, peer_id1) = create_tcp_swarm().await;
    let (mut swarm2, peer_id2) = create_tcp_swarm().await;
    
    // Swarm 1 listens
    swarm1.listen_on("/ip4/127.0.0.1/tcp/0".parse().unwrap()).unwrap();
    
    // Wait for swarm1 to get a listen address
    let addr1 = timeout(Duration::from_secs(5), async {
        loop {
            match swarm1.select_next_some().await {
                SwarmEvent::NewListenAddr { address, .. } => {
                    return address;
                }
                _ => continue,
            }
        }
    }).await.unwrap();
    
    // Swarm 2 dials swarm 1
    swarm2.dial(addr1.clone()).unwrap();
    
    // Wait for connection on both ends
    let connected = timeout(Duration::from_secs(10), async {
        let mut swarm1_connected = false;
        let mut swarm2_connected = false;
        
        loop {
            tokio::select! {
                event = swarm1.select_next_some() => {
                    if let SwarmEvent::ConnectionEstablished { peer_id, .. } = event {
                        if peer_id == peer_id2 {
                            swarm1_connected = true;
                        }
                    }
                }
                event = swarm2.select_next_some() => {
                    if let SwarmEvent::ConnectionEstablished { peer_id, .. } = event {
                        if peer_id == peer_id1 {
                            swarm2_connected = true;
                        }
                    }
                }
            }
            
            if swarm1_connected && swarm2_connected {
                return true;
            }
        }
    }).await;
    
    assert!(connected.is_ok(), "TCP peers should connect within timeout");
    assert!(connected.unwrap(), "Both peers should be connected");
}

#[tokio::test]
async fn test_quic_peer_connection() {
    let (mut swarm1, peer_id1) = create_quic_swarm().await;
    let (mut swarm2, peer_id2) = create_quic_swarm().await;
    
    // Swarm 1 listens
    swarm1.listen_on("/ip4/127.0.0.1/udp/0/quic-v1".parse().unwrap()).unwrap();
    
    // Wait for swarm1 to get a listen address
    let addr1 = timeout(Duration::from_secs(5), async {
        loop {
            match swarm1.select_next_some().await {
                SwarmEvent::NewListenAddr { address, .. } => {
                    return address;
                }
                _ => continue,
            }
        }
    }).await.unwrap();
    
    // Swarm 2 dials swarm 1
    swarm2.dial(addr1.clone()).unwrap();
    
    // Wait for connection on both ends
    let connected = timeout(Duration::from_secs(10), async {
        let mut swarm1_connected = false;
        let mut swarm2_connected = false;
        
        loop {
            tokio::select! {
                event = swarm1.select_next_some() => {
                    if let SwarmEvent::ConnectionEstablished { peer_id, .. } = event {
                        if peer_id == peer_id2 {
                            swarm1_connected = true;
                        }
                    }
                }
                event = swarm2.select_next_some() => {
                    if let SwarmEvent::ConnectionEstablished { peer_id, .. } = event {
                        if peer_id == peer_id1 {
                            swarm2_connected = true;
                        }
                    }
                }
            }
            
            if swarm1_connected && swarm2_connected {
                return true;
            }
        }
    }).await;
    
    assert!(connected.is_ok(), "QUIC peers should connect within timeout");
    assert!(connected.unwrap(), "Both QUIC peers should be connected");
}

// ============================================================================
// INTEGRATION TESTS - Request/Response over Both Transports
// ============================================================================

#[tokio::test]
async fn test_tcp_request_response() {
    let (mut swarm1, peer_id1) = create_tcp_swarm().await;
    let (mut swarm2, _peer_id2) = create_tcp_swarm().await;
    
    // Swarm 1 listens
    swarm1.listen_on("/ip4/127.0.0.1/tcp/0".parse().unwrap()).unwrap();
    
    // Wait for listen address and connect
    let addr1 = timeout(Duration::from_secs(5), async {
        loop {
            match swarm1.select_next_some().await {
                SwarmEvent::NewListenAddr { address, .. } => return address,
                _ => continue,
            }
        }
    }).await.unwrap();
    
    swarm2.dial(addr1).unwrap();
    
    // Wait for both connections to be established
    let mut swarm1_connected = false;
    let mut swarm2_connected = false;
    
    timeout(Duration::from_secs(10), async {
        loop {
            tokio::select! {
                event = swarm1.select_next_some() => {
                    if matches!(event, SwarmEvent::ConnectionEstablished { .. }) {
                        swarm1_connected = true;
                    }
                }
                event = swarm2.select_next_some() => {
                    if matches!(event, SwarmEvent::ConnectionEstablished { .. }) {
                        swarm2_connected = true;
                    }
                }
            }
            if swarm1_connected && swarm2_connected {
                break;
            }
        }
    }).await.unwrap();
    
    // Small delay to ensure connection is fully ready
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    // Send a request from swarm2 to swarm1
    let request = JsonMessage::new("test-client".to_string(), "Hello via TCP!".to_string());
    swarm2.behaviour_mut().request_response.send_request(&peer_id1, request);
    
    // Wait for request on swarm1, while also processing swarm2 events
    let received = timeout(Duration::from_secs(15), async {
        loop {
            tokio::select! {
                event = swarm1.select_next_some() => {
                    if let SwarmEvent::Behaviour(TestBehaviourEvent::RequestResponse(
                        request_response::Event::Message { message, .. }
                    )) = event {
                        if let request_response::Message::Request { request, .. } = message {
                            return request.message;
                        }
                    }
                }
                event = swarm2.select_next_some() => {
                    // Process swarm2 events to keep the connection alive
                    let _ = event;
                }
            }
        }
    }).await;
    
    assert!(received.is_ok(), "Should receive message within timeout");
    assert_eq!(received.unwrap(), "Hello via TCP!", "Message content should match");
}

#[tokio::test]
async fn test_quic_request_response() {
    let (mut swarm1, peer_id1) = create_quic_swarm().await;
    let (mut swarm2, peer_id2) = create_quic_swarm().await;
    
    // Swarm 1 listens on QUIC
    swarm1.listen_on("/ip4/127.0.0.1/udp/0/quic-v1".parse().unwrap()).unwrap();
    
    // Wait for listen address
    let addr1 = timeout(Duration::from_secs(5), async {
        loop {
            match swarm1.select_next_some().await {
                SwarmEvent::NewListenAddr { address, .. } => return address,
                _ => continue,
            }
        }
    }).await.unwrap();
    
    swarm2.dial(addr1).unwrap();
    
    // Wait for connection
    timeout(Duration::from_secs(10), async {
        loop {
            tokio::select! {
                event = swarm1.select_next_some() => {
                    if matches!(event, SwarmEvent::ConnectionEstablished { .. }) {
                        break;
                    }
                }
                event = swarm2.select_next_some() => {
                    if matches!(event, SwarmEvent::ConnectionEstablished { .. }) {
                        break;
                    }
                }
            }
        }
    }).await.unwrap();
    
    // Send a request from swarm2 to swarm1 via QUIC
    let request = JsonMessage::new("test-client".to_string(), "Hello via QUIC!".to_string());
    swarm2.behaviour_mut().request_response.send_request(&peer_id1, request);
    
    // Wait for request on swarm1
    let received = timeout(Duration::from_secs(10), async {
        loop {
            tokio::select! {
                event = swarm1.select_next_some() => {
                    if let SwarmEvent::Behaviour(TestBehaviourEvent::RequestResponse(
                        request_response::Event::Message { message, .. }
                    )) = event {
                        if let request_response::Message::Request { request, .. } = message {
                            return request.message;
                        }
                    }
                }
                _ = swarm2.select_next_some() => {}
            }
        }
    }).await;
    
    assert!(received.is_ok(), "Should receive QUIC message within timeout");
    assert_eq!(received.unwrap(), "Hello via QUIC!", "QUIC message content should match");
}

// ============================================================================
// STRESS TESTS
// ============================================================================

#[tokio::test]
async fn test_multiple_messages_tcp() {
    let (mut swarm1, peer_id1) = create_tcp_swarm().await;
    let (mut swarm2, _peer_id2) = create_tcp_swarm().await;
    
    swarm1.listen_on("/ip4/127.0.0.1/tcp/0".parse().unwrap()).unwrap();
    
    let addr1 = timeout(Duration::from_secs(5), async {
        loop {
            match swarm1.select_next_some().await {
                SwarmEvent::NewListenAddr { address, .. } => return address,
                _ => continue,
            }
        }
    }).await.unwrap();
    
    swarm2.dial(addr1).unwrap();
    
    // Wait for both connections
    let mut swarm1_connected = false;
    let mut swarm2_connected = false;
    
    timeout(Duration::from_secs(10), async {
        loop {
            tokio::select! {
                event = swarm1.select_next_some() => {
                    if matches!(event, SwarmEvent::ConnectionEstablished { .. }) {
                        swarm1_connected = true;
                    }
                }
                event = swarm2.select_next_some() => {
                    if matches!(event, SwarmEvent::ConnectionEstablished { .. }) {
                        swarm2_connected = true;
                    }
                }
            }
            if swarm1_connected && swarm2_connected {
                break;
            }
        }
    }).await.unwrap();
    
    // Small delay
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    // Send 10 messages
    for i in 0..10 {
        let request = JsonMessage::new("test".to_string(), format!("Message {}", i));
        swarm2.behaviour_mut().request_response.send_request(&peer_id1, request);
    }
    
    // Count received messages
    let count = timeout(Duration::from_secs(20), async {
        let mut count = 0;
        loop {
            tokio::select! {
                event = swarm1.select_next_some() => {
                    if let SwarmEvent::Behaviour(TestBehaviourEvent::RequestResponse(
                        request_response::Event::Message { message, .. }
                    )) = event {
                        if matches!(message, request_response::Message::Request { .. }) {
                            count += 1;
                            if count >= 10 {
                                return count;
                            }
                        }
                    }
                }
                event = swarm2.select_next_some() => {
                    let _ = event;
                }
            }
        }
    }).await;
    
    assert!(count.is_ok(), "Should receive all messages");
    assert_eq!(count.unwrap(), 10, "Should receive exactly 10 messages");
}

#[tokio::test]
async fn test_multiple_messages_quic() {
    let (mut swarm1, peer_id1) = create_quic_swarm().await;
    let (mut swarm2, _peer_id2) = create_quic_swarm().await;
    
    swarm1.listen_on("/ip4/127.0.0.1/udp/0/quic-v1".parse().unwrap()).unwrap();
    
    let addr1 = timeout(Duration::from_secs(5), async {
        loop {
            match swarm1.select_next_some().await {
                SwarmEvent::NewListenAddr { address, .. } => return address,
                _ => continue,
            }
        }
    }).await.unwrap();
    
    swarm2.dial(addr1).unwrap();
    
    // Wait for connection
    timeout(Duration::from_secs(10), async {
        loop {
            tokio::select! {
                event = swarm1.select_next_some() => {
                    if matches!(event, SwarmEvent::ConnectionEstablished { .. }) {
                        break;
                    }
                }
                event = swarm2.select_next_some() => {
                    if matches!(event, SwarmEvent::ConnectionEstablished { .. }) {
                        break;
                    }
                }
            }
        }
    }).await.unwrap();
    
    // Send 10 messages via QUIC
    for i in 0..10 {
        let request = JsonMessage::new("test".to_string(), format!("QUIC Message {}", i));
        swarm2.behaviour_mut().request_response.send_request(&peer_id1, request);
    }
    
    // Count received messages
    let count = timeout(Duration::from_secs(15), async {
        let mut count = 0;
        loop {
            tokio::select! {
                event = swarm1.select_next_some() => {
                    if let SwarmEvent::Behaviour(TestBehaviourEvent::RequestResponse(
                        request_response::Event::Message { message, .. }
                    )) = event {
                        if matches!(message, request_response::Message::Request { .. }) {
                            count += 1;
                            if count >= 10 {
                                return count;
                            }
                        }
                    }
                }
                _ = swarm2.select_next_some() => {}
            }
        }
    }).await;
    
    assert!(count.is_ok(), "Should receive all QUIC messages");
    assert_eq!(count.unwrap(), 10, "Should receive exactly 10 QUIC messages");
}

// ============================================================================
// REGRESSION TESTS - Ensure TCP still works after QUIC addition
// ============================================================================

#[tokio::test]
async fn test_tcp_not_broken_by_quic_addition() {
    // This test specifically verifies that adding QUIC support
    // has not broken the existing TCP implementation
    
    let key1 = Keypair::generate_ed25519();
    let key2 = Keypair::generate_ed25519();
    
    // Create TCP transports (original method)
    let transport1 = create_tcp_transport(&key1);
    let transport2 = create_tcp_transport(&key2);
    
    assert!(transport1.is_ok(), "TCP transport 1 should still work");
    assert!(transport2.is_ok(), "TCP transport 2 should still work");
    
    // Verify we can create multiple transports
    for _ in 0..5 {
        let key = Keypair::generate_ed25519();
        let transport = create_tcp_transport(&key);
        assert!(transport.is_ok(), "TCP transport creation should be repeatable");
    }
}

#[tokio::test]
async fn test_quic_parallel_to_tcp() {
    // Test that QUIC and TCP can be created in parallel without interference
    let key = Keypair::generate_ed25519();
    
    let tcp = create_tcp_transport(&key);
    let quic = create_quic_transport(&key);
    
    assert!(tcp.is_ok(), "TCP should work");
    assert!(quic.is_ok(), "QUIC should work");
}

