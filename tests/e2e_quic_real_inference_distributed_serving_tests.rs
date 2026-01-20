//! Real inference (distributed serving) QUIC test.
//!
//! This is intentionally `#[ignore]` because it requires local setup:
//! - `PUNCH_INFERENCE_BACKEND=llama_cpp`
//! - `LLAMA_CPP_EXE=.../llama-cli(.exe)`
//! - `LLAMA_GGUF_PATH=.../model.gguf`

use libp2p::futures::StreamExt;
use libp2p::swarm::Config as SwarmConfig;
use libp2p::{
    identity,
    request_response::{self, ProtocolSupport},
    swarm::{NetworkBehaviour, Swarm, SwarmEvent},
    Multiaddr, PeerId, StreamProtocol,
};
use std::collections::HashMap;
use std::time::Duration;
use tokio::time::timeout;

use punch_simple::{
    ai_inference_handler::{process_ai_inference, AIInferenceRequest},
    command_protocol::{commands, Command, CommandResponse},
    create_quic_transport, JsonCodec, JsonMessage,
};

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
        [(
            StreamProtocol::new("/json-message/1.0"),
            ProtocolSupport::Full,
        )],
        request_response::Config::default(),
    );

    let behaviour = RRBehaviour { request_response };
    let swarm_config =
        SwarmConfig::with_tokio_executor().with_idle_connection_timeout(Duration::from_secs(30));
    (
        Swarm::new(transport, behaviour, peer_id, swarm_config),
        peer_id,
    )
}

#[tokio::test]
#[ignore = "requires llama.cpp + GGUF: set PUNCH_INFERENCE_BACKEND=llama_cpp, LLAMA_CPP_EXE, LLAMA_GGUF_PATH"]
async fn test_quic_real_inference_distributed_serving_single_worker() {
    timeout(Duration::from_secs(120), async {
        // Worker (executor)
        let (mut worker, worker_id) = create_quic_rr_swarm().await;
        // Client (requester)
        let (mut client, client_id) = create_quic_rr_swarm().await;

        worker.listen_on("/ip4/127.0.0.1/udp/0/quic-v1".parse::<Multiaddr>().unwrap()).unwrap();
        client.listen_on("/ip4/127.0.0.1/udp/0/quic-v1".parse::<Multiaddr>().unwrap()).unwrap();

        let mut worker_addr: Option<Multiaddr> = None;
        let mut pending_request_id: Option<String> = None;

        loop {
            tokio::select! {
                ev = worker.select_next_some() => {
                    match ev {
                        SwarmEvent::NewListenAddr { address, .. } => {
                            if worker_addr.is_none() {
                                worker_addr = Some(address.clone());
                                // client dials worker
                                client.dial(address).unwrap();
                            }
                        }
                        SwarmEvent::Behaviour(RRBehaviourEvent::RequestResponse(
                            request_response::Event::Message { message, .. }
                        )) => {
                            if let request_response::Message::Request { request, channel, .. } = message {
                                let cmd = Command::from_json(&request.message).expect("command parses");
                                let ai_req = AIInferenceRequest::from_command(&cmd).expect("ai req parses");
                                let result = process_ai_inference(&ai_req).await.expect("real inference succeeds");

                                let mut response_data = HashMap::new();
                                if let Some(output) = result.get("output") {
                                    response_data.insert("output".to_string(), output.clone());
                                }

                                let resp = CommandResponse::success(
                                    &cmd.command,
                                    &cmd.request_id,
                                    &worker_id.to_string(),
                                    &cmd.from,
                                    response_data,
                                );
                                let msg = JsonMessage::new(worker_id.to_string(), resp.to_json().unwrap());
                                let _ = worker.behaviour_mut().request_response.send_response(channel, msg);
                            }
                        }
                        _ => {}
                    }
                }

                ev = client.select_next_some() => {
                    match ev {
                        SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                            if peer_id == worker_id && pending_request_id.is_none() {
                                // Send "real" request: model_name != mock
                                let mut cmd = Command::new(commands::EXECUTE_TASK, &client_id.to_string(), Some(&worker_id.to_string()));
                                cmd.params.insert("task_type".to_string(), serde_json::json!("ai_inference"));
                                cmd.params.insert("model_name".to_string(), serde_json::json!("real"));
                                cmd.params.insert("input_data".to_string(), serde_json::json!("Why is the sky blue?"));
                                cmd.params.insert("max_tokens".to_string(), serde_json::json!(128));
                                cmd.params.insert("temperature".to_string(), serde_json::json!(0.7));
                                cmd.params.insert("top_p".to_string(), serde_json::json!(0.9));

                                pending_request_id = Some(cmd.request_id.clone());
                                let msg = JsonMessage::new(client_id.to_string(), cmd.to_json().unwrap());
                                client.behaviour_mut().request_response.send_request(&worker_id, msg);
                            }
                        }
                        SwarmEvent::Behaviour(RRBehaviourEvent::RequestResponse(
                            request_response::Event::Message { message, .. }
                        )) => {
                            if let request_response::Message::Response { response, .. } = message {
                                let cmd_resp = CommandResponse::from_json(&response.message).expect("cmd response parses");
                                assert_eq!(Some(cmd_resp.request_id.clone()), pending_request_id);
                                let out = cmd_resp
                                    .result
                                    .and_then(|m| m.get("output").cloned())
                                    .and_then(|v| v.as_str().map(|s| s.to_string()))
                                    .unwrap_or_default();
                                assert!(!out.trim().is_empty(), "real inference must return non-empty output");
                                let answer = out;
                                // Don't assert specific keywords; models vary. Just sanity check it looks like text.
                                assert!(answer.len() > 20);
                                break;
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
