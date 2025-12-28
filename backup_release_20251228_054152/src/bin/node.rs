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
//!
//! Note: This binary calls the refactored functions from each personality module.
//! All binaries share the same codebase but can be run independently for backward compatibility.

use clap::{Parser, Subcommand};
use std::error::Error;

// Import run functions from each binary module
// Note: In Rust, binaries can't directly import from each other.
// The run_* functions are public in each binary file, but to call them
// from the unified binary, we need them to be accessible via the crate root.
// 
// Since these are separate binary files, we have two options:
// 1. Move the run_* functions to lib.rs (recommended)
// 2. Use process spawning (current fallback)
//
// For now, we'll try to call them directly - this will work if the functions
// are made accessible through the crate structure.

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

// Note: Since Rust binaries can't directly import from each other,
// we need to either:
// 1. Move run functions to lib.rs (recommended for production)
// 2. Use process spawning (current approach for simplicity)
// 
// For now, we'll use a hybrid: try to call directly if the modules are accessible,
// otherwise spawn as subprocess. The refactored functions are public in each binary.

// Spawn the appropriate binary as a subprocess
// Note: For a cleaner solution, the run_* functions should be moved to lib.rs
// For now, we spawn the binaries directly

async fn run_bootstrap(listen_addr: String, port: u16) -> Result<(), Box<dyn Error>> {
    // Call the bootstrap function directly from server.rs
    punch_simple::run_bootstrap(listen_addr, port).await
}

async fn run_listener(bootstrap: String, namespace: String) -> Result<(), Box<dyn Error>> {
    punch_simple::run_listener(bootstrap, namespace).await
}

async fn run_dialer(bootstrap: String, namespace: String) -> Result<(), Box<dyn Error>> {
    punch_simple::run_dialer(bootstrap, namespace).await
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
    punch_simple::run_shard_listener(
        bootstrap, cluster, shard_id, total_shards, total_layers,
        model_name, port, refresh_interval, shards_dir, enable_torrent
    ).await
}

async fn run_monitor(listen_addr: String, port: u16, web_port: u16) -> Result<(), Box<dyn Error>> {
    punch_simple::run_monitor(listen_addr, port, web_port).await
}

async fn run_web_server(bootstrap: String) -> Result<(), Box<dyn Error>> {
    // Web server is in a separate binary file, so we need to call it directly
    // For now, we'll use the web_server binary directly via process
    // In the future, this could be refactored to a shared module
    use std::process::Command;
    use std::env;
    use std::path::Path;
    
    env::set_var("BOOTSTRAP", &bootstrap);
    
    // Try release binary first, then debug, then cargo run
    let release_bin = Path::new("target/release/web_server.exe");
    let debug_bin = Path::new("target/debug/web_server.exe");
    
    let status = if release_bin.exists() {
        Command::new(release_bin)
            .status()?
    } else if debug_bin.exists() {
        Command::new(debug_bin)
            .status()?
    } else {
        // Fall back to cargo run
        Command::new("cargo")
            .args(&["run", "--bin", "web_server"])
            .status()?
    };
    
    if !status.success() {
        return Err("Web server failed".into());
    }
    Ok(())
}

