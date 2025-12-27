//! Punch Simple - P2P Network Library
//! 
//! This library provides P2P networking capabilities with:
//! - Kademlia DHT for peer discovery
//! - JSON command protocol for inter-node communication
//! - Weighted node selection based on capabilities
//! - Reputation tracking system
//! - Distributed Llama shard discovery and pipeline coordination
//! - **QUIC transport** (UDP-based, built-in TLS 1.3)
//! - TCP+Noise+Yamux fallback transport
//!
//! ## Transport Options
//! 
//! The library supports multiple transport types:
//! - `QuicOnly` - QUIC over UDP with built-in encryption
//! - `TcpOnly` - TCP with Noise encryption and Yamux multiplexing  
//! - `DualStack` - QUIC preferred, TCP fallback (default)
//!
//! ```rust,no_run
//! use punch_simple::quic_transport::{create_transport, TransportType};
//! use libp2p::identity::Keypair;
//!
//! let key = Keypair::generate_ed25519();
//! let transport = create_transport(&key, TransportType::DualStack).unwrap();
//! ```

pub mod message;
pub mod command_protocol;
pub mod command_validation;
pub mod protocol_logging;
pub mod capability_collector;
pub mod ai_inference_handler;
pub mod llama_fragment_processor;
pub mod llama_model_loader;
pub mod llama_inference;
pub mod kademlia_shard_discovery;
pub mod pipeline_coordinator;
pub mod quic_transport;
pub mod shard_optimization;

pub use message::{JsonMessage, JsonCodec};
pub use command_protocol::{Command, CommandResponse, NodeCapabilities, NodeWeights, ReputationData, ResponseStatus, commands};
pub use command_validation::{validate_command, ValidationError};
pub use protocol_logging::{
    log_connection_established, log_connection_closed, log_connection_failed,
    log_transaction_started, log_transaction_completed, log_transaction_failed, log_transaction_timeout,
};
pub use capability_collector::CapabilityCollector;
pub use ai_inference_handler::{AIInferenceRequest, process_ai_inference, create_ai_inference_response, create_ai_inference_error_response};
pub use llama_fragment_processor::{LlamaJob, LlamaFragment, FragmentResult, JobResult, process_fragment};
pub use llama_model_loader::{LlamaModelManager, RsyncConfig, ModelShard, create_model_manager};
pub use kademlia_shard_discovery::{
    KademliaShardDiscovery, ShardAnnouncement, ShardCapabilities, 
    ClusterMetadata, PipelineStatus, dht_keys
};
pub use pipeline_coordinator::{
    PipelineCoordinator, PipelineStrategy, PipelineError,
    InferenceRequest, InferenceResponse, CoordinatorState, CoordinatorStats,
    DynamicShardLoader, SingleNodeFallback, NodeSpawner,
};
pub use quic_transport::{
    create_quic_transport, create_tcp_transport, create_dual_transport,
    create_transport, TransportType, TransportError, TransportStats,
    get_listen_address, get_dual_listen_addresses,
};
pub use shard_optimization::{
    QuantizationType, ShardOptimization, OptimizationPriority,
    select_quantization,
};

// Re-export node runner functions for unified binary access
// These functions are defined in each binary file and made public
pub mod node_runners {
    // Note: These functions are actually in the binary files (server.rs, listener.rs, etc.)
    // For the unified binary to work, we need to either:
    // 1. Move these functions here, or
    // 2. Use process spawning, or  
    // 3. Create a shared module structure
    
    // For now, the unified binary will call them directly if they're in the same crate
    // In practice, you may need to move the run_* functions to this module
}





