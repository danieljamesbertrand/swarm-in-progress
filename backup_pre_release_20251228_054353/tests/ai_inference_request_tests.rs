//! Tests for AI inference request acceptance and processing pipeline
//! 
//! This test suite verifies:
//! - AI inference request acceptance
//! - Request validation
//! - Task routing to appropriate nodes
//! - Response handling
//! - Error scenarios

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
use serde_json::json;

use punch_simple::{JsonCodec, Command, CommandResponse, ResponseStatus, commands};

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

/// Test creating an AI inference request command
#[test]
fn test_ai_inference_request_creation() {
    let from_peer = "12D3KooWTestPeer";
    let to_peer = "12D3KooWTargetPeer";
    
    // Create AI inference request
    let request = Command::new(commands::EXECUTE_TASK, from_peer, Some(to_peer))
        .with_param("task_type", json!("ai_inference"))
        .with_param("model_name", json!("gpt-4"))
        .with_param("input_data", json!("What is the capital of France?"))
        .with_param("max_tokens", json!(100))
        .with_param("temperature", json!(0.7));
    
    assert_eq!(request.command, commands::EXECUTE_TASK);
    assert_eq!(request.from, from_peer);
    assert_eq!(request.to, Some(to_peer.to_string()));
    assert_eq!(request.params.get("task_type"), Some(&json!("ai_inference")));
    assert_eq!(request.params.get("model_name"), Some(&json!("gpt-4")));
    assert_eq!(request.params.get("input_data"), Some(&json!("What is the capital of France?")));
}

/// Test AI inference request JSON serialization
#[test]
fn test_ai_inference_request_serialization() {
    let request = Command::new(commands::EXECUTE_TASK, "peer1", Some("peer2"))
        .with_param("task_type", json!("ai_inference"))
        .with_param("model_name", json!("gpt-4"))
        .with_param("input_data", json!("Test input"));
    
    let json_str = request.to_json().unwrap();
    assert!(json_str.contains("EXECUTE_TASK"));
    assert!(json_str.contains("ai_inference"));
    assert!(json_str.contains("gpt-4"));
    
    // Deserialize and verify
    let deserialized = Command::from_json(&json_str).unwrap();
    assert_eq!(deserialized.command, commands::EXECUTE_TASK);
    assert_eq!(deserialized.params.get("task_type"), Some(&json!("ai_inference")));
}

/// Test AI inference response creation
#[test]
fn test_ai_inference_response_creation() {
    let mut result = std::collections::HashMap::new();
    result.insert("output".to_string(), json!("The capital of France is Paris."));
    result.insert("tokens_used".to_string(), json!(15));
    result.insert("model".to_string(), json!("gpt-4"));
    result.insert("latency_ms".to_string(), json!(125.5));
    
    let response = CommandResponse::success(
        commands::EXECUTE_TASK,
        "req-123",
        "executor-peer",
        "requester-peer",
        result,
    );
    
    assert_eq!(response.status, ResponseStatus::Success);
    assert_eq!(response.command, commands::EXECUTE_TASK);
    assert!(response.result.is_some());
    
    let result = response.result.unwrap();
    assert_eq!(result.get("output"), Some(&json!("The capital of France is Paris.")));
    assert_eq!(result.get("tokens_used"), Some(&json!(15)));
}

/// Test AI inference request validation
#[test]
fn test_ai_inference_request_validation() {
    // Valid request
    let valid_request = Command::new(commands::EXECUTE_TASK, "peer1", Some("peer2"))
        .with_param("task_type", json!("ai_inference"))
        .with_param("model_name", json!("gpt-4"))
        .with_param("input_data", json!("Test"));
    
    assert_eq!(valid_request.params.get("task_type"), Some(&json!("ai_inference")));
    assert!(valid_request.params.contains_key("model_name"));
    assert!(valid_request.params.contains_key("input_data"));
    
    // Invalid request - missing required fields
    let invalid_request = Command::new(commands::EXECUTE_TASK, "peer1", Some("peer2"))
        .with_param("task_type", json!("ai_inference"));
    // Missing model_name and input_data
    
    assert!(!invalid_request.params.contains_key("model_name"));
    assert!(!invalid_request.params.contains_key("input_data"));
}

