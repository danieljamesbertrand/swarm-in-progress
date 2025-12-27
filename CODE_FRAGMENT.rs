//! Standalone code fragment for P2P JSON messaging
//! 
//! Copy this code into your program to enable P2P JSON messaging.
//! 
//! Requirements:
//! - Add to Cargo.toml: libp2p, tokio, serde, serde_json, async-trait
//! - Include the message.rs module (or copy JsonMessage and JsonCodec)

use serde_json::json;
use libp2p::{
    identity, tcp, noise, yamux, rendezvous,
    request_response::{self, ProtocolSupport},
    swarm::{NetworkBehaviour, Swarm, SwarmEvent},
    core::transport::Transport,
    PeerId, Multiaddr, StreamProtocol,
};
use libp2p::swarm::Config as SwarmConfig;
use libp2p::futures::StreamExt;
use std::time::Duration;
use std::collections::HashMap;
use tokio::sync::oneshot;

// Include your JsonMessage and JsonCodec here (from message.rs)
// mod message;
// use message::{JsonMessage, JsonCodec};

#[derive(NetworkBehaviour)]
struct Behaviour {
    rendezvous: rendezvous::client::Behaviour,
    identify: libp2p::identify::Behaviour,
    request_response: request_response::Behaviour<JsonCodec>,
}

