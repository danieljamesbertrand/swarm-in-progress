//! End-to-end QUIC-only multi-node integration test:
//! discovery (registration) → weighted routing → distributed execution → response to requester.

use libp2p::{
    identity,
    kad,
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
    JsonCodec, JsonMessage,
    create_quic_transport,
    command_protocol::{Command, CommandResponse, NodeCapabilities, NodeWeights, commands},
    ai_inference_handler::{AIInferenceRequest, process_ai_inference},
};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
enum DiscoveryMessage {
    #[serde(rename = "register")]
    Register { peer_id: String, capabilities: NodeCapabilities },
    #[serde(rename = "ack")]
    Ack { ok: bool },
}

#[derive(NetworkBehaviour)]
struct NodeBehaviour {
    kademlia: kad::Behaviour<kad::store::MemoryStore>,
    request_response: request_response::Behaviour<JsonCodec>,
}

async fn create_quic_node() -> (Swarm<NodeBehaviour>, PeerId) {
    let key = identity::Keypair::generate_ed25519();
    let peer_id = PeerId::from(key.public());

    let transport = create_quic_transport(&key).expect("quic transport");

    let store = kad::store::MemoryStore::new(peer_id);
    let mut kad_config = kad::Config::default();
    kad_config.set_query_timeout(Duration::from_secs(5));
    let kademlia = kad::Behaviour::with_config(peer_id, store, kad_config);

    let request_response = request_response::Behaviour::with_codec(
        JsonCodec,
        [(StreamProtocol::new("/json-message/1.0"), ProtocolSupport::Full)],
        request_response::Config::default(),
    );

    let behaviour = NodeBehaviour { kademlia, request_response };
    let swarm_config = SwarmConfig::with_tokio_executor()
        .with_idle_connection_timeout(Duration::from_secs(30));

    (Swarm::new(transport, behaviour, peer_id, swarm_config), peer_id)
}

fn make_capabilities_fast() -> NodeCapabilities {
    NodeCapabilities {
        cpu_cores: 16,
        cpu_usage: 5.0,
        cpu_speed_ghz: 3.5,
        memory_total_mb: 32768,
        memory_available_mb: 24576,
        disk_total_mb: 1000000,
        disk_available_mb: 900000,
        latency_ms: 10.0,
        reputation: 1.0,
        gpu_memory_mb: 24576,
        gpu_compute_units: 10000,
        gpu_usage: 0.0,
        gpu_available: true,
    }
}

fn make_capabilities_slow() -> NodeCapabilities {
    NodeCapabilities {
        cpu_cores: 2,
        cpu_usage: 50.0,
        cpu_speed_ghz: 2.0,
        memory_total_mb: 4096,
        memory_available_mb: 1024,
        disk_total_mb: 100000,
        disk_available_mb: 50000,
        latency_ms: 80.0,
        reputation: 0.7,
        gpu_memory_mb: 0,
        gpu_compute_units: 0,
        gpu_usage: 0.0,
        gpu_available: false,
    }
}