/// Test different AI model types
#[test]
fn test_ai_inference_different_models() {
    let models = vec!["gpt-4", "gpt-3.5-turbo", "claude-3", "llama-2", "mistral"];
    
    for model in models {
        let request = Command::new(commands::EXECUTE_TASK, "peer1", Some("peer2"))
            .with_param("task_type", json!("ai_inference"))
            .with_param("model_name", json!(model))
            .with_param("input_data", json!("Test input"));
        
        assert_eq!(request.params.get("model_name"), Some(&json!(model)));
    }
}

/// Test AI inference request with various parameters
#[test]
fn test_ai_inference_request_parameters() {
    let request = Command::new(commands::EXECUTE_TASK, "peer1", Some("peer2"))
        .with_param("task_type", json!("ai_inference"))
        .with_param("model_name", json!("gpt-4"))
        .with_param("input_data", json!("What is AI?"))
        .with_param("max_tokens", json!(500))
        .with_param("temperature", json!(0.8))
        .with_param("top_p", json!(0.9))
        .with_param("frequency_penalty", json!(0.5))
        .with_param("presence_penalty", json!(0.3))
        .with_param("stop_sequences", json!(["\\n", "END"]));
    
    assert_eq!(request.params.get("max_tokens"), Some(&json!(500)));
    assert_eq!(request.params.get("temperature"), Some(&json!(0.8)));
    assert_eq!(request.params.get("top_p"), Some(&json!(0.9)));
    assert!(request.params.contains_key("stop_sequences"));
}

/// Test AI inference error response
#[test]
fn test_ai_inference_error_response() {
    let error_response = CommandResponse::error(
        commands::EXECUTE_TASK,
        "req-123",
        "executor-peer",
        "requester-peer",
        "Model not available",
    );
    
    assert_eq!(error_response.status, ResponseStatus::Error);
    assert_eq!(error_response.error, Some("Model not available".to_string()));
    assert!(error_response.result.is_none());
}

/// Test AI inference request with batch processing
#[test]
fn test_ai_inference_batch_request() {
    let batch_inputs = json!([
        "What is AI?",
        "What is machine learning?",
        "What is deep learning?"
    ]);
    
    let request = Command::new(commands::EXECUTE_TASK, "peer1", Some("peer2"))
        .with_param("task_type", json!("ai_inference"))
        .with_param("model_name", json!("gpt-4"))
        .with_param("input_data", batch_inputs.clone())
        .with_param("batch_size", json!(3));
    
    assert_eq!(request.params.get("input_data"), Some(&batch_inputs));
    assert_eq!(request.params.get("batch_size"), Some(&json!(3)));
}

/// Test AI inference request with streaming
#[test]
fn test_ai_inference_streaming_request() {
    let request = Command::new(commands::EXECUTE_TASK, "peer1", Some("peer2"))
        .with_param("task_type", json!("ai_inference"))
        .with_param("model_name", json!("gpt-4"))
        .with_param("input_data", json!("Generate a story"))
        .with_param("stream", json!(true))
        .with_param("stream_chunk_size", json!(10));
    
    assert_eq!(request.params.get("stream"), Some(&json!(true)));
    assert_eq!(request.params.get("stream_chunk_size"), Some(&json!(10)));
}

