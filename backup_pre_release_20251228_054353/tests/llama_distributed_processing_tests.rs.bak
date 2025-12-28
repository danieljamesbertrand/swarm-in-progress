//! Tests for Llama Distributed Fragment-Based Processing
//! 
//! This test suite verifies the distributed Llama processing system that
//! splits work into fragments and distributes them across nodes in the swarm.

use libp2p::{
    identity,
    tcp,
    noise,
    yamux,
    kad,
    relay,
    request_response::{self, ProtocolSupport},
    swarm::{NetworkBehaviour, Swarm},
    core::transport::Transport,
    PeerId, StreamProtocol,
};
use libp2p::swarm::Config as SwarmConfig;
use std::time::Duration;
use serde_json::json;

use punch_simple::{JsonCodec, Command, ResponseStatus, commands, LlamaJob, JobResult, FragmentResult, process_fragment};

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

/// Test creating a Llama job from a request
#[test]
fn test_llama_job_creation() {
    let request = Command::new(commands::EXECUTE_TASK, "peer1", Some("peer2"))
        .with_param("task_type", json!("llama_inference"))
        .with_param("model_name", json!("llama-2-7b"))
        .with_param("input_data", json!("This is a long text that will be split into fragments for distributed processing"));

    let job = LlamaJob::from_request(&request, 4).unwrap();
    assert_eq!(job.model_name, "llama-2-7b");
    assert_eq!(job.total_fragments, 4);
    assert_eq!(job.fragments.len(), 4);
    assert!(!job.job_id.is_empty());
}

/// Test fragment splitting for text input
#[test]
fn test_text_fragment_splitting() {
    let long_text = "This is a very long text that needs to be split into multiple fragments. ".repeat(10);
    let request = Command::new(commands::EXECUTE_TASK, "peer1", Some("peer2"))
        .with_param("task_type", json!("llama_inference"))
        .with_param("model_name", json!("llama-2-7b"))
        .with_param("input_data", json!(long_text));

    let job = LlamaJob::from_request(&request, 5).unwrap();
    
    // Verify all fragments have content
    for fragment in &job.fragments {
        assert!(!fragment.fragment_id.is_empty());
        assert_eq!(fragment.job_id, job.job_id);
        assert!(fragment.fragment_index < job.total_fragments);
        assert!(fragment.input_data.as_str().is_some());
    }
}

/// Test fragment splitting for array input
#[test]
fn test_array_fragment_splitting() {
    let items: Vec<serde_json::Value> = (0..20)
        .map(|i| json!(format!("Item {}", i)))
        .collect();
    
    let request = Command::new(commands::EXECUTE_TASK, "peer1", Some("peer2"))
        .with_param("task_type", json!("llama_inference"))
        .with_param("model_name", json!("llama-2-7b"))
        .with_param("input_data", json!(items));

    let job = LlamaJob::from_request(&request, 4).unwrap();
    assert_eq!(job.fragments.len(), 4);
    
    // Verify fragments contain array items
    for fragment in &job.fragments {
        assert!(fragment.input_data.is_array());
    }
}

/// Test fragment to command conversion
#[test]
fn test_fragment_to_command() {
    let request = Command::new(commands::EXECUTE_TASK, "peer1", Some("peer2"))
        .with_param("task_type", json!("llama_inference"))
        .with_param("model_name", json!("llama-2-7b"))
        .with_param("input_data", json!("Test input"))
        .with_param("max_tokens", json!(100))
        .with_param("temperature", json!(0.7));

    let job = LlamaJob::from_request(&request, 2).unwrap();
    let fragment = &job.fragments[0];
    let command = job.fragment_to_command(fragment, "target-node-id");

    assert_eq!(command.command, commands::EXECUTE_TASK);
    assert_eq!(command.to, Some("target-node-id".to_string()));
    assert_eq!(command.params.get("task_type"), Some(&json!("llama_fragment")));
    assert_eq!(command.params.get("job_id"), Some(&json!(job.job_id)));
    assert_eq!(command.params.get("fragment_id"), Some(&json!(fragment.fragment_id)));
    assert_eq!(command.params.get("fragment_index"), Some(&json!(fragment.fragment_index)));
    assert_eq!(command.params.get("total_fragments"), Some(&json!(job.total_fragments)));
    assert_eq!(command.params.get("max_tokens"), Some(&json!(100)));
    assert_eq!(command.params.get("temperature"), Some(&json!(0.7)));
}