#[tokio::test]
async fn test_e2e_quic_discovery_weighted_routing_distributed_ai() {
    let test_timeout = Duration::from_secs(30);
    timeout(test_timeout, async {
        let (mut coordinator, coordinator_id) = create_quic_node().await;
        let (mut worker_fast, worker_fast_id) = create_quic_node().await;
        let (mut worker_slow, worker_slow_id) = create_quic_node().await;

        // Listen (QUIC-only)
        coordinator.listen_on("/ip4/127.0.0.1/udp/0/quic-v1".parse::<Multiaddr>().unwrap()).unwrap();
        worker_fast.listen_on("/ip4/127.0.0.1/udp/0/quic-v1".parse::<Multiaddr>().unwrap()).unwrap();
        worker_slow.listen_on("/ip4/127.0.0.1/udp/0/quic-v1".parse::<Multiaddr>().unwrap()).unwrap();

        let mut coordinator_addr: Option<Multiaddr> = None;
        let mut worker_fast_addr: Option<Multiaddr> = None;
        let mut worker_slow_addr: Option<Multiaddr> = None;

        // Capabilities (what weighted routing uses)
        let caps_fast = make_capabilities_fast();
        let caps_slow = make_capabilities_slow();

        // Discovery results (workers register their capabilities over QUIC).
        let mut discovered: HashMap<PeerId, NodeCapabilities> = HashMap::new();

        // Responses collected (distributed execution)
        let prompt = "Why is the sky blue?";
        let request_id = uuid::Uuid::new_v4().to_string();
        let mut sent_requests = false;
        let mut received_outputs: HashMap<PeerId, String> = HashMap::new();

        // Worker bookkeeping: register once connected.
        let mut worker_fast_registered = false;
        let mut worker_slow_registered = false;
        let mut worker_fast_dialed = false;
        let mut worker_slow_dialed = false;

        loop {
            tokio::select! {
                event = coordinator.select_next_some() => {
                    match event {
                        SwarmEvent::NewListenAddr { address, .. } => {
                            if coordinator_addr.is_none() {
                                assert!(address.to_string().contains("quic-v1"));
                                coordinator_addr = Some(address);
                            }
                        }
                        SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                            let _ = peer_id; // discovery handled via registration messages
                        }
                        SwarmEvent::Behaviour(NodeBehaviourEvent::RequestResponse(
                            request_response::Event::Message { message, .. }
                        )) => {
                            match message {
                                request_response::Message::Request { request, channel, .. } => {
                                    if let Ok(msg) = serde_json::from_str::<DiscoveryMessage>(&request.message) {
                                        if let DiscoveryMessage::Register { peer_id, capabilities } = msg {
                                            let peer: PeerId = peer_id.parse().expect("peer id parses");
                                            discovered.insert(peer, capabilities);

                                            // Ack registration
                                            let ack = DiscoveryMessage::Ack { ok: true };
                                            let resp = JsonMessage::new(coordinator_id.to_string(), serde_json::to_string(&ack).unwrap());
                                            let _ = coordinator.behaviour_mut().request_response.send_response(channel, resp);

                                            // When both workers are discovered, select by weight and dispatch tasks.
                                            if discovered.len() == 2 && !sent_requests {
                                                let weights = NodeWeights::default();
                                                let fast_score = discovered.get(&worker_fast_id).unwrap().calculate_score(&weights);
                                                let slow_score = discovered.get(&worker_slow_id).unwrap().calculate_score(&weights);
                                                assert!(fast_score > slow_score, "weighted routing should prefer the higher-capability node");

                                                // Distributed execution: send same request_id to both workers, collect + aggregate.
                                                for (peer, part) in [(worker_fast_id, "explanation"), (worker_slow_id, "details")] {
                                                    let mut cmd = Command::new(commands::EXECUTE_TASK, &coordinator_id.to_string(), Some(&peer.to_string()));
                                                    cmd.request_id = request_id.clone();
                                                    cmd.params.insert("task_type".to_string(), serde_json::json!("ai_inference"));
                                                    cmd.params.insert("model_name".to_string(), serde_json::json!("mock"));
                                                    cmd.params.insert("input_data".to_string(), serde_json::json!(prompt));
                                                    cmd.params.insert("part".to_string(), serde_json::json!(part));

                                                    let req_msg = JsonMessage::new(coordinator_id.to_string(), cmd.to_json().unwrap());
                                                    coordinator.behaviour_mut().request_response.send_request(&peer, req_msg);
                                                }
                                                sent_requests = true;
                                            }
                                        }
                                    }
                                }
                                request_response::Message::Response { response, .. } => {
                                    // AI responses
                                    let cmd_resp = CommandResponse::from_json(&response.message).expect("response json parses");
                                    assert_eq!(cmd_resp.request_id, request_id, "response must match originating request_id");
                                    if let Some(result) = cmd_resp.result {
                                        if let Some(output) = result.get("output").and_then(|v| v.as_str()) {
                                            let from_peer: PeerId = cmd_resp.from.parse().expect("executor peer id parses");
                                            received_outputs.insert(from_peer, output.to_string());
                                        }
                                    }
                                }
                            }

                            if received_outputs.len() == 2 {
                                let combined = format!(
                                    "{}\n{}",
                                    received_outputs.get(&worker_fast_id).unwrap(),
                                    received_outputs.get(&worker_slow_id).unwrap()
                                );

                                // “Real answer” guardrails: key scientific points must be present.
                                let combined_lower = combined.to_lowercase();
                                assert!(combined_lower.contains("rayleigh"), "answer should mention Rayleigh scattering");
                                assert!(combined_lower.contains("shorter"), "answer should mention shorter wavelengths");
                                assert!(combined_lower.contains("scatter"), "answer should explain scattering");
                                assert!(combined_lower.contains("sunset") || combined_lower.contains("sunrise") || combined_lower.contains("reds") || combined_lower.contains("oranges"),
                                    "answer should connect to sunrise/sunset color shift");

                                return; // success
                            }
                        }
                        _ => {}
                    }
                }

                event = worker_fast.select_next_some() => {
                    match event {
                        SwarmEvent::NewListenAddr { address, .. } => {
                            if worker_fast_addr.is_none() {
                                assert!(address.to_string().contains("quic-v1"));
                                worker_fast_addr = Some(address);
                            }
                        }
                        SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                            if peer_id == coordinator_id {
                                worker_fast.behaviour_mut().kademlia.add_address(&coordinator_id, coordinator_addr.clone().unwrap());
                                let _ = worker_fast.behaviour_mut().kademlia.bootstrap();
                                // Register capabilities with coordinator over QUIC.
                                if !worker_fast_registered {
                                    let reg = DiscoveryMessage::Register { peer_id: worker_fast_id.to_string(), capabilities: caps_fast.clone() };
                                    let msg = JsonMessage::new(worker_fast_id.to_string(), serde_json::to_string(&reg).unwrap());
                                    worker_fast.behaviour_mut().request_response.send_request(&coordinator_id, msg);
                                    worker_fast_registered = true;
                                }
                            }
                        }
                        SwarmEvent::Behaviour(NodeBehaviourEvent::RequestResponse(
                            request_response::Event::Message { message, .. }
                        )) => {
                            if let request_response::Message::Request { request, channel, .. } = message {
                                if let Ok(cmd) = Command::from_json(&request.message) {
                                    if let Ok(ai_req) = AIInferenceRequest::from_command(&cmd) {
                                        let result = process_ai_inference(&ai_req).await.unwrap();
                                        let mut response_data = HashMap::new();
                                        response_data.insert("output".to_string(), result.get("output").cloned().unwrap_or_else(|| serde_json::json!("")));
                                        response_data.insert("model".to_string(), result.get("model").cloned().unwrap_or_else(|| serde_json::json!("mock")));
                                        response_data.insert("part".to_string(), cmd.params.get("part").cloned().unwrap_or_else(|| serde_json::json!("explanation")));
                                        let resp = CommandResponse::success(
                                            &cmd.command,
                                            &cmd.request_id,
                                            &worker_fast_id.to_string(),
                                            &cmd.from,
                                            response_data,
                                        );
                                        let msg = JsonMessage::new(worker_fast_id.to_string(), resp.to_json().unwrap());
                                        let _ = worker_fast.behaviour_mut().request_response.send_response(channel, msg);
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }

                event = worker_slow.select_next_some() => {
                    match event {
                        SwarmEvent::NewListenAddr { address, .. } => {
                            if worker_slow_addr.is_none() {
                                assert!(address.to_string().contains("quic-v1"));
                                worker_slow_addr = Some(address);
                            }
                        }
                        SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                            if peer_id == coordinator_id {
                                worker_slow.behaviour_mut().kademlia.add_address(&coordinator_id, coordinator_addr.clone().unwrap());
                                let _ = worker_slow.behaviour_mut().kademlia.bootstrap();
                                if !worker_slow_registered {
                                    let reg = DiscoveryMessage::Register { peer_id: worker_slow_id.to_string(), capabilities: caps_slow.clone() };
                                    let msg = JsonMessage::new(worker_slow_id.to_string(), serde_json::to_string(&reg).unwrap());
                                    worker_slow.behaviour_mut().request_response.send_request(&coordinator_id, msg);
                                    worker_slow_registered = true;
                                }
                            }
                        }
                        SwarmEvent::Behaviour(NodeBehaviourEvent::RequestResponse(
                            request_response::Event::Message { message, .. }
                        )) => {
                            if let request_response::Message::Request { request, channel, .. } = message {
                                if let Ok(cmd) = Command::from_json(&request.message) {
                                    if let Ok(ai_req) = AIInferenceRequest::from_command(&cmd) {
                                        // Simulate “distributed pipeline” by adding a tiny delay on the weaker node.
                                        tokio::time::sleep(Duration::from_millis(50)).await;
                                        let result = process_ai_inference(&ai_req).await.unwrap();
                                        let mut response_data = HashMap::new();
                                        response_data.insert("output".to_string(), result.get("output").cloned().unwrap_or_else(|| serde_json::json!("")));
                                        response_data.insert("model".to_string(), result.get("model").cloned().unwrap_or_else(|| serde_json::json!("mock")));
                                        response_data.insert("part".to_string(), cmd.params.get("part").cloned().unwrap_or_else(|| serde_json::json!("details")));
                                        let resp = CommandResponse::success(
                                            &cmd.command,
                                            &cmd.request_id,
                                            &worker_slow_id.to_string(),
                                            &cmd.from,
                                            response_data,
                                        );
                                        let msg = JsonMessage::new(worker_slow_id.to_string(), resp.to_json().unwrap());
                                        let _ = worker_slow.behaviour_mut().request_response.send_response(channel, msg);
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }

            // Kick off worker dials once coordinator is listening.
            if let Some(ref addr) = coordinator_addr {
                // Ensure workers connect to the coordinator (discovery begins from connection + DHT).
                if !worker_fast_dialed && worker_fast_addr.is_some() {
                    let _ = worker_fast.dial(addr.clone());
                    worker_fast_dialed = true;
                }
                if !worker_slow_dialed && worker_slow_addr.is_some() {
                    let _ = worker_slow.dial(addr.clone());
                    worker_slow_dialed = true;
                }
            }
        }
    }).await.expect("test should complete within timeout");
}

