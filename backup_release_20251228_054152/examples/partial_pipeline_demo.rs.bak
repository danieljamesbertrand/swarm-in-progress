//! Partial Pipeline Demo
//!
//! This example demonstrates how the PipelineCoordinator handles scenarios
//! where not all shards are available for distributed Llama inference.
//!
//! ## Scenarios Demonstrated:
//! 1. Complete pipeline - all shards available
//! 2. Incomplete pipeline with WaitAndRetry - waits for missing shards
//! 3. Incomplete pipeline with Dynamic Loading - loads shards on capable nodes
//! 4. Incomplete pipeline with Single-Node Fallback - uses high-memory node
//! 5. Adaptive strategy - tries all methods in sequence
//!
//! Usage:
//!   cargo run --example partial_pipeline_demo
//!
//! This example runs entirely in-memory without network connections,
//! simulating the discovery and coordination logic.

use punch_simple::{
    KademliaShardDiscovery, ShardAnnouncement, ShardCapabilities,
    PipelineCoordinator, PipelineStrategy, PipelineError,
    InferenceRequest, CoordinatorState,
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
        latency_ms: 10.0 + (shard_id as f64 * 2.0),
        reputation: 0.95,
        shard_loaded: true,
        active_requests: 0,
        max_concurrent: 4,
    };
    ann
}

fn print_header(title: &str) {
    println!();
    println!("╔══════════════════════════════════════════════════════════════════════════╗");
    println!("║  {:<72} ║", title);
    println!("╚══════════════════════════════════════════════════════════════════════════╝");
}

fn print_section(title: &str) {
    println!();
    println!("┌──────────────────────────────────────────────────────────────────────────┐");
    println!("│ {:<72} │", title);
    println!("└──────────────────────────────────────────────────────────────────────────┘");
}