/// Test fragment result aggregation
#[test]
fn test_fragment_result_aggregation() {
    let mut fragment_results = Vec::new();
    
    // Create results from 4 different nodes
    for i in 0..4 {
        fragment_results.push(FragmentResult {
            fragment_id: format!("frag-{}", i),
            job_id: "job-123".to_string(),
            fragment_index: i,
            output: json!(format!("Fragment {} output", i)),
            tokens_generated: 50 + (i as u32 * 10),
            processing_time_ms: 100.0 + (i as f64 * 10.0),
            node_id: format!("node-{}", i),
        });
    }

    let job_result = JobResult::from_fragments("job-123", fragment_results);
    
    assert_eq!(job_result.job_id, "job-123");
    assert_eq!(job_result.fragment_results.len(), 4);
    assert_eq!(job_result.total_tokens, 50 + 60 + 70 + 80);
    assert!(job_result.combined_output.contains("Fragment 0 output"));
    assert!(job_result.combined_output.contains("Fragment 3 output"));
    assert!(job_result.total_processing_time_ms > 0.0);
}

/// Test fragment result ordering
#[test]
fn test_fragment_result_ordering() {
    let mut fragment_results = Vec::new();
    
    // Create results out of order
    fragment_results.push(FragmentResult {
        fragment_id: "frag-3".to_string(),
        job_id: "job-1".to_string(),
        fragment_index: 3,
        output: json!("Third"),
        tokens_generated: 30,
        processing_time_ms: 30.0,
        node_id: "node-3".to_string(),
    });
    
    fragment_results.push(FragmentResult {
        fragment_id: "frag-1".to_string(),
        job_id: "job-1".to_string(),
        fragment_index: 1,
        output: json!("First"),
        tokens_generated: 10,
        processing_time_ms: 10.0,
        node_id: "node-1".to_string(),
    });
    
    fragment_results.push(FragmentResult {
        fragment_id: "frag-0".to_string(),
        job_id: "job-1".to_string(),
        fragment_index: 0,
        output: json!("Zero"),
        tokens_generated: 0,
        processing_time_ms: 0.0,
        node_id: "node-0".to_string(),
    });

    let job_result = JobResult::from_fragments("job-1", fragment_results);
    
    // Verify results are sorted by index
    assert_eq!(job_result.fragment_results[0].fragment_index, 0);
    assert_eq!(job_result.fragment_results[1].fragment_index, 1);
    assert_eq!(job_result.fragment_results[2].fragment_index, 3);
    
    // Combined output should have correct order
    assert!(job_result.combined_output.contains("Zero"));
    assert!(job_result.combined_output.contains("First"));
    assert!(job_result.combined_output.contains("Third"));
}

/// Test job result to response conversion
#[test]
fn test_job_result_to_response() {
    let request = Command::new(commands::EXECUTE_TASK, "requester", Some("coordinator"))
        .with_param("task_type", json!("llama_inference"))
        .with_param("model_name", json!("llama-2-7b"))
        .with_param("input_data", json!("Test"));

    let fragment_results = vec![
        FragmentResult {
            fragment_id: "frag-0".to_string(),
            job_id: "job-1".to_string(),
            fragment_index: 0,
            output: json!("Output"),
            tokens_generated: 100,
            processing_time_ms: 150.0,
            node_id: "node-1".to_string(),
        }
    ];

    let job_result = JobResult::from_fragments("job-1", fragment_results);
    let response = job_result.to_response(&request);

    assert_eq!(response.status, ResponseStatus::Success);
    assert_eq!(response.command, commands::EXECUTE_TASK);
    assert!(response.result.is_some());
    
    let result = response.result.unwrap();
    assert!(result.contains_key("output"));
    assert!(result.contains_key("total_tokens"));
    assert!(result.contains_key("fragments_processed"));
    assert_eq!(result.get("total_tokens"), Some(&json!(100)));
}

