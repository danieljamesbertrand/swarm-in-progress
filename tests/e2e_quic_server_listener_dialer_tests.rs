//! QUIC-only integration test for the classic 3-node topology:
//! - **server**: rendezvous/bootstrap registry
//! - **listener**: registers itself and answers AI questions
//! - **dialer**: discovers listener via server, then asks: "Why is the sky blue?"
//!
//! This is intentionally deterministic and does not rely on DHT record propagation timing.

use libp2p::{
    identity,
    request_response::{self, ProtocolSupport},
    swarm::{NetworkBehaviour, Swarm, SwarmEvent},
    Multiaddr, PeerId, StreamProtocol,
};
use libp2p::swarm::Config as SwarmConfig;
use libp2p::futures::StreamExt;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;
use tokio::time::timeout;

use punch_simple::{
    JsonCodec, JsonMessage, create_quic_transport,
    ai_inference_handler::{AIInferenceRequest, process_ai_inference},
    command_protocol::{Command, CommandResponse, commands},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PeerDiscoveryRecord {
    peer_id: String,
    addrs: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
enum RegistryMsg {
    #[serde(rename = "register")]
    Register { namespace: String, record: PeerDiscoveryRecord },
    #[serde(rename = "lookup")]
    Lookup { namespace: String },
    #[serde(rename = "lookup_result")]
    LookupResult { namespace: String, records: Vec<PeerDiscoveryRecord> },
    #[serde(rename = "ack")]
    Ack { ok: bool },
}

#[derive(NetworkBehaviour)]
struct RRBehaviour {
    request_response: request_response::Behaviour<JsonCodec>,
}

async fn create_quic_rr_swarm() -> (Swarm<RRBehaviour>, PeerId) {
    let key = identity::Keypair::generate_ed25519();
    let peer_id = PeerId::from(key.public());
    let transport = create_quic_transport(&key).expect("quic transport");

    let request_response = request_response::Behaviour::with_codec(
        JsonCodec,
        [(StreamProtocol::new("/json-message/1.0"), ProtocolSupport::Full)],
        request_response::Config::default(),
    );

    let behaviour = RRBehaviour { request_response };
    let swarm_config = SwarmConfig::with_tokio_executor()
        .with_idle_connection_timeout(Duration::from_secs(30));
    (Swarm::new(transport, behaviour, peer_id, swarm_config), peer_id)
}

#[tokio::test]
async fn test_e2e_quic_server_listener_dialer_question() {
    timeout(Duration::from_secs(20), async {
        let namespace = "simple-chat".to_string();
        let prompt = "Why is the sky blue?".to_string();

        let (mut server, server_id) = create_quic_rr_swarm().await;
        let (mut listener, listener_id) = create_quic_rr_swarm().await;
        let (mut dialer, dialer_id) = create_quic_rr_swarm().await;

        server
            .listen_on("/ip4/127.0.0.1/udp/0/quic-v1".parse::<Multiaddr>().unwrap())
            .unwrap();
        listener
            .listen_on("/ip4/127.0.0.1/udp/0/quic-v1".parse::<Multiaddr>().unwrap())
            .unwrap();
        dialer
            .listen_on("/ip4/127.0.0.1/udp/0/quic-v1".parse::<Multiaddr>().unwrap())
            .unwrap();

        let mut server_addr: Option<Multiaddr> = None;
        let mut listener_addrs: Vec<String> = Vec::new();

        // Server registry state (namespace → records)
        let mut registry: HashMap<String, Vec<PeerDiscoveryRecord>> = HashMap::new();

        // Listener state
        let mut listener_connected_to_server = false;
        let mut listener_registered = false;

        // Dialer state
        let mut dialer_lookup_sent = false;
        let mut dialer_dialed_listener = false;
        let mut dialer_ai_sent = false;
        let mut pending_request_id: Option<String> = None;

        loop {
            tokio::select! {
                event = server.select_next_some() => {
                    match event {
                        SwarmEvent::NewListenAddr { address, .. } => {
                            if server_addr.is_none() {
                                server_addr = Some(address);
                                // Connect both listener and dialer to the rendezvous server.
                                listener.dial(server_addr.clone().unwrap()).unwrap();
                                dialer.dial(server_addr.clone().unwrap()).unwrap();
                            }
                        }
                        SwarmEvent::Behaviour(RRBehaviourEvent::RequestResponse(
                            request_response::Event::Message { peer, message, .. }
                        )) => {
                            if let request_response::Message::Request { request, channel, .. } = message {
                                if let Ok(msg) = serde_json::from_str::<RegistryMsg>(&request.message) {
                                    match msg {
                                        RegistryMsg::Register { namespace, record } => {
                                            registry.entry(namespace).or_default().push(record);
                                            let ack = RegistryMsg::Ack { ok: true };
                                            let resp = JsonMessage::new(server_id.to_string(), serde_json::to_string(&ack).unwrap());
                                            let _ = server.behaviour_mut().request_response.send_response(channel, resp);
                                        }
                                        RegistryMsg::Lookup { namespace } => {
                                            let records = registry.get(&namespace).cloned().unwrap_or_default();
                                            let res = RegistryMsg::LookupResult { namespace, records };
                                            let resp = JsonMessage::new(server_id.to_string(), serde_json::to_string(&res).unwrap());
                                            let _ = server.behaviour_mut().request_response.send_response(channel, resp);
                                        }
                                        _ => {}
                                    }
                                } else {
                                    // Ignore unknown requests on server
                                    let ack = RegistryMsg::Ack { ok: false };
                                    let resp = JsonMessage::new(server_id.to_string(), serde_json::to_string(&ack).unwrap());
                                    let _ = server.behaviour_mut().request_response.send_response(channel, resp);
                                }
                            }
                            let _ = peer;
                        }
                        _ => {}
                    }
                }

                event = listener.select_next_some() => {
                    match event {
                        SwarmEvent::NewListenAddr { address, .. } => {
                            listener_addrs.push(address.to_string());
                        }
                        SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                            if peer_id == server_id {
                                listener_connected_to_server = true;
                            }
                        }
                        SwarmEvent::Behaviour(RRBehaviourEvent::RequestResponse(
                            request_response::Event::Message { message, .. }
                        )) => {
                            match message {
                                request_response::Message::Request { request, channel, .. } => {
                                    // Listener answers AI questions via Command/CommandResponse.
                                    if let Ok(cmd) = Command::from_json(&request.message) {
                                        if cmd.command == commands::EXECUTE_TASK {
                                            if let Ok(ai_req) = AIInferenceRequest::from_command(&cmd) {
                                                let result = process_ai_inference(&ai_req).await.unwrap();
                                                let mut response_data = HashMap::new();
                                                if let Some(output) = result.get("output") {
                                                    response_data.insert("output".to_string(), output.clone());
                                                }
                                                let resp = CommandResponse::success(
                                                    &cmd.command,
                                                    &cmd.request_id,
                                                    &listener_id.to_string(),
                                                    &cmd.from,
                                                    response_data,
                                                );
                                                let msg = JsonMessage::new(listener_id.to_string(), resp.to_json().unwrap());
                                                let _ = listener.behaviour_mut().request_response.send_response(channel, msg);
                                            }
                                        }
                                    }
                                }
                                _ => {}
                            }
                        }
                        _ => {}
                    }

                    // Register once we have at least one listen addr and are connected to server.
                    if listener_connected_to_server && !listener_registered && !listener_addrs.is_empty() {
                        let record = PeerDiscoveryRecord {
                            peer_id: listener_id.to_string(),
                            addrs: listener_addrs.clone(),
                        };
                        let reg = RegistryMsg::Register {
                            namespace: namespace.clone(),
                            record,
                        };
                        let msg = JsonMessage::new(listener_id.to_string(), serde_json::to_string(&reg).unwrap());
                        listener.behaviour_mut().request_response.send_request(&server_id, msg);
                        listener_registered = true;
                    }
                }

                event = dialer.select_next_some() => {
                    match event {
                        SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                            if peer_id == server_id && !dialer_lookup_sent {
                                // Ask rendezvous server for peers in namespace.
                                let lookup = RegistryMsg::Lookup { namespace: namespace.clone() };
                                let msg = JsonMessage::new(dialer_id.to_string(), serde_json::to_string(&lookup).unwrap());
                                dialer.behaviour_mut().request_response.send_request(&server_id, msg);
                                dialer_lookup_sent = true;
                            }

                            if peer_id == listener_id && dialer_dialed_listener && !dialer_ai_sent {
                                // Send AI question to listener only once we are connected.
                                let mut cmd = Command::new(
                                    commands::EXECUTE_TASK,
                                    &dialer_id.to_string(),
                                    Some(&listener_id.to_string()),
                                );
                                cmd.params.insert("task_type".to_string(), serde_json::json!("ai_inference"));
                                cmd.params.insert("model_name".to_string(), serde_json::json!("mock"));
                                cmd.params.insert("input_data".to_string(), serde_json::json!(prompt));
                                pending_request_id = Some(cmd.request_id.clone());
                                let msg = JsonMessage::new(dialer_id.to_string(), cmd.to_json().unwrap());
                                dialer.behaviour_mut().request_response.send_request(&listener_id, msg);
                                dialer_ai_sent = true;
                            }
                        }
                        SwarmEvent::Behaviour(RRBehaviourEvent::RequestResponse(
                            request_response::Event::Message { message, .. }
                        )) => {
                            match message {
                                request_response::Message::Response { response, .. } => {
                                    // First: lookup result from server. Then: CommandResponse from listener.
                                    if let Ok(reg) = serde_json::from_str::<RegistryMsg>(&response.message) {
                                        if let RegistryMsg::LookupResult { records, .. } = reg {
                                            if let Some(rec) = records.into_iter().find(|r| r.peer_id == listener_id.to_string()) {
                                                let addr_str = rec
                                                    .addrs
                                                    .into_iter()
                                                    .find(|a| a.contains("quic-v1"))
                                                    .expect("listener has quic addr");
                                                let addr: Multiaddr = addr_str.parse().unwrap();
                                                dialer.dial(addr).unwrap();
                                                dialer_dialed_listener = true;
                                            } else {
                                                // Not registered yet; retry lookup.
                                                let lookup = RegistryMsg::Lookup { namespace: namespace.clone() };
                                                let msg = JsonMessage::new(dialer_id.to_string(), serde_json::to_string(&lookup).unwrap());
                                                dialer.behaviour_mut().request_response.send_request(&server_id, msg);
                                            }
                                        }
                                    } else if let Ok(cmd_resp) = CommandResponse::from_json(&response.message) {
                                        // Validate request_id and content.
                                        assert_eq!(Some(cmd_resp.request_id.clone()), pending_request_id);
                                        let output = cmd_resp
                                            .result
                                            .and_then(|m| m.get("output").cloned())
                                            .and_then(|v| v.as_str().map(|s| s.to_string()))
                                            .unwrap_or_default();
                                        let out_l = output.to_lowercase();
                                        assert!(out_l.contains("rayleigh"));
                                        assert!(out_l.contains("scatter"));
                                        assert!(out_l.contains("wavelength"));
                                        return;
                                    }
                                }
                                _ => {}
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    })
    .await
    .expect("test should complete within timeout");
}

