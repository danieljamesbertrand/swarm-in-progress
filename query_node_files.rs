//! Query a node for its available torrent files and loaded shards
//! Usage: cargo run --example query_node_files -- <peer_id>

use punch_simple::command_protocol::{Command, commands};
use serde_json::json;
use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    
    if args.len() < 2 {
        eprintln!("Usage: cargo run --example query_node_files -- <peer_id>");
        eprintln!("Example: cargo run --example query_node_files -- 12D3KooW...");
        return Ok(());
    }
    
    let peer_id = &args[1];
    
    println!("Querying node {} for files and capabilities...", peer_id);
    
    // TODO: Implement P2P command sending
    // This would require:
    // 1. Connect to bootstrap
    // 2. Discover node via DHT
    // 3. Send LIST_FILES command
    // 4. Send GET_CAPABILITIES command
    // 5. Display results
    
    println!("LIST_FILES command would return:");
    println!("  - All 4 shard files (shard-0.gguf through shard-3.gguf)");
    println!("  - With info_hash and size for each");
    
    println!("\nGET_CAPABILITIES command would return:");
    println!("  - shard_id: assigned shard");
    println!("  - capabilities.shard_loaded: true/false");
    println!("  - loaded_shards: list of loaded shard IDs");
    
    Ok(())
}

