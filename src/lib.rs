//! Punch Simple - P2P Network Library
//! 
//! This library provides P2P networking capabilities with:
//! - Kademlia DHT for peer discovery
//! - JSON command protocol for inter-node communication
//! - Weighted node selection based on capabilities
//! - Reputation tracking system

pub mod message;
pub mod command_protocol;
pub mod capability_collector;

pub use command_protocol::{Command, CommandResponse, NodeCapabilities, NodeWeights, ReputationData, commands};
pub use capability_collector::CapabilityCollector;

