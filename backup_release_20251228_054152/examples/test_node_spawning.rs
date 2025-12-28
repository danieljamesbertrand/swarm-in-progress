//! Test node spawning functionality
//! 
//! This example demonstrates how the pipeline coordinator can spawn nodes on demand
//! when shards are missing.

use punch_simple::pipeline_coordinator::{
    PipelineCoordinator, PipelineStrategy, InferenceRequest, NodeSpawner
};
use punch_simple::kademlia_shard_discovery::KademliaShardDiscovery;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Testing Node Spawning ===\n");

    // Create discovery
    let discovery = KademliaShardDiscovery::with_expected_shards("test-cluster", 4);

    // Create node spawner
    let spawner = NodeSpawner::new(
        "/ip4/127.0.0.1/tcp/51820".to_string(), // bootstrap
        "test-cluster".to_string(),
        4,  // total_shards
        32, // total_layers
        "llama-8b".to_string(),
        "models_cache/shards".to_string(),
    );

    // Create coordinator with spawner
    let mut coordinator = PipelineCoordinator::new(discovery)
        .with_node_spawner(spawner);

    // Set strategy to spawn nodes
    coordinator.set_strategy(PipelineStrategy::SpawnNodes {
        max_nodes_per_request: 4,
        min_memory_per_node_mb: 4096,
        spawn_command_template: "cargo run --bin shard_listener".to_string(),
        node_startup_timeout_secs: 30,
    });

    println!("[TEST] Coordinator configured with node spawning");
    println!("[TEST] Submitting inference request (will spawn missing shards)...\n");

    // Submit request - should spawn nodes for missing shards
    let request = InferenceRequest::new("What is AI?");
    let result = coordinator.submit_inference(request).await;

    match result {
        Ok(response) => {
            println!("[TEST] ✓ Inference successful!");
            println!("[TEST] Response: {}", response.text);
            println!("[TEST] Tokens: {}", response.tokens_generated);
            println!("[TEST] Latency: {:.2}ms", response.total_latency_ms);
        }
        Err(e) => {
            println!("[TEST] ✗ Inference failed: {}", e);
        }
    }

    // Get stats
    let stats = coordinator.stats().await;
    println!("\n[TEST] Statistics:");
    println!("  Total requests: {}", stats.total_requests);
    println!("  Successful: {}", stats.successful_requests);
    println!("  Nodes spawned: {}", stats.nodes_spawned);

    Ok(())
}





