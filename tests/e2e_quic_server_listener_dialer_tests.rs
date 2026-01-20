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

fn trace_enabled() -> bool {
    // Opt-in detailed trace output:
    // - Linux/macOS: `PUNCH_TRACE=1 cargo test ... -- --nocapture`
    // - Windows (PowerShell): `$env:PUNCH_TRACE=1; cargo test ... -- --nocapture`
    match std::env::var("PUNCH_TRACE") {
        Ok(v) => v != "0" && !v.trim().is_empty(),
        Err(_) => false,
    }
}

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
        let trace = trace_enabled();
        let mut step: u32 = 0;

        let namespace = "simple-chat".to_string();
        let prompt = "Why is the sky blue?".to_string();

        let (mut server, server_id) = create_quic_rr_swarm().await;
        let (mut listener, listener_id) = create_quic_rr_swarm().await;
        let (mut dialer, dialer_id) = create_quic_rr_swarm().await;

        if trace {
            step += 1;
            println!("\n[TRACE {:02}] Test start", step);
            println!("  namespace: {}", namespace);
            println!("  prompt: {}", prompt);
            println!("  server_id:   {}", server_id);
            println!("  listener_id: {}", listener_id);
            println!("  dialer_id:   {}", dialer_id);
        }

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
                                if trace {
                                    step += 1;
                                    println!("\n[TRACE {:02}] Server is listening", step);
                                    println!("  server_addr: {}", server_addr.as_ref().unwrap());
                                    println!("  action: dialer + listener dial server");
                                }
                                // Connect both listener and dialer to the rendezvous server.
                                listener.dial(server_addr.clone().unwrap()).unwrap();
                                dialer.dial(server_addr.clone().unwrap()).unwrap();
                            }
                        }
                        SwarmEvent::Behaviour(RRBehaviourEvent::RequestResponse(
                            request_response::Event::Message { peer, message, .. }
                        )) => {
                            if let request_response::Message::Request { request, channel, .. } = message {
                                if trace {
                                    step += 1;
                                    println!("\n[TRACE {:02}] Server received request", step);
                                    println!("  from_peer: {}", peer);
                                    println!("  request.from: {}", request.from);
                                    println!("  request.message: {}", request.message);
                                }
                                if let Ok(msg) = serde_json::from_str::<RegistryMsg>(&request.message) {
                                    match msg {
                                        RegistryMsg::Register { namespace, record } => {
                                            if trace {
                                                step += 1;
                                                println!("\n[TRACE {:02}] Server handling register", step);
                                                println!("  namespace: {}", namespace);
                                                println!("  record.peer_id: {}", record.peer_id);
                                                println!("  record.addrs: {:?}", record.addrs);
                                            }
                                            registry.entry(namespace).or_default().push(record);
                                            let ack = RegistryMsg::Ack { ok: true };
                                            let resp = JsonMessage::new(server_id.to_string(), serde_json::to_string(&ack).unwrap());
                                            let _ = server.behaviour_mut().request_response.send_response(channel, resp);
                                        }
                                        RegistryMsg::Lookup { namespace } => {
                                            let records = registry.get(&namespace).cloned().unwrap_or_default();
                                            if trace {
                                                step += 1;
                                                println!("\n[TRACE {:02}] Server handling lookup", step);
                                                println!("  namespace: {}", namespace);
                                                println!("  records_found: {}", records.len());
                                            }
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
                        }
                        _ => {}
                    }
                }

                event = listener.select_next_some() => {
                    match event {
                        SwarmEvent::NewListenAddr { address, .. } => {
                            listener_addrs.push(address.to_string());
                            if trace {
                                step += 1;
                                println!("\n[TRACE {:02}] Listener is listening", step);
                                println!("  listener_addr: {}", address);
                            }
                        }
                        SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                            if peer_id == server_id {
                                listener_connected_to_server = true;
                                if trace {
                                    step += 1;
                                    println!("\n[TRACE {:02}] Listener connected to server", step);
                                    println!("  server_id: {}", server_id);
                                }
                            }
                        }
                        SwarmEvent::Behaviour(RRBehaviourEvent::RequestResponse(
                            request_response::Event::Message { message, .. }
                        )) => {
                            match message {
                                request_response::Message::Request { request, channel, .. } => {
                                    // Listener answers AI questions via Command/CommandResponse.
                                    if let Ok(cmd) = Command::from_json(&request.message) {
                                        if trace {
                                            step += 1;
                                            println!("\n[TRACE {:02}] Listener received Command", step);
                                            println!("  cmd.command: {}", cmd.command);
                                            println!("  cmd.request_id: {}", cmd.request_id);
                                            println!("  cmd.from: {}", cmd.from);
                                            println!("  cmd.to: {:?}", cmd.to);
                                            println!("  cmd.params: {}", serde_json::to_string(&cmd.params).unwrap_or_default());
                                        }
                                        if cmd.command == commands::EXECUTE_TASK {
                                            if let Ok(ai_req) = AIInferenceRequest::from_command(&cmd) {
                                                if trace {
                                                    step += 1;
                                                    println!("\n[TRACE {:02}] Listener executing AI inference", step);
                                                    println!("  ai_req.model_name: {}", ai_req.model_name);
                                                    println!("  ai_req.input_data: {}", ai_req.input_data);
                                                    println!("  ai_req.max_tokens: {:?}", ai_req.max_tokens);
                                                    println!("  ai_req.temperature: {:?}", ai_req.temperature);
                                                    println!("  ai_req.top_p: {:?}", ai_req.top_p);
                                                    println!("  ai_req.stream: {:?}", ai_req.stream);
                                                    println!("  ai_req.priority: {:?}", ai_req.priority);
                                                    println!("  ai_req.timeout_seconds: {:?}", ai_req.timeout_seconds);
                                                }
                                                let result = process_ai_inference(&ai_req).await.unwrap();
                                                if trace {
                                                    step += 1;
                                                    println!("\n[TRACE {:02}] Listener AI inference complete", step);
                                                    println!("  result: {}", serde_json::to_string(&result).unwrap_or_default());
                                                }
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
                                                if trace {
                                                    step += 1;
                                                    println!("\n[TRACE {:02}] Listener sending CommandResponse", step);
                                                    println!("  response.request_id: {}", resp.request_id);
                                                    println!("  response.to: {}", resp.to);
                                                    println!("  response.from: {}", resp.from);
                                                }
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
                        if trace {
                            step += 1;
                            println!("\n[TRACE {:02}] Listener registering to server", step);
                            println!("  namespace: {}", namespace);
                            println!("  register_msg: {}", serde_json::to_string(&reg).unwrap_or_default());
                        }
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
                                if trace {
                                    step += 1;
                                    println!("\n[TRACE {:02}] Dialer connected to server; sending lookup", step);
                                    println!("  lookup_msg: {}", serde_json::to_string(&lookup).unwrap_or_default());
                                }
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
                                if trace {
                                    step += 1;
                                    println!("\n[TRACE {:02}] Dialer connected to listener; sending AI command", step);
                                    println!("  request_id: {}", cmd.request_id);
                                    println!("  command_json: {}", cmd.to_json().unwrap_or_default());
                                }
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
                                    if trace {
                                        step += 1;
                                        println!("\n[TRACE {:02}] Dialer received response", step);
                                        println!("  response.from: {}", response.from);
                                        println!("  response.message: {}", response.message);
                                    }
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
                                                if trace {
                                                    step += 1;
                                                    println!("\n[TRACE {:02}] Dialer got listener record; dialing listener", step);
                                                    println!("  listener_quic_addr: {}", addr);
                                                }
                                                dialer.dial(addr).unwrap();
                                                dialer_dialed_listener = true;
                                            } else {
                                                // Not registered yet; retry lookup.
                                                if trace {
                                                    step += 1;
                                                    println!("\n[TRACE {:02}] Dialer lookup returned no listener yet; retrying", step);
                                                }
                                                let lookup = RegistryMsg::Lookup { namespace: namespace.clone() };
                                                let msg = JsonMessage::new(dialer_id.to_string(), serde_json::to_string(&lookup).unwrap());
                                                dialer.behaviour_mut().request_response.send_request(&server_id, msg);
                                            }
                                        }
                                    } else if let Ok(cmd_resp) = CommandResponse::from_json(&response.message) {
                                        // Validate request_id and content.
                                        if trace {
                                            step += 1;
                                            println!("\n[TRACE {:02}] Dialer parsed CommandResponse", step);
                                            println!("  cmd_resp.command: {}", cmd_resp.command);
                                            println!("  cmd_resp.request_id: {}", cmd_resp.request_id);
                                            println!("  cmd_resp.from: {}", cmd_resp.from);
                                            println!("  cmd_resp.to: {}", cmd_resp.to);
                                            println!("  cmd_resp.status: {:?}", cmd_resp.status);
                                        }
                                        assert_eq!(Some(cmd_resp.request_id.clone()), pending_request_id);
                                        let output = cmd_resp
                                            .result
                                            .and_then(|m| m.get("output").cloned())
                                            .and_then(|v| v.as_str().map(|s| s.to_string()))
                                            .unwrap_or_default();
                                        let out_l = output.to_lowercase();
                                        if trace {
                                            step += 1;
                                            println!("\n[TRACE {:02}] Final output extracted", step);
                                            println!("  output: {}", output);
                                            println!("  assertions: contains(rayleigh, scatter, wavelength)");
                                            println!("\n[TRACE {:02}] TRACE COMPLETE (E2E QUIC AI request finished)", step);
                                        }
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

