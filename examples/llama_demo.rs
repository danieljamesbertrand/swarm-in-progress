//! Promethos-AI Swarm - Llama Distributed Inference Demo
//! 
//! This demo simulates a complete distributed Llama inference pipeline:
//! - 4 shard nodes (each handling part of the model)
//! - Pipeline coordination
//! - Query processing with realistic timings
//! - Response generation
//!
//! Run with: cargo run --example llama_demo

use std::time::{Duration, Instant};
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::sleep;

/// Simulated shard node
#[derive(Debug, Clone)]
struct ShardNode {
    shard_id: u32,
    layer_start: u32,
    layer_end: u32,
    has_embeddings: bool,
    has_output: bool,
    processing_time_ms: u64,
}

impl ShardNode {
    fn new(shard_id: u32, total_shards: u32, total_layers: u32) -> Self {
        let layers_per_shard = total_layers / total_shards;
        let layer_start = shard_id * layers_per_shard;
        let layer_end = if shard_id == total_shards - 1 {
            total_layers
        } else {
            (shard_id + 1) * layers_per_shard
        };
        
        Self {
            shard_id,
            layer_start,
            layer_end,
            has_embeddings: shard_id == 0,
            has_output: shard_id == total_shards - 1,
            processing_time_ms: 100 + (shard_id as u64 * 50), // Vary by shard
        }
    }

    async fn process(&self, input: &str) -> String {
        let start = Instant::now();
        
        // Simulate processing time
        sleep(Duration::from_millis(self.processing_time_ms)).await;
        
        let elapsed = start.elapsed().as_millis();
        
        if self.has_embeddings {
            println!("  🧠 Shard {} [Layers {}-{}] Tokenized & embedded input ({} tokens) in {}ms",
                self.shard_id, self.layer_start, self.layer_end,
                input.split_whitespace().count() * 2, elapsed);
        } else if self.has_output {
            println!("  🧠 Shard {} [Layers {}-{}] Generated output tokens in {}ms",
                self.shard_id, self.layer_start, self.layer_end, elapsed);
        } else {
            println!("  🧠 Shard {} [Layers {}-{}] Processed hidden states in {}ms",
                self.shard_id, self.layer_start, self.layer_end, elapsed);
        }
        
        format!("shard_{}_output", self.shard_id)
    }
}

/// Simulated distributed pipeline
struct DistributedPipeline {
    shards: Vec<ShardNode>,
    model_name: String,
}

impl DistributedPipeline {
    fn new(model_name: &str, total_shards: u32, total_layers: u32) -> Self {
        let shards: Vec<ShardNode> = (0..total_shards)
            .map(|id| ShardNode::new(id, total_shards, total_layers))
            .collect();
        
        Self {
            shards,
            model_name: model_name.to_string(),
        }
    }

    async fn inference(&self, query: &str) -> InferenceResult {
        let total_start = Instant::now();
        
        println!("\n╔══════════════════════════════════════════════════════════════╗");
        println!("║          PROMETHOS-AI DISTRIBUTED INFERENCE                   ║");
        println!("╚══════════════════════════════════════════════════════════════╝\n");
        
        println!("📝 Query: \"{}\"", query);
        println!("🤖 Model: {} (Q4_K_M quantization)", self.model_name);
        println!("🔗 Pipeline: {} shards across {} layers\n", self.shards.len(), 
            self.shards.last().map(|s| s.layer_end).unwrap_or(0));
        
        println!("┌─────────────────────────────────────────────────────────────┐");
        println!("│                    PIPELINE EXECUTION                        │");
        println!("└─────────────────────────────────────────────────────────────┘");
        
        // Process through each shard sequentially (pipeline parallelism)
        let mut current_output = query.to_string();
        for shard in &self.shards {
            current_output = shard.process(&current_output).await;
        }
        
        let total_time = total_start.elapsed();
        
        // Generate a contextual response based on the query
        let response = generate_response(query);
        let tokens = response.split_whitespace().count();
        
        println!("\n┌─────────────────────────────────────────────────────────────┐");
        println!("│                      INFERENCE COMPLETE                       │");
        println!("└─────────────────────────────────────────────────────────────┘");
        println!("  ⏱️  Total latency: {:?}", total_time);
        println!("  📊 Tokens generated: {}", tokens);
        println!("  🚀 Throughput: {:.1} tokens/sec", tokens as f64 / total_time.as_secs_f64());
        
        InferenceResult {
            response,
            latency: total_time,
            tokens_generated: tokens,
        }
    }
}

struct InferenceResult {
    response: String,
    latency: Duration,
    tokens_generated: usize,
}

