//! Spawn Nodes and Get Collaborative Answer
//!
//! This example spawns multiple shard_listener nodes and gets them
//! to collaboratively answer "Describe a cat."

use punch_simple::{
    KademliaShardDiscovery,
    PipelineCoordinator, PipelineStrategy, NodeSpawner,
    InferenceRequest,
};
use std::sync::Arc;
use std::time::Duration;
use std::process::Command;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n");
    println!("╔══════════════════════════════════════════════════════════════════════════╗");
    println!("║   SPAWNING NODES FOR COLLABORATIVE ANSWER: \"Describe a cat.\"          ║");
    println!("╚══════════════════════════════════════════════════════════════════════════╝");
    println!();

    let bootstrap = "/ip4/127.0.0.1/tcp/51820";
    let cluster_name = "cat-answer-cluster";
    let total_shards = 4;
    let total_layers = 32;
    let model_name = "mistral-7b";
    let shards_dir = "models_cache/shards";

    println!("[STEP 1] Starting bootstrap server...");
    
    // Start bootstrap server in background
    let mut server_process = Command::new("cargo")
        .args(&["run", "--bin", "server", "--", "--port", "51820"])
        .spawn()
        .map_err(|e| format!("Failed to start server: {}. Make sure you're in the project directory.", e))?;
    
    println!("   ✓ Server started (PID: {})", server_process.id());
    println!("   Waiting 3 seconds for server to initialize...");
    tokio::time::sleep(Duration::from_secs(3)).await;
    println!();

    println!("[STEP 2] Setting up distributed pipeline coordinator...");
    
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
    
    // Use adaptive strategy
    coordinator.set_strategy(PipelineStrategy::Adaptive {
        wait_timeout_secs: 90,
        min_memory_for_shard_mb: 2048,
        min_memory_for_full_mb: 8192,
    });
    
    let coordinator = Arc::new(coordinator);
    
    println!("   ✓ Coordinator ready with node spawning enabled");
    println!();

    println!("[STEP 3] Spawning {} collaborative nodes...", total_shards);
    println!("   Each node will handle a portion of the model");
    println!();

    // Spawn nodes for all shards
    if let Err(e) = coordinator.spawn_missing_nodes_on_startup().await {
        eprintln!("[WARNING] Some nodes may have failed to spawn: {}", e);
        eprintln!("   Continuing anyway...");
    }

    println!("[STEP 4] Waiting for nodes to come online and join network...");
    
    // Wait for pipeline to be ready
    let mut ready = false;
    for attempt in 0..120 {
        let status = coordinator.pipeline_status().await;
        
        if status.is_complete {
            println!("   ✓ Pipeline complete! {}/{} shards online", 
                status.discovered_shards, status.expected_shards);
            ready = true;
            break;
        }
        
        if attempt % 10 == 0 {
            println!("   Waiting... ({}/{} shards discovered, attempt {})", 
                status.discovered_shards, status.expected_shards, attempt + 1);
        }
        
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    
    if !ready {
        println!("[WARNING] Pipeline not fully ready, but proceeding with available shards...");
    }
    println!();

    println!("[STEP 5] Submitting query: \"Describe a cat.\"");
    println!("[STEP 6] Nodes will collaborate to generate the answer...");
    println!();

    let request = InferenceRequest::new("Describe a cat.")
        .with_max_tokens(256)
        .with_temperature(0.7);

    let start = std::time::Instant::now();
    
    match coordinator.submit_inference(request).await {
        Ok(response) => {
            let total_time = start.elapsed();
            
            println!();
            println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
            println!("COLLABORATIVE AI ANSWER:");
            println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
            println!("{}", response.text);
            println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
            println!();
            
            println!("[STEP 7] Collaborative Processing Summary:");
            println!("   • Tokens Generated: {}", response.tokens_generated);
            println!("   • Total Latency: {:.2}ms", response.total_latency_ms);
            println!("   • Strategy Used: {}", response.strategy_used);
            println!("   • Success: {}", response.success);
            println!();
            
            if !response.shard_latencies.is_empty() {
                println!("[STEP 8] Node Contributions:");
                for sl in &response.shard_latencies {
                    let role = match sl.shard_id {
                        0 => "Entry (Tokenization + Embeddings + Layers 0-7)",
                        n if n == total_shards - 1 => "Exit (Final Layers + Output Head + Sampling)",
                        _ => "Middle (Transformer Layers)",
                    };
                    println!("   • Node {} (Shard {}): {:.2}ms - {}", 
                        sl.node_id, sl.shard_id, sl.latency_ms, role);
                }
                println!();
            }
            
            println!("[SUCCESS] ✓ Collaborative inference completed!");
            println!("   Total processing time: {:.2}s", total_time.as_secs_f64());
            println!();
            
            println!("[WHAT THE NODES DID COLLABORATIVELY]:");
            println!("   1. Node 0 (Entry): Received \"Describe a cat.\"");
            println!("      → Tokenized input into token IDs");
            println!("      → Converted tokens to embeddings");
            println!("      → Processed through layers 0-7");
            println!("      → Sent activations to Node 1");
            println!();
            println!("   2. Node 1 (Middle): Received activations from Node 0");
            println!("      → Processed through layers 8-15");
            println!("      → Sent activations to Node 2");
            println!();
            println!("   3. Node 2 (Middle): Received activations from Node 1");
            println!("      → Processed through layers 16-23");
            println!("      → Sent activations to Node 3");
            println!();
            println!("   4. Node 3 (Exit): Received activations from Node 2");
            println!("      → Processed through layers 24-31");
            println!("      → Applied output head → generated logits");
            println!("      → Sampled first token: \"A\" or \"Cats\"");
            println!("      → Sent token back through pipeline for next token");
            println!("      → Repeated for each token until complete");
            println!("      → Assembled final text response");
            println!();
            println!("   5. Response returned through coordinator to client");
            println!();
        }
        Err(e) => {
            println!("[ERROR] ❌ Collaborative inference failed!");
            println!("   Error: {}", e);
            println!();
            println!("[TROUBLESHOOTING]");
            println!("   • Check that nodes spawned successfully");
            println!("   • Verify bootstrap server is running");
            println!("   • Check node logs for errors");
            println!();
        }
    }

    // Show statistics
    let stats = coordinator.stats().await;
    println!("[FINAL STATISTICS]");
    println!("   • Total Requests: {}", stats.total_requests);
    println!("   • Successful: {}", stats.successful_requests);
    println!("   • Nodes Spawned: {}", stats.nodes_spawned);
    println!("   • Average Latency: {:.2}ms", stats.average_latency_ms);
    println!();

    // Cleanup: kill server
    println!("[CLEANUP] Stopping bootstrap server...");
    let _ = server_process.kill();
    server_process.wait().ok();
    println!("   ✓ Server stopped");
    println!();

    Ok(())
}




