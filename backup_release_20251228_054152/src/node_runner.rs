//! Shared node runner - Common code for all node personalities
//! 
//! This module provides shared functionality for all node types to keep the codebase simple.

use std::error::Error;

/// Node personality/mode
#[derive(Debug, Clone)]
pub enum NodeMode {
    Bootstrap { listen_addr: String, port: u16 },
    Listener { bootstrap: String, namespace: String },
    Dialer { bootstrap: String, namespace: String },
    ShardListener {
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
    },
    Monitor { listen_addr: String, port: u16, web_port: u16 },
    WebServer { bootstrap: String },
}

/// Run a node with the specified mode
pub async fn run_node(mode: NodeMode) -> Result<(), Box<dyn Error>> {
    match mode {
        NodeMode::Bootstrap { listen_addr, port } => {
            // This will call the refactored server logic
            // For now, delegate to the server binary's main
            run_bootstrap_mode(listen_addr, port).await
        }
        NodeMode::Listener { bootstrap, namespace } => {
            run_listener_mode(bootstrap, namespace).await
        }
        NodeMode::Dialer { bootstrap, namespace } => {
            run_dialer_mode(bootstrap, namespace).await
        }
        NodeMode::ShardListener { .. } => {
            // Shard listener is complex, will need full refactor
            todo!("Shard listener mode - needs refactoring")
        }
        NodeMode::Monitor { .. } => {
            todo!("Monitor mode - needs refactoring")
        }
        NodeMode::WebServer { .. } => {
            todo!("Web server mode - needs refactoring")
        }
    }
}

// Placeholder functions - these will be implemented by calling the refactored binaries
async fn run_bootstrap_mode(listen_addr: String, port: u16) -> Result<(), Box<dyn Error>> {
    // TODO: Import and call server.rs::run_bootstrap once it's public
    eprintln!("Bootstrap mode: {}:{}", listen_addr, port);
    Ok(())
}

async fn run_listener_mode(bootstrap: String, namespace: String) -> Result<(), Box<dyn Error>> {
    eprintln!("Listener mode: bootstrap={}, namespace={}", bootstrap, namespace);
    Ok(())
}

async fn run_dialer_mode(bootstrap: String, namespace: String) -> Result<(), Box<dyn Error>> {
    eprintln!("Dialer mode: bootstrap={}, namespace={}", bootstrap, namespace);
    Ok(())
}


