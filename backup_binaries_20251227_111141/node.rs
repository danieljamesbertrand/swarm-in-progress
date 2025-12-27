//! Unified Node - Single binary with multiple personalities
//! 
//! This is the unified entry point for all node types. Use --mode to select personality:
//!   - bootstrap: Bootstrap/DHT server node
//!   - listener: Generic peer that waits for connections
//!   - dialer: Peer that discovers and connects to others
//!   - shard-listener: AI inference node for distributed Llama models
//!   - monitor: Network monitoring dashboard
//!   - web-server: Web interface for AI inference
//!
//! Usage examples:
//!   cargo run --bin node -- bootstrap --listen-addr 0.0.0.0 --port 51820
//!   cargo run --bin node -- shard-listener --shard-id 0 --total-shards 4
//!   cargo run --bin node -- listener --namespace my-app

use clap::{Parser, Subcommand};
use std::error::Error;

// Import the run functions from each binary module
// Note: We'll need to make these functions public in each module

#[derive(Parser)]
#[command(name = "node")]
#[command(about = "Unified P2P Node - Single binary with multiple personalities")]
struct Cli {
    #[command(subcommand)]
    mode: Mode,
}

#[derive(Subcommand)]
enum Mode {
    /// Bootstrap/DHT server node
    Bootstrap {
        /// Listen address (default: 0.0.0.0)
        #[arg(long, default_value = "0.0.0.0")]
        listen_addr: String,
        /// Listen port (default: 51820)
        #[arg(long, default_value = "51820")]
        port: u16,
    },
    /// Generic peer that waits for connections
    Listener {
        /// Bootstrap node address (Multiaddr format)
        #[arg(long, default_value = "/ip4/127.0.0.1/tcp/51820")]
        bootstrap: String,
        /// Namespace for peer discovery
        #[arg(long, default_value = "simple-chat")]
        namespace: String,
    },
    /// Peer that discovers and connects to others
    Dialer {
        /// Bootstrap node address (Multiaddr format)
        #[arg(long, default_value = "/ip4/127.0.0.1/tcp/51820")]
        bootstrap: String,
        /// Namespace for peer discovery
        #[arg(long, default_value = "simple-chat")]
        namespace: String,
    },
    /// AI inference node for distributed Llama models
    ShardListener {
        /// Bootstrap node address (Multiaddr format)
        #[arg(long, default_value = "/ip4/127.0.0.1/tcp/51820")]
        bootstrap: String,
        /// Cluster name for shard discovery
        #[arg(long, default_value = "llama-cluster")]
        cluster: String,
        /// Shard ID for this node (0, 1, 2, ...)
        #[arg(long, env = "LLAMA_SHARD_ID")]
        shard_id: Option<u32>,
        /// Total number of shards in cluster
        #[arg(long, env = "LLAMA_TOTAL_SHARDS", default_value = "4")]
        total_shards: u32,
        /// Total layers in the model
        #[arg(long, env = "LLAMA_TOTAL_LAYERS", default_value = "32")]
        total_layers: u32,
        /// Model name
        #[arg(long, env = "LLAMA_MODEL_NAME", default_value = "llama-8b")]
        model_name: String,
        /// Listen port (0 for random)
        #[arg(long, default_value = "0")]
        port: u16,
        /// Announcement refresh interval in seconds
        #[arg(long, default_value = "60")]
        refresh_interval: u64,
        /// Directory containing GGUF shards
        #[arg(long, env = "LLAMA_SHARDS_DIR", default_value = "models_cache/shards")]
        shards_dir: String,
        /// Enable torrent server to seed all GGUF files
        #[arg(long, default_value = "true")]
        enable_torrent: bool,
    },
    /// Network monitoring dashboard
    Monitor {
        /// Listen address for bootstrap (default: 0.0.0.0)
        #[arg(long, default_value = "0.0.0.0")]
        listen_addr: String,
        /// Listen port for bootstrap (default: 51820)
        #[arg(long, default_value = "51820")]
        port: u16,
        /// Web dashboard port (default: 8080)
        #[arg(long, default_value = "8080")]
        web_port: u16,
    },
    /// Web interface for AI inference
    WebServer {
        /// Bootstrap node address (Multiaddr format)
        #[arg(long, env = "BOOTSTRAP", default_value = "/ip4/127.0.0.1/tcp/51820")]
        bootstrap: String,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let cli = Cli::parse();

    match cli.mode {
        Mode::Bootstrap { listen_addr, port } => {
            // Delegate to server.rs logic
            run_bootstrap(listen_addr, port).await
        }
        Mode::Listener { bootstrap, namespace } => {
            // Delegate to listener.rs logic
            run_listener(bootstrap, namespace).await
        }
        Mode::Dialer { bootstrap, namespace } => {
            // Delegate to dialer.rs logic
            run_dialer(bootstrap, namespace).await
        }
        Mode::ShardListener {
            bootstrap,
            cluster,
            shard_id,
            total_shards,
            total_layers,
            model_name,
            port,
            refresh_interval,
            shards_dir,
            enable_torrent,
        } => {
            // Delegate to shard_listener.rs logic
            run_shard_listener(
                bootstrap, cluster, shard_id, total_shards, total_layers,
                model_name, port, refresh_interval, shards_dir, enable_torrent
            ).await
        }
        Mode::Monitor { listen_addr, port, web_port } => {
            // Delegate to monitor.rs logic
            run_monitor(listen_addr, port, web_port).await
        }
        Mode::WebServer { bootstrap } => {
            // Delegate to web_server.rs logic
            run_web_server(bootstrap).await
        }
    }
}

// Re-export the main functions from each binary
// These will be implemented by importing/calling the existing code

async fn run_bootstrap(listen_addr: String, port: u16) -> Result<(), Box<dyn Error>> {
    // Call the refactored function from server.rs
    // Note: server.rs needs to be made a module or we need to use a different approach
    // For now, we'll spawn the server binary as a subprocess or use a shared library approach
    // Actually, better: make server.rs a module in lib.rs and export the function
    println!("Starting bootstrap node on {}:{}", listen_addr, port);
    
    // TODO: Import from server module once it's refactored
    // For now, this is a placeholder that shows the structure
    eprintln!("Note: Need to refactor server.rs to be callable from here");
    eprintln!("Options:");
    eprintln!("  1. Make server.rs a module and export run_bootstrap()");
    eprintln!("  2. Use std::process::Command to spawn the server binary");
    eprintln!("  3. Create a shared node_runner module");
    
    // Temporary: spawn the server binary
    use std::process::Command;
    Command::new("cargo")
        .args(&["run", "--bin", "server", "--", "--listen-addr", &listen_addr, "--port", &port.to_string()])
        .spawn()?
        .wait()?;
    
    Ok(())
}

async fn run_listener(bootstrap: String, namespace: String) -> Result<(), Box<dyn Error>> {
    println!("Starting listener with bootstrap: {}, namespace: {}", bootstrap, namespace);
    println!("Note: This is a placeholder. Need to refactor listener.rs to expose run_listener()");
    Ok(())
}

async fn run_dialer(bootstrap: String, namespace: String) -> Result<(), Box<dyn Error>> {
    println!("Starting dialer with bootstrap: {}, namespace: {}", bootstrap, namespace);
    println!("Note: This is a placeholder. Need to refactor dialer.rs to expose run_dialer()");
    Ok(())
}

async fn run_shard_listener(
    bootstrap: String,
    cluster: String,
    shard_id: Option<u32>,
    total_shards: u32,
    total_layers: u32,
    model_name: String,
    port: u16,
    refresh_interval: u64,
    shards_dir: String,
    enable_torrent: bool,
) -> Result<(), Box<dyn Error>> {
    println!("Starting shard listener:");
    println!("  Bootstrap: {}", bootstrap);
    println!("  Cluster: {}", cluster);
    println!("  Shard ID: {:?}", shard_id);
    println!("  Total shards: {}", total_shards);
    println!("Note: This is a placeholder. Need to refactor shard_listener.rs to expose run_shard_listener()");
    Ok(())
}

async fn run_monitor(listen_addr: String, port: u16, web_port: u16) -> Result<(), Box<dyn Error>> {
    println!("Starting monitor:");
    println!("  Bootstrap: {}:{}", listen_addr, port);
    println!("  Web dashboard: {}", web_port);
    println!("Note: This is a placeholder. Need to refactor monitor.rs to expose run_monitor()");
    Ok(())
}

async fn run_web_server(bootstrap: String) -> Result<(), Box<dyn Error>> {
    println!("Starting web server with bootstrap: {}", bootstrap);
    println!("Note: This is a placeholder. Need to refactor web_server.rs to expose run_web_server()");
    Ok(())
}