/// Generate contextual responses for demo
fn generate_response(query: &str) -> String {
    let query_lower = query.to_lowercase();
    
    if query_lower.contains("pinball wizard") {
        return "Pete Townshend wrote \"Pinball Wizard\" for The Who's 1969 rock opera \"Tommy\". \
                The song tells the story of a \"deaf, dumb and blind kid\" who becomes a pinball champion. \
                It was released as a single and reached #4 on the UK charts and #19 in the US. \
                Elton John later covered it for the 1975 film adaptation.".to_string();
    }
    
    if query_lower.contains("hello") || query_lower.contains("hi") {
        return "Hello! I'm an AI assistant running on the Promethos-AI distributed swarm network. \
                My responses are generated by processing your query through multiple nodes, \
                each handling different layers of the neural network. How can I help you today?".to_string();
    }
    
    if query_lower.contains("what is") && query_lower.contains("promethos") {
        return "Promethos-AI Swarm is a distributed AI inference system that splits large language models \
                across multiple nodes. Each node processes a portion of the model's layers, allowing \
                the network to run models that wouldn't fit on a single machine. The name 'Promethos' \
                references the Greek titan Prometheus who brought fire to humanity - similarly, we're \
                bringing powerful AI capabilities to everyone through distributed computing.".to_string();
    }
    
    if query_lower.contains("capital") {
        if query_lower.contains("france") {
            return "The capital of France is Paris. Paris is located in the north-central part of France \
                    along the Seine River. It's known as the 'City of Light' and is home to famous \
                    landmarks like the Eiffel Tower, the Louvre, and Notre-Dame Cathedral.".to_string();
        }
        if query_lower.contains("japan") {
            return "The capital of Japan is Tokyo. Tokyo is the most populous metropolitan area in the world \
                    and serves as Japan's political, economic, and cultural center. It's known for its \
                    blend of traditional temples and ultramodern architecture.".to_string();
        }
    }
    
    if query_lower.contains("code") || query_lower.contains("rust") || query_lower.contains("program") {
        return "Here's an example of a simple Rust program:\n\n\
                ```rust\n\
                fn main() {\n    \
                    println!(\"Hello from Promethos-AI!\");\n    \
                    let numbers: Vec<i32> = (1..=10).collect();\n    \
                    let sum: i32 = numbers.iter().sum();\n    \
                    println!(\"Sum of 1-10: {}\", sum);\n\
                }\n\
                ```\n\n\
                This demonstrates Rust's iterator patterns and type inference.".to_string();
    }
    
    if query_lower.contains("weather") {
        return "I don't have access to real-time weather data, but I can help you understand weather patterns! \
                Weather is determined by factors like atmospheric pressure, humidity, temperature, and wind. \
                For current weather information, I'd recommend checking a service like weather.gov or a weather app.".to_string();
    }
    
    // Default response
    format!("Based on your query \"{}\", I've processed this through the distributed Llama pipeline. \
            The inference was distributed across {} neural network shards, each handling different \
            transformer layers. This distributed approach allows running large language models \
            that wouldn't fit on a single machine. Is there anything specific you'd like to know?", 
            query, 4)
}

/// Interactive query loop
async fn interactive_mode(pipeline: &DistributedPipeline) {
    use std::io::{self, Write};
    
    println!("\n");
    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║         🔥 PROMETHOS-AI SWARM - INTERACTIVE MODE 🔥          ║");
    println!("╠══════════════════════════════════════════════════════════════╣");
    println!("║  Type your questions below. Type 'quit' or 'exit' to stop.   ║");
    println!("║  Try asking about: Pinball Wizard, capitals, Rust code, etc. ║");
    println!("╚══════════════════════════════════════════════════════════════╝\n");
    
    loop {
        print!("\n🔥 You: ");
        io::stdout().flush().unwrap();
        
        let mut input = String::new();
        if io::stdin().read_line(&mut input).is_err() {
            break;
        }
        
        let query = input.trim();
        
        if query.is_empty() {
            continue;
        }
        
        if query.eq_ignore_ascii_case("quit") || query.eq_ignore_ascii_case("exit") {
            println!("\n👋 Goodbye! Thanks for using Promethos-AI Swarm.\n");
            break;
        }
        
        let result = pipeline.inference(query).await;
        
        println!("\n┌─────────────────────────────────────────────────────────────┐");
        println!("│                         RESPONSE                             │");
        println!("└─────────────────────────────────────────────────────────────┘");
        println!("\n🤖 Promethos: {}\n", result.response);
    }
}

#[tokio::main]
async fn main() {
    println!("\n");
    println!("    ██████╗ ██████╗  ██████╗ ███╗   ███╗███████╗████████╗██╗  ██╗ ██████╗ ███████╗");
    println!("    ██╔══██╗██╔══██╗██╔═══██╗████╗ ████║██╔════╝╚══██╔══╝██║  ██║██╔═══██╗██╔════╝");
    println!("    ██████╔╝██████╔╝██║   ██║██╔████╔██║█████╗     ██║   ███████║██║   ██║███████╗");
    println!("    ██╔═══╝ ██╔══██╗██║   ██║██║╚██╔╝██║██╔══╝     ██║   ██╔══██║██║   ██║╚════██║");
    println!("    ██║     ██║  ██║╚██████╔╝██║ ╚═╝ ██║███████╗   ██║   ██║  ██║╚██████╔╝███████║");
    println!("    ╚═╝     ╚═╝  ╚═╝ ╚═════╝ ╚═╝     ╚═╝╚══════╝   ╚═╝   ╚═╝  ╚═╝ ╚═════╝ ╚══════╝");
    println!("                        🔥 AI SWARM - Distributed Intelligence 🔥");
    println!("\n");
    
    // Initialize the distributed pipeline
    println!("🚀 Initializing distributed Llama pipeline...\n");
    
    let pipeline = DistributedPipeline::new("Llama-8B", 4, 32);
    
    println!("✅ Pipeline ready with {} shards:", pipeline.shards.len());
    for shard in &pipeline.shards {
        let role = if shard.has_embeddings {
            "📥 [Entry - Embeddings]"
        } else if shard.has_output {
            "📤 [Exit - Output Head]"
        } else {
            "🔄 [Hidden Layers]"
        };
        println!("   Shard {}: Layers {}-{} {}", 
            shard.shard_id, shard.layer_start, shard.layer_end, role);
    }
    
    // Check for command line arguments
    let args: Vec<String> = std::env::args().collect();
    
    if args.len() > 1 {
        // Single query mode
        let query = args[1..].join(" ");
        let result = pipeline.inference(&query).await;
        
        println!("\n┌─────────────────────────────────────────────────────────────┐");
        println!("│                         RESPONSE                             │");
        println!("└─────────────────────────────────────────────────────────────┘");
        println!("\n{}\n", result.response);
    } else {
        // Interactive mode
        interactive_mode(&pipeline).await;
    }
}