/// Test fragment processing with different input types
#[tokio::test]
#[ignore = "Requires rsync to be installed"]
async fn test_fragment_processing() {
    let request = Command::new(commands::EXECUTE_TASK, "peer1", Some("peer2"))
        .with_param("task_type", json!("llama_inference"))
        .with_param("model_name", json!("llama-2-7b"))
        .with_param("input_data", json!("Test fragment input"));

    let job = LlamaJob::from_request(&request, 1).unwrap();
    let fragment = &job.fragments[0];
    
    let result = process_fragment(fragment).await.unwrap();
    
    assert_eq!(result.fragment_id, fragment.fragment_id);
    assert_eq!(result.job_id, fragment.job_id);
    assert_eq!(result.fragment_index, fragment.fragment_index);
    assert!(result.tokens_generated > 0);
    assert!(result.processing_time_ms >= 0.0);
    assert!(!result.node_id.is_empty());
}

/// Test distributed processing workflow
#[tokio::test]
#[ignore = "Requires rsync to be installed"]
async fn test_distributed_processing_workflow() {
    // Step 1: Create a Llama job
    let request = Command::new(commands::EXECUTE_TASK, "client", Some("coordinator"))
        .with_param("task_type", json!("llama_inference"))
        .with_param("model_name", json!("llama-2-7b"))
        .with_param("input_data", json!("This is a test input for distributed Llama processing across multiple nodes"))
        .with_param("max_tokens", json!(200))
        .with_param("temperature", json!(0.8));

    let job = LlamaJob::from_request(&request, 3).unwrap();
    assert_eq!(job.fragments.len(), 3);

    // Step 2: Simulate processing fragments on different nodes
    let mut fragment_results = Vec::new();
    for fragment in &job.fragments {
        let result = process_fragment(fragment).await.unwrap();
        fragment_results.push(result);
    }

    // Step 3: Aggregate results
    let job_result = JobResult::from_fragments(&job.job_id, fragment_results);
    
    // Step 4: Verify complete result
    assert_eq!(job_result.fragment_results.len(), 3);
    assert!(job_result.total_tokens > 0);
    assert!(!job_result.combined_output.is_empty());
    
    // Step 5: Convert to response
    let response = job_result.to_response(&request);
    assert_eq!(response.status, ResponseStatus::Success);
}

/// Test fragment splitting edge cases
#[test]
fn test_fragment_splitting_edge_cases() {
    // Test with single fragment
    let request = Command::new(commands::EXECUTE_TASK, "peer1", Some("peer2"))
        .with_param("task_type", json!("llama_inference"))
        .with_param("model_name", json!("llama-2-7b"))
        .with_param("input_data", json!("Short"));

    let job = LlamaJob::from_request(&request, 1).unwrap();
    assert_eq!(job.fragments.len(), 1);

    // Test with more fragments than input length
    let request2 = Command::new(commands::EXECUTE_TASK, "peer1", Some("peer2"))
        .with_param("task_type", json!("llama_inference"))
        .with_param("model_name", json!("llama-2-7b"))
        .with_param("input_data", json!("ABC"));

    let job2 = LlamaJob::from_request(&request2, 10).unwrap();
    // Should only create fragments for available data
    assert!(job2.fragments.len() <= 3);
}

/// Test fragment context windows
#[test]
fn test_fragment_context_windows() {
    let text = "ABCDEFGHIJKLMNOPQRSTUVWXYZ";
    let request = Command::new(commands::EXECUTE_TASK, "peer1", Some("peer2"))
        .with_param("task_type", json!("llama_inference"))
        .with_param("model_name", json!("llama-2-7b"))
        .with_param("input_data", json!(text));

    let job = LlamaJob::from_request(&request, 3).unwrap();
    
    // Verify context windows are set
    for fragment in &job.fragments {
        assert!(fragment.context_window_start <= fragment.context_window_end);
    }
}

/// Test job with parameters preservation
#[test]
fn test_parameter_preservation() {
    let request = Command::new(commands::EXECUTE_TASK, "peer1", Some("peer2"))
        .with_param("task_type", json!("llama_inference"))
        .with_param("model_name", json!("llama-2-7b"))
        .with_param("input_data", json!("Test"))
        .with_param("max_tokens", json!(500))
        .with_param("temperature", json!(0.9))
        .with_param("top_p", json!(0.95));

    let job = LlamaJob::from_request(&request, 2).unwrap();
    
    // Verify parameters are preserved in fragments
    for fragment in &job.fragments {
        assert_eq!(fragment.parameters.get("max_tokens"), Some(&json!(500)));
        assert_eq!(fragment.parameters.get("temperature"), Some(&json!(0.9)));
        assert_eq!(fragment.parameters.get("top_p"), Some(&json!(0.95)));
    }
}