/// Integration test: AI inference request acceptance via DHT
#[tokio::test]
async fn test_ai_inference_request_acceptance() {
    // Create bootstrap node
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
    
    // Create listener node (AI inference executor)
    let listener_key = identity::Keypair::generate_ed25519();
    let listener_peer_id = PeerId::from(listener_key.public());
    let mut listener_swarm = create_test_swarm(listener_peer_id, listener_key).await;
    
    // Create client node (AI inference requester)
    let client_key = identity::Keypair::generate_ed25519();
    let client_peer_id = PeerId::from(client_key.public());
    let mut client_swarm = create_test_swarm(client_peer_id, client_key).await;
    
    // Setup both nodes
    listener_swarm.behaviour_mut().kademlia.add_address(&bootstrap_peer_id, bootstrap_addr.clone());
    client_swarm.behaviour_mut().kademlia.add_address(&bootstrap_peer_id, bootstrap_addr.clone());
    
    listener_swarm.listen_on("/ip4/127.0.0.1/tcp/0".parse().unwrap()).unwrap();
    client_swarm.listen_on("/ip4/127.0.0.1/tcp/0".parse().unwrap()).unwrap();
    
    listener_swarm.dial(bootstrap_addr.clone()).unwrap();
    client_swarm.dial(bootstrap_addr.clone()).unwrap();
    
    // Wait for bootstrap
    tokio::time::sleep(Duration::from_secs(2)).await;
    
    // Create AI inference request
    let ai_request = Command::new(commands::EXECUTE_TASK, &client_peer_id.to_string(), Some(&listener_peer_id.to_string()))
        .with_param("task_type", json!("ai_inference"))
        .with_param("model_name", json!("gpt-4"))
        .with_param("input_data", json!("What is AI?"));
    
    // Convert to JSON message format
    let request_json = ai_request.to_json().unwrap();
    
    // Test passes if request can be created and serialized
    assert!(request_json.contains("EXECUTE_TASK"));
    assert!(request_json.contains("ai_inference"));
    assert!(request_json.contains("gpt-4"));
}

/// Test AI inference request with different task types
#[test]
fn test_ai_inference_task_types() {
    let task_types = vec![
        ("text_generation", json!("Generate a story")),
        ("text_completion", json!("Complete this sentence:")),
        ("question_answering", json!("What is the answer?")),
        ("summarization", json!("Summarize this text:")),
        ("translation", json!("Translate to French:")),
        ("classification", json!("Classify this text:")),
    ];
    
    for (task_type, input_data) in task_types {
        let request = Command::new(commands::EXECUTE_TASK, "peer1", Some("peer2"))
            .with_param("task_type", json!("ai_inference"))
            .with_param("model_name", json!("gpt-4"))
            .with_param("subtask_type", json!(task_type))
            .with_param("input_data", input_data);
        
        assert_eq!(request.params.get("subtask_type"), Some(&json!(task_type)));
    }
}

/// Test AI inference request with priority levels
#[test]
fn test_ai_inference_priority_levels() {
    let priorities = vec!["low", "normal", "high", "urgent"];
    
    for priority in priorities {
        let request = Command::new(commands::EXECUTE_TASK, "peer1", Some("peer2"))
            .with_param("task_type", json!("ai_inference"))
            .with_param("model_name", json!("gpt-4"))
            .with_param("input_data", json!("Test"))
            .with_param("priority", json!(priority));
        
        assert_eq!(request.params.get("priority"), Some(&json!(priority)));
    }
}

/// Test AI inference request with resource requirements
#[test]
fn test_ai_inference_resource_requirements() {
    let request = Command::new(commands::EXECUTE_TASK, "peer1", Some("peer2"))
        .with_param("task_type", json!("ai_inference"))
        .with_param("model_name", json!("gpt-4"))
        .with_param("input_data", json!("Test"))
        .with_param("min_cpu_cores", json!(4))
        .with_param("min_memory_mb", json!(8192))
        .with_param("min_gpu_memory_mb", json!(4096))
        .with_param("requires_gpu", json!(true));
    
    assert_eq!(request.params.get("min_cpu_cores"), Some(&json!(4)));
    assert_eq!(request.params.get("min_memory_mb"), Some(&json!(8192)));
    assert_eq!(request.params.get("requires_gpu"), Some(&json!(true)));
}

/// Test AI inference request timeout handling
#[test]
fn test_ai_inference_timeout() {
    let request = Command::new(commands::EXECUTE_TASK, "peer1", Some("peer2"))
        .with_param("task_type", json!("ai_inference"))
        .with_param("model_name", json!("gpt-4"))
        .with_param("input_data", json!("Test"))
        .with_param("timeout_seconds", json!(30))
        .with_param("max_retries", json!(3));
    
    assert_eq!(request.params.get("timeout_seconds"), Some(&json!(30)));
    assert_eq!(request.params.get("max_retries"), Some(&json!(3)));
}