fn print_state(state: &CoordinatorState) {
    match state {
        CoordinatorState::Ready => {
            println!("   State: ✅ READY - Pipeline complete, can process requests");
        }
        CoordinatorState::WaitingForShards { missing } => {
            println!("   State: ⏳ WAITING - Missing shards: {:?}", missing);
        }
        CoordinatorState::LoadingShards { loading } => {
            println!("   State: 📥 LOADING - Dynamically loading shards: {:?}", loading);
        }
        CoordinatorState::FallbackMode { node_id } => {
            println!("   State: 🔄 FALLBACK - Using single node: {}", node_id);
        }
        CoordinatorState::Unavailable { reason } => {
            println!("   State: ❌ UNAVAILABLE - {}", reason);
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    print_header("Partial Pipeline Handling Demo");
    println!();
    println!("This demo shows how the PipelineCoordinator handles incomplete pipelines");
    println!("when not all model shards are available for distributed inference.");
    println!();
    println!("Model Configuration:");
    println!("   • Model: llama-8b-demo (simulated)");
    println!("   • Total Shards: 4");
    println!("   • Layers per Shard: 8 (32 total)");
    println!("   • Shard Layout:");
    println!("     - Shard 0: Embeddings + Layers 0-7");
    println!("     - Shard 1: Layers 8-15");
    println!("     - Shard 2: Layers 16-23");
    println!("     - Shard 3: Layers 24-31 + Output Head");

    // =========================================================================
    // Scenario 1: Complete Pipeline
    // =========================================================================
    print_section("Scenario 1: Complete Pipeline (All Shards Available)");
    
    {
        let discovery = KademliaShardDiscovery::with_expected_shards("demo-cluster", 4);
        let coordinator = PipelineCoordinator::new(discovery);

        println!("\n   Adding all 4 shards to the pipeline...");
        for i in 0..4 {
            coordinator.add_shard(create_shard(i, 4, 32, 8192, &format!("node-{}", i))).await;
            println!("   ✓ Added shard {} (node-{})", i, i);
        }

        print_state(&coordinator.state().await);
        
        let status = coordinator.pipeline_status().await;
        println!("\n   Pipeline Status:");
        println!("   • Discovered: {}/{} shards", status.discovered_shards, status.expected_shards);
        println!("   • Has Entry Node: {}", status.has_entry);
        println!("   • Has Exit Node: {}", status.has_exit);
        println!("   • Complete: {}", status.is_complete);

        println!("\n   Submitting inference request...");
        let request = InferenceRequest::new("What is the meaning of life?")
            .with_max_tokens(100)
            .with_temperature(0.7);

        match coordinator.submit_inference(request).await {
            Ok(response) => {
                println!("\n   ✅ Inference Successful!");
                println!("   • Response: {}", response.text);
                println!("   • Tokens Generated: {}", response.tokens_generated);
                println!("   • Total Latency: {:.2}ms", response.total_latency_ms);
                println!("   • Strategy Used: {}", response.strategy_used);
                println!("   • Shard Latencies:");
                for sl in &response.shard_latencies {
                    println!("     - Shard {}: {:.2}ms ({})", sl.shard_id, sl.latency_ms, sl.node_id);
                }
            }
            Err(e) => println!("   ❌ Failed: {}", e),
        }

        let stats = coordinator.stats().await;
        println!("\n   Statistics:");
        println!("   • Total Requests: {}", stats.total_requests);
        println!("   • Successful: {}", stats.successful_requests);
        println!("   • Average Latency: {:.2}ms", stats.average_latency_ms);
    }

    // =========================================================================
    // Scenario 2: FailFast Strategy (Immediate failure)
    // =========================================================================
    print_section("Scenario 2: FailFast Strategy (Incomplete Pipeline)");
    
    {
        let discovery = KademliaShardDiscovery::with_expected_shards("demo-cluster", 4);
        let mut coordinator = PipelineCoordinator::new(discovery);
        coordinator.set_strategy(PipelineStrategy::FailFast);

        println!("\n   Adding only 2 of 4 shards (entry and exit nodes only)...");
        coordinator.add_shard(create_shard(0, 4, 32, 8192, "node-entry")).await;
        println!("   ✓ Added shard 0 (entry node with embeddings)");
        coordinator.add_shard(create_shard(3, 4, 32, 8192, "node-exit")).await;
        println!("   ✓ Added shard 3 (exit node with output head)");

        print_state(&coordinator.state().await);
        
        let status = coordinator.pipeline_status().await;
        println!("\n   Missing Shards: {:?}", status.missing_shards);

        println!("\n   Submitting inference request with FailFast strategy...");
        let request = InferenceRequest::new("Tell me a joke");

        match coordinator.submit_inference(request).await {
            Ok(_) => println!("   Unexpected success!"),
            Err(e) => {
                println!("\n   ❌ Request Failed (Expected Behavior)");
                println!("   • Error: {}", e);
                match e {
                    PipelineError::NoFallback { reason } => {
                        println!("   • Reason: {}", reason);
                    }
                    _ => {}
                }
            }
        }
    }

    // =========================================================================
    // Scenario 3: WaitAndRetry Strategy
    // =========================================================================
    print_section("Scenario 3: WaitAndRetry Strategy (Simulated Shard Arrival)");
    
    {
        let discovery = KademliaShardDiscovery::with_expected_shards("demo-cluster", 4);
        let mut coordinator = PipelineCoordinator::new(discovery);
        coordinator.set_strategy(PipelineStrategy::WaitAndRetry {
            timeout_secs: 5,
            retry_interval_ms: 500,
        });

        // Start with incomplete pipeline
        println!("\n   Starting with incomplete pipeline (shards 0, 2 only)...");
        coordinator.add_shard(create_shard(0, 4, 32, 8192, "node-0")).await;
        coordinator.add_shard(create_shard(2, 4, 32, 8192, "node-2")).await;
        
        print_state(&coordinator.state().await);

        // Spawn a task to add missing shards after a delay
        let coordinator_clone = std::sync::Arc::new(coordinator);
        let coordinator_for_task = coordinator_clone.clone();
        
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(1)).await;
            println!("\n   [Background] Adding shard 1...");
            coordinator_for_task.add_shard(create_shard(1, 4, 32, 8192, "node-1")).await;
            
            tokio::time::sleep(Duration::from_millis(500)).await;
            println!("   [Background] Adding shard 3...");
            coordinator_for_task.add_shard(create_shard(3, 4, 32, 8192, "node-3")).await;
        });

        println!("\n   Submitting request (will wait for missing shards)...");
        let request = InferenceRequest::new("Explain quantum computing");

        match coordinator_clone.submit_inference(request).await {
            Ok(response) => {
                println!("\n   ✅ Inference Successful after waiting!");
                println!("   • Strategy: {}", response.strategy_used);
                println!("   • Total Latency: {:.2}ms (includes wait time)", response.total_latency_ms);
            }
            Err(e) => {
                println!("\n   ❌ Timeout: {}", e);
            }
        }

        let stats = coordinator_clone.stats().await;
        println!("\n   Queue Statistics:");
        println!("   • Queued Requests: {}", stats.queued_requests);
        println!("   • Avg Queue Time: {:.2}ms", stats.average_queue_time_ms);
    }

    // =========================================================================
    // Scenario 4: Single-Node Fallback
    // =========================================================================
    print_section("Scenario 4: Single-Node Fallback (High-Memory Node Available)");
    
    {
        let discovery = KademliaShardDiscovery::with_expected_shards("demo-cluster", 4);
        let mut coordinator = PipelineCoordinator::new(discovery);
        coordinator.set_strategy(PipelineStrategy::SingleNodeFallback {
            required_memory_mb: 16000,  // 16GB required for full model
        });

        println!("\n   Adding only entry node, but with HIGH memory (32GB)...");
        let mut high_mem_node = create_shard(0, 4, 32, 32000, "high-memory-node");
        high_mem_node.capabilities.memory_available_mb = 32000;
        coordinator.add_shard(high_mem_node).await;
        println!("   ✓ Added shard 0 on high-memory-node (32GB available)");

        print_state(&coordinator.state().await);

        println!("\n   Pipeline is incomplete, but fallback node is available!");
        let status = coordinator.pipeline_status().await;
        println!("   Missing Shards: {:?}", status.missing_shards);

        println!("\n   Submitting inference request...");
        let request = InferenceRequest::new("Write a haiku about AI");

        match coordinator.submit_inference(request).await {
            Ok(response) => {
                println!("\n   ✅ Inference Successful via Fallback!");
                println!("   • Strategy: {}", response.strategy_used);
                println!("   • Response: {}", response.text);
                println!("   • Processed on: {}", response.shard_latencies[0].node_id);
            }
            Err(e) => println!("   ❌ Failed: {}", e),
        }

        let stats = coordinator.stats().await;
        println!("\n   Fallback Statistics:");
        println!("   • Fallback Requests: {}", stats.fallback_requests);
    }

    // =========================================================================
    // Scenario 5: Adaptive Strategy
    // =========================================================================
    print_section("Scenario 5: Adaptive Strategy (Tries Multiple Methods)");
    
    {
        let discovery = KademliaShardDiscovery::with_expected_shards("demo-cluster", 4);
        let mut coordinator = PipelineCoordinator::new(discovery);
        coordinator.set_strategy(PipelineStrategy::Adaptive {
            wait_timeout_secs: 2,
            min_memory_for_shard_mb: 4096,
            min_memory_for_full_mb: 16000,
        });

        println!("\n   Adaptive Strategy Order:");
        println!("   1. Try Dynamic Shard Loading");
        println!("   2. Wait for Missing Shards (2s timeout)");
        println!("   3. Fall back to Single-Node if available");

        println!("\n   Adding partial pipeline with one high-memory node...");
        coordinator.add_shard(create_shard(0, 4, 32, 8192, "node-0")).await;
        
        let mut capable_node = create_shard(1, 4, 32, 24000, "capable-node");
        capable_node.capabilities.memory_available_mb = 24000;
        coordinator.add_shard(capable_node).await;
        println!("   ✓ Added shard 0 (8GB) and shard 1 (24GB capable node)");

        print_state(&coordinator.state().await);

        println!("\n   Submitting inference request...");
        let request = InferenceRequest::new("What are the benefits of distributed computing?");

        let start = std::time::Instant::now();
        match coordinator.submit_inference(request).await {
            Ok(response) => {
                println!("\n   ✅ Inference Successful!");
                println!("   • Strategy Used: {}", response.strategy_used);
                println!("   • Time to Complete: {:.2}ms", start.elapsed().as_millis());
            }
            Err(e) => {
                println!("\n   Strategy exhausted: {}", e);
            }
        }
    }

    // =========================================================================
    // Summary
    // =========================================================================
    print_section("Summary: Strategy Selection Guide");

    println!();
    println!("   ┌─────────────────────┬─────────────────────────────────────────────────┐");
    println!("   │ Strategy            │ Use Case                                         │");
    println!("   ├─────────────────────┼─────────────────────────────────────────────────┤");
    println!("   │ FailFast            │ Low latency requirements, no tolerance for delay│");
    println!("   │ WaitAndRetry        │ Can wait, shards expected to come online soon   │");
    println!("   │ DynamicLoading      │ Nodes have spare capacity to load extra shards  │");
    println!("   │ SingleNodeFallback  │ One powerful node can run full model            │");
    println!("   │ Adaptive            │ Best for production - tries all strategies      │");
    println!("   └─────────────────────┴─────────────────────────────────────────────────┘");
    println!();
    println!("   Pipeline Parallelism Requirements:");
    println!("   • Entry Node (Shard 0): REQUIRED - Contains embedding layer");
    println!("   • Middle Shards: ALL REQUIRED - Sequential layer dependencies");  
    println!("   • Exit Node (Last Shard): REQUIRED - Contains output head");
    println!();
    println!("   Without ALL shards, you need:");
    println!("   • A node capable of running the FULL model (SingleNodeFallback)");
    println!("   • Or the ability to dynamically load shards (DynamicLoading)");
    println!("   • Or patience to wait for nodes to join (WaitAndRetry)");
    println!();

    print_header("Demo Complete!");

    Ok(())
}

