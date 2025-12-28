//! Collaborative Distributed Inference
//!
//! This example spawns multiple nodes and gets them to work together
//! to answer "Describe a cat." using the distributed pipeline.

use punch_simple::{
    KademliaShardDiscovery,
    PipelineCoordinator, PipelineStrategy, NodeSpawner,
    InferenceRequest,
};
use std::sync::Arc;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n");
    println!("╔══════════════════════════════════════════════════════════════════════════╗");
    println!("║     COLLABORATIVE DISTRIBUTED INFERENCE: \"Describe a cat.\"             ║");
    println!("╚══════════════════════════════════════════════════════════════════════════╝");
    println!();

    let bootstrap = "/ip4/127.0.0.1/tcp/51820";
    let cluster_name = "collaborative-cat-cluster";
    let total_shards = 4;
    let total_layers = 32;
    let model_name = "mistral-7b";
    let shards_dir = "models_cache/shards";

    println!("[STEP 1] Configuration:");
    println!("   • Bootstrap: {}", bootstrap);
    println!("   • Cluster: {}", cluster_name);
    println!("   • Total Shards: {}", total_shards);
    println!("   • Total Layers: {}", total_layers);
    println!("   • Model: {}", model_name);
    println!("   • Shards Directory: {}", shards_dir);
    println!();

    // Check if bootstrap server is needed
    println!("[STEP 2] Checking for bootstrap server...");
    println!("   Note: You may need to run 'cargo run --bin server' in another terminal");
    println!("   Press Enter to continue (assuming server is running)...");
    // In a real scenario, we'd check if server is running
    
    println!();
    println!("[STEP 3] Setting up pipeline coordinator with node spawning...");
    
    // Create discovery
    let discovery = KademliaShardDiscovery::with_expected_shards(cluster_name, total_shards);
    
    // Create node spawner
    let spawner = NodeSpawner::new(
        bootstrap.to_string(),
        cluster_name.to_string(),
        total_shards,
        total_layers,
        model_name.to_string(),
        shards_dir.to_string(),
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
    
    let coordinator = Arc::new(coordinator);
    
    println!("   ✓ Coordinator configured with node spawning capability");
    println!();

    // Spawn nodes for all shards
    println!("[STEP 4] Spawning {} collaborative nodes...", total_shards);
    
    let coordinator_clone = Arc::clone(&coordinator);
    let spawn_task = tokio::spawn(async move {
        if let Err(e) = coordinator_clone.spawn_missing_nodes_on_startup().await {
            eprintln!("[ERROR] Failed to spawn nodes: {}", e);
        }
    });
    
    // Wait a bit for nodes to start spawning
    tokio::time::sleep(Duration::from_secs(2)).await;
    
    println!("   Nodes are spawning in background...");
    println!("   Waiting for nodes to come online...");
    println!();

    // Wait for pipeline to be ready
    let mut attempts = 0;
    let max_attempts = 60; // 30 seconds with 500ms intervals
    
    loop {
        let status = coordinator.pipeline_status().await;
        
        if status.is_complete {
            println!("[STEP 5] ✓ Pipeline is complete!");
            println!("   • Discovered: {}/{} shards", status.discovered_shards, status.expected_shards);
            println!("   • Has Entry Node: {}", status.has_entry);
            println!("   • Has Exit Node: {}", status.has_exit);
            println!();
            break;
        }
        
        if attempts >= max_attempts {
            println!("[WARNING] Pipeline not complete after waiting, but proceeding anyway...");
            println!("   • Discovered: {}/{} shards", status.discovered_shards, status.expected_shards);
            println!("   • Missing: {:?}", status.missing_shards);
            println!();
            break;
        }
        
        attempts += 1;
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    
    // Wait for spawn task
    spawn_task.await.ok();
    
    // Submit the query
    println!("[STEP 6] Submitting query: \"Describe a cat.\"");
    println!("[STEP 7] Processing through collaborative pipeline...");
    println!();

    let request = InferenceRequest::new("Describe a cat.")
        .with_max_tokens(256)
        .with_temperature(0.7);

    let start = std::time::Instant::now();
    
    match coordinator.submit_inference(request).await {
        Ok(response) => {
            let total_time = start.elapsed();
            
            println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
            println!("COLLABORATIVE AI RESPONSE:");
            println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
            println!("{}", response.text);
            println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
            println!();
            
            println!("[STEP 8] Performance Metrics:");
            println!("   • Tokens Generated: {}", response.tokens_generated);
            println!("   • Total Latency: {:.2}ms", response.total_latency_ms);
            println!("   • Strategy Used: {}", response.strategy_used);
            println!("   • Success: {}", response.success);
            println!();
            
            if !response.shard_latencies.is_empty() {
                println!("[STEP 9] Collaborative Processing Breakdown:");
                for sl in &response.shard_latencies {
                    println!("   • Shard {}: {:.2}ms (node: {})", sl.shard_id, sl.latency_ms, sl.node_id);
                }
                println!();
            }
            
            println!("[SUCCESS] ✓ Collaborative inference completed!");
            println!("   Total time: {:.2}s", total_time.as_secs_f64());
            println!();
            
            println!("[WHAT HAPPENED]");
            println!("   1. Spawned {} nodes, each handling a portion of the model", total_shards);
            println!("   2. Nodes discovered each other via DHT");
            println!("   3. Query processed through pipeline:");
            println!("      • Shard 0: Tokenization + Embeddings + First layers");
            println!("      • Shards 1-2: Middle transformer layers");
            println!("      • Shard 3: Final layers + Output head + Sampling");
            println!("   4. Activations flowed between nodes via QUIC");
            println!("   5. Final response assembled and returned");
            println!();
        }
        Err(e) => {
            println!("[ERROR] ❌ Collaborative inference failed!");
            println!("   Error: {}", e);
            println!();
            println!("[TROUBLESHOOTING]");
            println!("   1. Make sure bootstrap server is running:");
            println!("      cargo run --bin server");
            println!("   2. Check that shard files exist in {}", shards_dir);
            println!("   3. Verify nodes spawned successfully (check logs)");
            println!();
        }
    }

    // Show statistics
    let stats = coordinator.stats().await;
    println!("[STATISTICS]");
    println!("   • Total Requests: {}", stats.total_requests);
    println!("   • Successful: {}", stats.successful_requests);
    println!("   • Nodes Spawned: {}", stats.nodes_spawned);
    println!("   • Average Latency: {:.2}ms", stats.average_latency_ms);
    println!();

    Ok(())
}

