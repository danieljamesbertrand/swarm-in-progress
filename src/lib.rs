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

#![allow(warnings)]
#![allow(dead_code)]
#![allow(unused_variables)]
#![allow(unused_imports)]
#![allow(unused_mut)]
#![allow(unused_assignments)]
#![allow(unused_must_use)]
#![allow(clippy::all)]

// Allow referring to this crate by its package name (`punch_simple::...`) even
// from within the crate itself. This keeps shared source files usable both as
// library modules and as standalone `[[bin]]` crate roots.
extern crate self as punch_simple;

pub mod ai_inference_handler;
pub mod capability_collector;
pub mod command_protocol;
pub mod command_validation;
pub mod kademlia_shard_discovery;
pub mod llama_cpp_backend;
pub mod llama_fragment_processor;
pub mod llama_inference;
pub mod llama_model_loader;
pub mod message;
pub mod metrics;
pub mod pipeline_coordinator;
pub mod protocol_logging;
pub mod quic_transport;
pub mod quic_diagnostics;
pub mod shard_optimization;
pub mod shard_loader;

pub use ai_inference_handler::{
    create_ai_inference_error_response, create_ai_inference_response, process_ai_inference,
    AIInferenceRequest,
};
pub use capability_collector::CapabilityCollector;
pub use command_protocol::{
    commands, Command, CommandResponse, NodeCapabilities, NodeWeights, ReputationData,
    ResponseStatus,
};
pub use command_validation::{validate_command, ValidationError};
pub use kademlia_shard_discovery::{
    dht_keys, ClusterMetadata, KademliaShardDiscovery, PipelineStatus, ShardAnnouncement,
    ShardCapabilities,
};
pub use llama_fragment_processor::{
    process_fragment, FragmentResult, JobResult, LlamaFragment, LlamaJob,
};
pub use llama_model_loader::{create_model_manager, LlamaModelManager, ModelShard, RsyncConfig};
pub use message::{JsonCodec, JsonMessage};
pub use metrics::{MetricsCodec, MetricsRequest, MetricsResponse, PeerMetrics};
pub use pipeline_coordinator::{
    CoordinatorState, CoordinatorStats, DynamicShardLoader, InferenceRequest, InferenceResponse,
    NodeSpawner, PipelineCoordinator, PipelineError, PipelineStrategy, SingleNodeFallback,
};
pub use protocol_logging::{
    log_connection_closed, log_connection_established, log_connection_failed,
    log_transaction_completed, log_transaction_failed, log_transaction_started,
    log_transaction_timeout,
};
pub use quic_transport::{
    create_dual_transport, create_quic_transport, create_tcp_transport, create_transport,
    get_dual_listen_addresses, get_listen_address, TransportError, TransportStats, TransportType,
};
pub use quic_diagnostics::{
    QuicDiagnosticsManager, QuicHandshakeStage, QuicEventType, QuicConnectionEvent, QuicConnectionStats,
};
pub use shard_optimization::{
    select_quantization, OptimizationPriority, QuantizationType, ShardOptimization,
};
pub use shard_loader::{ShardLoader, ShardMetadata, ShardPlan, ShardStatus};

// Re-export node runner functions for unified binary access
pub mod dialer;
pub mod listener;
pub mod monitor;
pub mod server;
pub mod shard_listener;

// Re-export the run functions
pub use dialer::run_dialer;
pub use listener::run_listener;
pub use monitor::run_monitor;
pub use server::run_bootstrap;
pub use shard_listener::run_shard_listener;

// Web server run function is in bin/web_server.rs, we'll need to access it differently
// For now, we'll create a wrapper or the node binary will handle it
