//! Test Query: "Describe a cat."
//!
//! This example submits a specific query to the distributed AI system
//! and shows exactly what happens at each step.

use punch_simple::{
    KademliaShardDiscovery, ShardAnnouncement, ShardCapabilities,
    PipelineCoordinator, PipelineStrategy,
    InferenceRequest,
};
use std::time::Duration;

/// Create a test shard with configurable parameters
fn create_shard(
    shard_id: u32,
    total_shards: u32,
    total_layers: u32,
    memory_mb: u64,
    node_name: &str,
) -> ShardAnnouncement {
    let mut ann = ShardAnnouncement::new(
        node_name,
        shard_id,
        total_shards,
        total_layers,
        &format!("/ip4/10.0.0.{}/tcp/51820", shard_id + 1),
        "llama-8b-demo",
    );
    ann.capabilities = ShardCapabilities {
        cpu_cores: 8,
        cpu_usage: 25.0,
        memory_total_mb: memory_mb + 4096,
        memory_available_mb: memory_mb,
        gpu_memory_mb: 0,
        gpu_compute_units: 0,
        gpu_usage: 0.0,
        gpu_available: false,
        latency_ms: 10.0 + (shard_id as f64 * 2.0),
        reputation: 0.95,
        shard_loaded: true,
        active_requests: 0,
        max_concurrent: 4,
    };
    ann
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n");
    println!("╔══════════════════════════════════════════════════════════════════════════╗");
    println!("║              AI QUERY TEST: \"Describe a cat.\"                           ║");
    println!("╚══════════════════════════════════════════════════════════════════════════╝");
    println!();

    // Initialize the pipeline coordinator
    println!("[STEP 1] Initializing Pipeline Coordinator...");
    let discovery = KademliaShardDiscovery::with_expected_shards("cat-query-cluster", 4);
    let coordinator = PipelineCoordinator::new(discovery);
    println!("   ✓ Coordinator created");
    println!();

    // Add all shards to simulate a complete pipeline
    println!("[STEP 2] Building Distributed Pipeline...");
    println!("   Model: Llama-8B (simulated)");
    println!("   Shards: 4 (32 layers total, 8 layers per shard)");
    println!();
    
    for i in 0..4 {
        let shard = create_shard(i, 4, 32, 8192, &format!("node-{}", i));
        coordinator.add_shard(shard).await;
        
        let role = if i == 0 {
            "Entry Node (Embeddings + Layers 0-7)"
        } else if i == 3 {
            "Exit Node (Layers 24-31 + Output Head)"
        } else {
            "Middle Node (Layers)"
        };
        
        println!("   ✓ Shard {} added: {} - {}", i, format!("node-{}", i), role);
    }
    println!();

    // Check pipeline status
    println!("[STEP 3] Validating Pipeline Status...");
    let status = coordinator.pipeline_status().await;
    println!("   • Discovered: {}/{} shards", status.discovered_shards, status.expected_shards);
    println!("   • Has Entry Node: {}", status.has_entry);
    println!("   • Has Exit Node: {}", status.has_exit);
    println!("   • Complete: {}", status.is_complete);
    println!();

    // Submit the query
    println!("[STEP 4] Submitting Query: \"Describe a cat.\"");
    println!("   • Max Tokens: 256");
    println!("   • Temperature: 0.7");
    println!("   • Request ID: Generated");
    println!();

    let request = InferenceRequest::new("Describe a cat.")
        .with_max_tokens(256)
        .with_temperature(0.7);

    println!("[STEP 5] Processing Through Distributed Pipeline...");
    println!();
    
    let start = std::time::Instant::now();
    
    match coordinator.submit_inference(request).await {
        Ok(response) => {
            let total_time = start.elapsed();
            
            println!("[STEP 6] ✅ Inference Complete!");
            println!();
            println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
            println!("RESPONSE:");
            println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
            println!("{}", response.text);
            println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
            println!();
            
            println!("[STEP 7] Performance Metrics:");
            println!("   • Tokens Generated: {}", response.tokens_generated);
            println!("   • Total Latency: {:.2}ms", response.total_latency_ms);
            println!("   • Strategy Used: {}", response.strategy_used);
            println!("   • Success: {}", response.success);
            println!();
            
            println!("[STEP 8] Shard Processing Breakdown:");
            for (idx, sl) in response.shard_latencies.iter().enumerate() {
                let shard_role = match sl.shard_id {
                    0 => "Entry (Tokenization + Embeddings + Layers 0-7)",
                    n if n == response.shard_latencies.len() as u32 - 1 => "Exit (Layers 24-31 + Output Head + Sampling)",
                    _ => "Middle (Transformer Layers)",
                };
                
                println!("   Shard {}: {:.2}ms - {} ({})", 
                    sl.shard_id, 
                    sl.latency_ms,
                    shard_role,
                    sl.node_id
                );
            }
            println!();
            
            println!("[STEP 9] What the AI Did:");
            println!("   1. Received query: \"Describe a cat.\"");
            println!("   2. Tokenized input into token IDs");
            println!("   3. Converted tokens to embeddings (Shard 0)");
            println!("   4. Processed through Transformer layers 0-7 (Shard 0)");
            println!("   5. Transferred activations to Shard 1");
            println!("   6. Processed through Transformer layers 8-15 (Shard 1)");
            println!("   7. Transferred activations to Shard 2");
            println!("   8. Processed through Transformer layers 16-23 (Shard 2)");
            println!("   9. Transferred activations to Shard 3");
            println!("  10. Processed through Transformer layers 24-31 (Shard 3)");
            println!("  11. Applied output head to generate logits (Shard 3)");
            println!("  12. Sampled next token using temperature=0.7");
            println!("  13. Repeated steps 3-12 for each generated token");
            println!("  14. Assembled tokens into final text response");
            println!("  15. Returned complete response to client");
            println!();
            
            println!("Total Processing Time: {:.2}ms", total_time.as_secs_f64() * 1000.0);
        }
        Err(e) => {
            println!("[ERROR] ❌ Inference Failed!");
            println!("   Error: {}", e);
        }
    }

    // Show statistics
    let stats = coordinator.stats().await;
    println!();
    println!("[STATISTICS]");
    println!("   • Total Requests: {}", stats.total_requests);
    println!("   • Successful: {}", stats.successful_requests);
    println!("   • Average Latency: {:.2}ms", stats.average_latency_ms);
    println!();

    Ok(())
}




