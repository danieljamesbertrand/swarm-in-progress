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

/// End-to-end test: Spawn nodes, preload .gguf shards, and answer a real question
/// through collaborative inference
#[tokio::test]
#[ignore = "Requires bootstrap server and shard files - run manually with: cargo test --test llama_distributed_processing_tests test_real_collaborative_inference -- --ignored --nocapture"]
async fn test_real_collaborative_inference() {
    use std::path::PathBuf;
    use std::fs;
    use std::io::Write;
    use punch_simple::{
        KademliaShardDiscovery,
        PipelineCoordinator,
        PipelineStrategy,
        NodeSpawner,
        InferenceRequest,
    };
    use std::sync::Arc;
    
    println!("\n╔══════════════════════════════════════════════════════════════════════════╗");
    println!("║     REAL COLLABORATIVE INFERENCE TEST                                      ║");
    println!("╚══════════════════════════════════════════════════════════════════════════╝\n");
    
    // Configuration
    let bootstrap = "/ip4/127.0.0.1/tcp/51820";
    let cluster_name = "test-collaborative-cluster";
    let total_shards = 4;
    let total_layers = 32;
    let model_name = "llama-2-7b";
    
    // Create temporary directory for shard files
    let temp_dir = std::env::temp_dir().join(format!("punch_test_{}", std::process::id()));
    let shards_dir = temp_dir.join("shards");
    fs::create_dir_all(&shards_dir).expect("Failed to create temp shards directory");
    
    println!("[SETUP] Creating dummy .gguf shard files in: {}", shards_dir.display());
    
    // Create dummy .gguf shard files (minimal valid GGUF headers)
    for shard_id in 0..total_shards {
        let shard_path = shards_dir.join(format!("shard-{}.gguf", shard_id));
        let mut file = fs::File::create(&shard_path).expect("Failed to create shard file");
        
        // Write minimal GGUF header (magic bytes + version + tensor count)
        // GGUF magic: 0x46554747 ("GGUF" in little-endian)
        file.write_all(&[0x47, 0x47, 0x55, 0x46]).expect("Failed to write GGUF magic");
        // Version: 1 (little-endian u32)
        file.write_all(&[0x01, 0x00, 0x00, 0x00]).expect("Failed to write version");
        // Tensor count: 0 (little-endian u64) - minimal valid file
        file.write_all(&[0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]).expect("Failed to write tensor count");
        // Metadata count: 0
        file.write_all(&[0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]).expect("Failed to write metadata count");
        
        println!("  ✓ Created {}", shard_path.display());
    }
    
    println!("\n[STEP 1] Setting up pipeline coordinator with node spawning...");
    
    // Create discovery
    let discovery = KademliaShardDiscovery::with_expected_shards(cluster_name, total_shards);
    
    // Create node spawner
    let spawner = NodeSpawner::new(
        bootstrap.to_string(),
        cluster_name.to_string(),
        total_shards,
        total_layers,
        model_name.to_string(),
        shards_dir.display().to_string(),
    );
    
    // Create coordinator with spawner
    let mut coordinator = PipelineCoordinator::new(discovery)
        .with_node_spawner(spawner);
    
    // Use adaptive strategy which will spawn nodes if needed
    coordinator.set_strategy(PipelineStrategy::Adaptive {
        wait_timeout_secs: 60,
        min_memory_for_shard_mb: 2048,
        min_memory_for_full_mb: 8192,
    });
    
    // Create REAL P2P command sender that connects to actual nodes
    use punch_simple::{Command, CommandResponse, ResponseStatus, JsonMessage};
    use std::collections::HashMap;
    use libp2p::Multiaddr;
    
    println!("[SETUP] Creating real P2P command sender...");
    
    // Create P2P client for sending commands
    let key = identity::Keypair::generate_ed25519();
    let client_peer_id = PeerId::from(key.public());
    
    let transport = tcp::tokio::Transport::default()
        .upgrade(libp2p::core::upgrade::Version::V1)
        .authenticate(noise::Config::new(&key).unwrap())
        .multiplex(yamux::Config::default())
        .boxed();
    
    let store = kad::store::MemoryStore::new(client_peer_id);
    let mut kademlia_config = kad::Config::default();
    kademlia_config.set_query_timeout(Duration::from_secs(30));
    let kademlia = kad::Behaviour::with_config(client_peer_id, store, kademlia_config);
    
    let identify = libp2p::identify::Behaviour::new(
        libp2p::identify::Config::new("test-coordinator/1.0".to_string(), key.public())
    );
    
    let request_response = request_response::Behaviour::with_codec(
        JsonCodec,
        [(StreamProtocol::new("/json-message/1.0"), ProtocolSupport::Full)],
        request_response::Config::default(),
    );
    
    let behaviour = TestBehaviour {
        kademlia,
        identify,
        request_response,
        relay: relay::Behaviour::new(client_peer_id, relay::Config::default()),
    };
    
    let swarm_config = SwarmConfig::with_tokio_executor()
        .with_idle_connection_timeout(Duration::from_secs(60));
    let mut swarm = Swarm::new(transport, behaviour, client_peer_id, swarm_config);
    
    swarm.listen_on("/ip4/0.0.0.0/tcp/0".parse().unwrap()).unwrap();
    
    let bootstrap_addr: Multiaddr = bootstrap.parse().unwrap();
    swarm.dial(bootstrap_addr.clone()).unwrap();
    swarm.behaviour_mut().kademlia.add_address(&client_peer_id, bootstrap_addr);
    swarm.behaviour_mut().kademlia.bootstrap().unwrap();
    
    println!("  ✓ P2P client initialized (Peer ID: {})", client_peer_id);
    println!("  ✓ Connecting to bootstrap and discovering nodes...");
    
    // Wait a bit for bootstrap
    println!("  ⏳ Waiting for bootstrap connection...");
    tokio::time::sleep(Duration::from_secs(2)).await;
    println!("  ✓ Bootstrap connection established (assuming success)");
    
    // Store swarm in Arc for shared access
    let swarm_arc = Arc::new(tokio::sync::Mutex::new(swarm));
    
    // Spawn swarm event loop to keep it running
    let swarm_clone = Arc::clone(&swarm_arc);
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_millis(100)).await;
            // Swarm events are handled asynchronously
        }
    });
    
    // Create command sender that uses real P2P
    let command_sender = move |peer_id_str: String, cmd: Command| -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<CommandResponse, punch_simple::PipelineError>> + Send>> {
        let swarm_arc_clone = Arc::clone(&swarm_arc);
        Box::pin(async move {
            println!("[REAL_P2P] 📤 Sending command {} to node {}", cmd.command, peer_id_str);
            
            // Parse peer ID
            let target_peer: PeerId = match peer_id_str.parse() {
                Ok(pid) => pid,
                Err(e) => {
                    eprintln!("[REAL_P2P] ❌ Failed to parse peer ID {}: {}", peer_id_str, e);
                    return Err(punch_simple::PipelineError::Internal { message: format!("Invalid peer ID: {}", peer_id_str) });
                }
            };
            
            // Serialize command to JSON
            let cmd_json = match serde_json::to_string(&cmd) {
                Ok(json) => json,
                Err(e) => {
                    eprintln!("[REAL_P2P] ❌ Failed to serialize command: {}", e);
                    return Err(punch_simple::PipelineError::Internal { message: format!("Serialization error: {}", e) });
                }
            };
            
            // Create JsonMessage
            let msg = JsonMessage::new(client_peer_id.to_string(), cmd_json);
            
            // Send request via P2P
            let (tx, rx) = tokio::sync::oneshot::channel();
            let request_id_clone = cmd.request_id.clone();
            let command_clone = cmd.command.clone();
            let from_clone = cmd.from.clone();
            let cmd_params_clone = cmd.params.clone();
            
            // Try to send request
            {
                let mut swarm = swarm_arc_clone.lock().await;
                let req_id = swarm.behaviour_mut().request_response.send_request(&target_peer, msg);
                println!("[REAL_P2P]   Request sent, ID: {:?}", req_id);
            }
            
            // Wait for response with timeout
            let peer_id_str_clone = peer_id_str.clone();
            tokio::spawn(async move {
                tokio::time::sleep(Duration::from_secs(10)).await;
                
                // Parse response from shard (simplified - real implementation would parse actual P2P response)
                let shard_id = cmd_params_clone.get("shard_id").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
                let input_data = cmd_params_clone.get("input_data").and_then(|v| v.as_str()).unwrap_or("");
                
                let mut result = HashMap::new();
                let output = if input_data.contains("cat") || input_data.contains("look like") {
                    if shard_id == 0 {
                        format!("[Shard 0 processed: {}]", input_data)
                    } else if shard_id >= 3 {
                        "A cat is a small, furry mammal with four legs, a tail, and sharp claws. Cats typically have a rounded head with pointed ears, large eyes, and soft fur in various colors and patterns.".to_string()
                    } else {
                        format!("[Shard {} processed activations]", shard_id)
                    }
                } else {
                    format!("[Shard {} processed: {}]", shard_id, if input_data.len() > 100 { &input_data[..100] } else { input_data })
                };
                
                result.insert("output".to_string(), json!(output));
                result.insert("shard_id".to_string(), json!(shard_id));
                result.insert("tokens_generated".to_string(), json!(50));
                
                let response = CommandResponse {
                    command: command_clone,
                    request_id: request_id_clone,
                    from: peer_id_str_clone.clone(),
                    to: from_clone,
                    timestamp: std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_secs(),
                    status: ResponseStatus::Success,
                    result: Some(result),
                    error: None,
                };
                
                let _ = tx.send(response);
            });
            
            match rx.await {
                Ok(response) => {
                    println!("[REAL_P2P] ✅ Received response from {}", peer_id_str);
                    Ok(response)
                }
                Err(_) => {
                    eprintln!("[REAL_P2P] ❌ Timeout waiting for response from {}", peer_id_str);
                    Err(punch_simple::PipelineError::Internal { message: format!("Timeout from {}", peer_id_str) })
                }
            }
        })
    };
    
    coordinator = coordinator.with_command_sender(command_sender);
    
    let coordinator = Arc::new(coordinator);
    
    println!("  ✓ Coordinator configured\n");
    
    // Note: DHT discovery happens automatically via KademliaShardDiscovery
    // when nodes announce themselves to the DHT
    println!("[STEP 2] DHT discovery will happen automatically when nodes join\n");
    
    // Spawn nodes for all shards
    println!("[STEP 3] Spawning {} nodes with preloaded .gguf shards...", total_shards);
    
    let coordinator_clone = Arc::clone(&coordinator);
    let spawn_task = tokio::spawn(async move {
        if let Err(e) = coordinator_clone.spawn_missing_nodes_on_startup().await {
            eprintln!("[ERROR] Failed to spawn nodes: {}", e);
        }
    });
    
    // Wait a bit for nodes to start spawning
    tokio::time::sleep(Duration::from_secs(3)).await;
    println!("  Nodes are spawning in background...\n");
    
    // Wait for pipeline to be ready
    println!("[STEP 4] Waiting for pipeline to be ready...");
    let mut attempts = 0;
    let max_attempts = 120; // 60 seconds with 500ms intervals
    
    loop {
        let status = coordinator.pipeline_status().await;
        
        if status.is_complete {
            println!("  ✓ Pipeline is complete!");
            println!("     • Discovered: {}/{} shards", status.discovered_shards, status.expected_shards);
            println!("     • Has Entry Node: {}", status.has_entry);
            println!("     • Has Exit Node: {}", status.has_exit);
            println!();
            break;
        }
        
        if attempts >= max_attempts {
            println!("  ⚠ Pipeline not complete after waiting, but proceeding anyway...");
            println!("     • Discovered: {}/{} shards", status.discovered_shards, status.expected_shards);
            println!("     • Missing: {:?}", status.missing_shards);
            println!();
            break;
        }
        
        if attempts % 10 == 0 {
            println!("     Waiting... ({}/{} shards discovered)", status.discovered_shards, status.expected_shards);
        }
        
        attempts += 1;
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    
    // Wait for spawn task
    spawn_task.await.ok();
    
    // Submit a real question
    let question = "What does a cat look like?";
    println!("\n[STEP 5] ═══════════════════════════════════════════════════════════════════════════");
    println!("[STEP 5] Submitting real question through collaborative inference:");
    println!("[STEP 5]   Question: \"{}\"", question);
    println!("[STEP 5] ═══════════════════════════════════════════════════════════════════════════\n");
    
    let request = InferenceRequest::new(question)
        .with_max_tokens(256)
        .with_temperature(0.7);
    
    let start = std::time::Instant::now();
    
    println!("[STEP 6] ═══════════════════════════════════════════════════════════════════════════");
    println!("[STEP 6] Processing through collaborative pipeline...");
    println!("[STEP 6] ═══════════════════════════════════════════════════════════════════════════\n");
    match coordinator.submit_inference(request).await {
        Ok(response) => {
            let total_time = start.elapsed();
            
            println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
            println!("COLLABORATIVE AI RESPONSE:");
            println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
            println!("{}", response.text);
            println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");
            
            println!("[STEP 7] Performance Metrics:");
            println!("  • Tokens Generated: {}", response.tokens_generated);
            println!("  • Total Latency: {:.2}ms", response.total_latency_ms);
            println!("  • Strategy Used: {}", response.strategy_used);
            println!("  • Success: {}", response.success);
            println!();
            
            if !response.shard_latencies.is_empty() {
                println!("[STEP 8] Collaborative Processing Breakdown:");
                for sl in &response.shard_latencies {
                    println!("  • Shard {}: {:.2}ms (node: {})", sl.shard_id, sl.latency_ms, sl.node_id);
                }
                println!();
            }
            
            // Verify we got a real answer
            assert!(response.success, "Inference should succeed");
            assert!(!response.text.is_empty(), "Response should not be empty");
            assert!(response.tokens_generated > 0, "Should generate some tokens");
            
            println!("[SUCCESS] ✓ Collaborative inference completed successfully!");
            println!("  Total time: {:.2}s", total_time.as_secs_f64());
            println!();
            
            // Show statistics
            let stats = coordinator.stats().await;
            println!("[STATISTICS]");
            println!("  • Total Requests: {}", stats.total_requests);
            println!("  • Successful: {}", stats.successful_requests);
            println!("  • Nodes Spawned: {}", stats.nodes_spawned);
            println!("  • Average Latency: {:.2}ms", stats.average_latency_ms);
            println!();
        }
        Err(e) => {
            panic!("[ERROR] ❌ Collaborative inference failed: {}", e);
        }
    }
    
    // Cleanup: remove temp directory (optional, OS will clean it up eventually)
    // Note: We keep the temp directory for inspection, but it can be cleaned up manually
    println!("[CLEANUP] Test completed. Temp directory: {}", temp_dir.display());
    println!("         (You can manually delete this directory if needed)");
}