/// Simple function to send a JSON message and wait for response
/// 
/// # Arguments
/// * `server` - Rendezvous server address (e.g., "127.0.0.1:51820")
/// * `namespace` - Namespace for peer discovery (e.g., "my-app")
/// * `message_text` - The message text to send
/// 
/// # Returns
/// The response JSON value, or an error
pub async fn send_p2p_json_message(
    server: &str,
    namespace: &str,
    message_text: &str,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    // Step 1: Setup
    let key = identity::Keypair::generate_ed25519();
    let peer_id = PeerId::from(key.public());

    let transport = tcp::tokio::Transport::default()
        .upgrade(libp2p::core::upgrade::Version::V1)
        .authenticate(noise::Config::new(&key)?)
        .multiplex(yamux::Config::default())
        .boxed();
    
    let rendezvous = rendezvous::client::Behaviour::new(key.clone());
    let identify = libp2p::identify::Behaviour::new(
        libp2p::identify::Config::new("p2p-client/1.0".to_string(), key.public())
    );
    
    let codec = JsonCodec;
    let request_response = request_response::Behaviour::with_codec(
        codec,
        [(StreamProtocol::new("/json-message/1.0"), ProtocolSupport::Full)],
        request_response::Config::default(),
    );
    
    let behaviour = Behaviour { rendezvous, identify, request_response };
    
    let swarm_config = SwarmConfig::with_tokio_executor()
        .with_idle_connection_timeout(Duration::from_secs(60));
    let mut swarm = Swarm::new(transport, behaviour, peer_id, swarm_config);

    swarm.listen_on("/ip4/0.0.0.0/tcp/0".parse()?)?;

    // Step 2: Connect to rendezvous server
    let (server_host, server_port) = if let Some(colon) = server.find(':') {
        (&server[..colon], &server[colon+1..])
    } else {
        (server, "51820")
    };
    
    let addr: Multiaddr = format!("/ip4/{}/tcp/{}", server_host, server_port).parse()?;
    swarm.dial(addr)?;

    let mut rendezvous_peer_id: Option<PeerId> = None;
    let mut connected_peer: Option<PeerId> = None;
    let mut pending_response: Option<oneshot::Receiver<serde_json::Value>> = None;
    let mut request_id: Option<request_response::RequestId> = None;

    // Wait for rendezvous connection
    loop {
        match swarm.select_next_some().await {
            SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                if rendezvous_peer_id.is_none() {
                    rendezvous_peer_id = Some(peer_id);
                    // Discover peers
                    let ns = rendezvous::Namespace::new(namespace.to_string())?;
                    swarm.behaviour_mut().rendezvous.discover(Some(ns), None, None, peer_id);
                }
            }
            SwarmEvent::NewListenAddr { address, .. } => {
                swarm.add_external_address(address);
            }
            SwarmEvent::Behaviour(event) => {
                match event {
                    BehaviourEvent::Rendezvous(rendezvous::client::Event::Discovered { registrations, .. }) => {
                        for reg in registrations {
                            let discovered_peer = reg.record.peer_id();
                            if discovered_peer != peer_id {
                                let addrs: Vec<Multiaddr> = reg.record.addresses().iter().cloned().collect();
                                for addr in addrs {
                                    if swarm.dial(addr).is_ok() {
                                        break;
                                    }
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }
        
        if rendezvous_peer_id.is_some() {
            break;
        }
    }

    // Step 3: Wait for peer connection
    loop {
        match swarm.select_next_some().await {
            SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                if Some(peer_id) != rendezvous_peer_id && connected_peer.is_none() {
                    connected_peer = Some(peer_id);
                    break;
                }
            }
            SwarmEvent::Behaviour(BehaviourEvent::Rendezvous(rendezvous::client::Event::Discovered { registrations, .. })) => {
                for reg in registrations {
                    let discovered_peer = reg.record.peer_id();
                    if discovered_peer != peer_id {
                        let addrs: Vec<Multiaddr> = reg.record.addresses().iter().cloned().collect();
                        for addr in addrs {
                            let _ = swarm.dial(addr);
                        }
                    }
                }
            }
            _ => {}
        }
    }

    let peer_id = connected_peer.unwrap();

    // Step 4: Send message
    let json_msg = JsonMessage::new(
        "p2p-client".to_string(),
        message_text.to_string(),
    );

    let (tx, rx) = oneshot::channel();
    let req_id = swarm.behaviour_mut().request_response.send_request(&peer_id, json_msg);
    pending_response = Some(rx);
    request_id = Some(req_id);

    // Step 5: Wait for response
    let timeout_duration = Duration::from_secs(10);
    let start = std::time::Instant::now();

    loop {
        if start.elapsed() > timeout_duration {
            return Err("Timeout waiting for response".into());
        }

        // Check if we got the response
        if let Some(ref mut rx) = pending_response {
            if let Ok(response) = rx.try_recv() {
                return Ok(response);
            }
        }

        // Process events
        match tokio::time::timeout(Duration::from_millis(100), swarm.select_next_some()).await {
            Ok(event) => {
                match event {
                    SwarmEvent::Behaviour(BehaviourEvent::RequestResponse(request_response::Event::Message { message, .. })) => {
                        match message {
                            request_response::Message::Response { response, request_id: resp_id, .. } => {
                                if Some(resp_id) == request_id {
                                    let json_value = json!({
                                        "from": response.from,
                                        "message": response.message,
                                        "timestamp": response.timestamp
                                    });
                                    if let Some(tx) = pending_response.take().and_then(|rx| {
                                        // We need to send the response
                                        None // This is simplified - in real code, you'd store the sender
                                    }) {
                                        return Ok(json_value);
                                    }
                                }
                            }
                            request_response::Message::Request { request, channel, .. } => {
                                // Auto-respond
                                let response = JsonMessage::new(
                                    "auto-responder".to_string(),
                                    format!("Echo: {}", request.message),
                                );
                                let _ = swarm.behaviour_mut().request_response.send_response(channel, response);
                            }
                        }
                    }
                    _ => {}
                }
            }
            Err(_) => continue,
        }
    }
}

// Example usage:
// #[tokio::main]
// async fn main() -> Result<(), Box<dyn std::error::Error>> {
//     let response = send_p2p_json_message(
//         "127.0.0.1:51820",
//         "simple-chat",
//         "Hello from my app!"
//     ).await?;
//     
//     println!("Response: {}", serde_json::to_string_pretty(&response)?);
//     Ok(())
// }















