//! Integration tests for Kademlia Shard Discovery
//!
//! Tests the ShardAnnouncement, KademliaShardDiscovery, and pipeline building logic.

use punch_simple::{
    KademliaShardDiscovery, ShardAnnouncement, ShardCapabilities,
    ClusterMetadata, PipelineStatus, dht_keys,
};

// ============================================================================
// ShardAnnouncement Tests
// ============================================================================

#[test]
fn test_shard_announcement_creation() {
    let announcement = ShardAnnouncement::new(
        "12D3KooWTestPeer",
        0,      // shard_id
        4,      // total_shards
        32,     // total_layers
        "/ip4/192.168.1.100/tcp/51820",
        "llama-8b",
    );

    assert_eq!(announcement.peer_id, "12D3KooWTestPeer");
    assert_eq!(announcement.shard_id, 0);
    assert_eq!(announcement.layer_start, 0);
    assert_eq!(announcement.layer_end, 8);
    assert_eq!(announcement.num_layers, 8);
    assert!(announcement.has_embeddings);
    assert!(!announcement.has_output);
    assert_eq!(announcement.total_shards, 4);
    assert_eq!(announcement.model_name, "llama-8b");
}

#[test]
fn test_shard_announcement_layer_calculation() {
    // Test all 4 shards of a 32-layer model
    let shards: Vec<_> = (0..4)
        .map(|i| ShardAnnouncement::new(
            &format!("peer-{}", i),
            i,
            4,
            32,
            &format!("/ip4/10.0.0.{}/tcp/51820", i),
            "llama",
        ))
        .collect();

    // Verify layer ranges
    assert_eq!(shards[0].layer_start, 0);
    assert_eq!(shards[0].layer_end, 8);
    
    assert_eq!(shards[1].layer_start, 8);
    assert_eq!(shards[1].layer_end, 16);
    
    assert_eq!(shards[2].layer_start, 16);
    assert_eq!(shards[2].layer_end, 24);
    
    assert_eq!(shards[3].layer_start, 24);
    assert_eq!(shards[3].layer_end, 32);

    // Verify embeddings and output flags
    assert!(shards[0].has_embeddings);
    assert!(!shards[0].has_output);
    
    assert!(!shards[1].has_embeddings);
    assert!(!shards[1].has_output);
    
    assert!(!shards[2].has_embeddings);
    assert!(!shards[2].has_output);
    
    assert!(!shards[3].has_embeddings);
    assert!(shards[3].has_output);
}

#[test]
fn test_shard_announcement_serialization() {
    let announcement = ShardAnnouncement::new(
        "12D3KooWTest",
        1,
        4,
        32,
        "/ip4/192.168.1.101/tcp/51820",
        "llama-8b",
    );

    // Serialize
    let bytes = announcement.to_bytes().expect("Serialization should succeed");
    assert!(!bytes.is_empty());

    // Deserialize
    let deserialized = ShardAnnouncement::from_bytes(&bytes)
        .expect("Deserialization should succeed");

    assert_eq!(deserialized.shard_id, announcement.shard_id);
    assert_eq!(deserialized.peer_id, announcement.peer_id);
    assert_eq!(deserialized.layer_start, announcement.layer_start);
    assert_eq!(deserialized.layer_end, announcement.layer_end);
    assert_eq!(deserialized.model_name, announcement.model_name);
}

#[test]
fn test_shard_announcement_freshness() {
    let announcement = ShardAnnouncement::new(
        "peer-1",
        0,
        4,
        32,
        "/ip4/10.0.0.1/tcp/51820",
        "llama",
    );

    // Should be fresh immediately (within 5 minute TTL)
    assert!(announcement.is_fresh(300));
    
    // Should be fresh with 1 hour TTL
    assert!(announcement.is_fresh(3600));
}

// ============================================================================
// ShardCapabilities Tests
// ============================================================================

#[test]
fn test_shard_capabilities_detect() {
    let caps = ShardCapabilities::detect();
    
    // Should have at least 1 CPU core
    assert!(caps.cpu_cores >= 1);
    
    // Default values
    assert_eq!(caps.reputation, 1.0);
    assert_eq!(caps.active_requests, 0);
    assert!(caps.max_concurrent > 0);
}

#[test]
fn test_shard_capabilities_score_calculation() {
    use punch_simple::NodeWeights;
    
    let caps = ShardCapabilities {
        cpu_cores: 16,
        cpu_usage: 25.0,
        memory_total_mb: 32768,
        memory_available_mb: 24576,
        gpu_memory_mb: 0,
        latency_ms: 10.0,
        reputation: 0.95,
        shard_loaded: true,
        active_requests: 1,
        max_concurrent: 4,
    };

    let weights = NodeWeights::default();
    let score = caps.calculate_score(&weights);

    // Score should be positive and reasonable
    assert!(score > 0.0);
    assert!(score <= 2.0); // With all bonuses, shouldn't exceed ~1.5
}

#[test]
fn test_capabilities_score_comparison() {
    use punch_simple::NodeWeights;
    
    let weights = NodeWeights::default();

    // High-capacity node
    let high_caps = ShardCapabilities {
        cpu_cores: 32,
        cpu_usage: 10.0,
        memory_total_mb: 65536,
        memory_available_mb: 50000,
        gpu_memory_mb: 24000,
        latency_ms: 5.0,
        reputation: 1.0,
        shard_loaded: true,
        active_requests: 0,
        max_concurrent: 8,
    };

    // Low-capacity node
    let low_caps = ShardCapabilities {
        cpu_cores: 4,
        cpu_usage: 90.0,
        memory_total_mb: 8192,
        memory_available_mb: 1000,
        gpu_memory_mb: 0,
        latency_ms: 100.0,
        reputation: 0.5,
        shard_loaded: false,
        active_requests: 4,
        max_concurrent: 4,
    };

    let high_score = high_caps.calculate_score(&weights);
    let low_score = low_caps.calculate_score(&weights);

    // High-capacity node should score better
    assert!(high_score > low_score, 
        "High-capacity score {} should be greater than low-capacity score {}", 
        high_score, low_score);
}

// ============================================================================
// KademliaShardDiscovery Tests
// ============================================================================

#[test]
fn test_discovery_creation() {
    let discovery = KademliaShardDiscovery::new("test-cluster");
    
    assert_eq!(discovery.cluster_name(), "test-cluster");
    assert_eq!(discovery.shard_count(), 0);
    assert!(!discovery.is_pipeline_complete());
}

#[test]
fn test_discovery_with_expected_shards() {
    let discovery = KademliaShardDiscovery::with_expected_shards("test-cluster", 4);
    
    let status = discovery.status();
    assert_eq!(status.expected_shards, 4);
    assert_eq!(status.discovered_shards, 0);
    assert!(!status.is_complete);
}

#[test]
fn test_discovery_add_shard() {
    let mut discovery = KademliaShardDiscovery::new("test-cluster");

    let announcement = ShardAnnouncement::new(
        "peer-0",
        0,
        4,
        32,
        "/ip4/10.0.0.1/tcp/51820",
        "llama",
    );

    discovery.add_shard(announcement);

    assert_eq!(discovery.shard_count(), 1);
    assert!(discovery.entry_node().is_some());
    assert!(discovery.exit_node().is_none()); // Exit node not added yet
}

#[test]
fn test_discovery_pipeline_building() {
    let mut discovery = KademliaShardDiscovery::with_expected_shards("test-cluster", 4);

    // Add shards out of order
    discovery.add_shard(ShardAnnouncement::new(
        "peer2", 2, 4, 32, "/ip4/10.0.0.2/tcp/51820", "llama",
    ));
    discovery.add_shard(ShardAnnouncement::new(
        "peer0", 0, 4, 32, "/ip4/10.0.0.0/tcp/51820", "llama",
    ));
    discovery.add_shard(ShardAnnouncement::new(
        "peer3", 3, 4, 32, "/ip4/10.0.0.3/tcp/51820", "llama",
    ));
    discovery.add_shard(ShardAnnouncement::new(
        "peer1", 1, 4, 32, "/ip4/10.0.0.1/tcp/51820", "llama",
    ));

    // Pipeline should be sorted
    let pipeline = discovery.get_pipeline();
    assert_eq!(pipeline.len(), 4);
    
    for (i, shard) in pipeline.iter().enumerate() {
        assert_eq!(shard.shard_id, i as u32, 
            "Pipeline index {} should have shard_id {}", i, i);
    }
}

#[test]
fn test_discovery_entry_exit_nodes() {
    let mut discovery = KademliaShardDiscovery::with_expected_shards("test-cluster", 4);

    // Add all shards
    for i in 0..4 {
        discovery.add_shard(ShardAnnouncement::new(
            &format!("peer{}", i),
            i,
            4,
            32,
            &format!("/ip4/10.0.0.{}/tcp/51820", i),
            "llama",
        ));
    }

    // Check entry node (shard 0 with embeddings)
    let entry = discovery.entry_node().expect("Should have entry node");
    assert_eq!(entry.shard_id, 0);
    assert!(entry.has_embeddings);

    // Check exit node (shard 3 with output)
    let exit = discovery.exit_node().expect("Should have exit node");
    assert_eq!(exit.shard_id, 3);
    assert!(exit.has_output);
}

#[test]
fn test_discovery_next_shard() {
    let mut discovery = KademliaShardDiscovery::new("test-cluster");

    for i in 0..4 {
        discovery.add_shard(ShardAnnouncement::new(
            &format!("peer{}", i),
            i,
            4,
            32,
            &format!("/ip4/10.0.0.{}/tcp/51820", i),
            "llama",
        ));
    }

    // Test next_shard navigation
    assert_eq!(discovery.next_shard(0).unwrap().shard_id, 1);
    assert_eq!(discovery.next_shard(1).unwrap().shard_id, 2);
    assert_eq!(discovery.next_shard(2).unwrap().shard_id, 3);
    assert!(discovery.next_shard(3).is_none()); // No shard after last
}

#[test]
fn test_discovery_previous_shard() {
    let mut discovery = KademliaShardDiscovery::new("test-cluster");

    for i in 0..4 {
        discovery.add_shard(ShardAnnouncement::new(
            &format!("peer{}", i),
            i,
            4,
            32,
            &format!("/ip4/10.0.0.{}/tcp/51820", i),
            "llama",
        ));
    }

    // Test previous_shard navigation
    assert!(discovery.previous_shard(0).is_none()); // No shard before first
    assert_eq!(discovery.previous_shard(1).unwrap().shard_id, 0);
    assert_eq!(discovery.previous_shard(2).unwrap().shard_id, 1);
    assert_eq!(discovery.previous_shard(3).unwrap().shard_id, 2);
}

#[test]
fn test_discovery_incomplete_pipeline() {
    let mut discovery = KademliaShardDiscovery::with_expected_shards("test-cluster", 4);

    // Add only 2 of 4 shards
    discovery.add_shard(ShardAnnouncement::new(
        "peer0", 0, 4, 32, "/ip4/10.0.0.0/tcp/51820", "llama",
    ));
    discovery.add_shard(ShardAnnouncement::new(
        "peer2", 2, 4, 32, "/ip4/10.0.0.2/tcp/51820", "llama",
    ));

    assert!(!discovery.is_pipeline_complete());
    
    let missing = discovery.get_missing_shards();
    assert_eq!(missing.len(), 2);
    assert!(missing.contains(&1));
    assert!(missing.contains(&3));
}

#[test]
fn test_discovery_multiple_replicas() {
    let mut discovery = KademliaShardDiscovery::new("test-cluster");

    // Add two replicas for shard 0
    let mut replica1 = ShardAnnouncement::new(
        "peer0a", 0, 4, 32, "/ip4/10.0.0.1/tcp/51820", "llama",
    );
    replica1.capabilities.cpu_cores = 8;
    replica1.capabilities.reputation = 0.9;

    let mut replica2 = ShardAnnouncement::new(
        "peer0b", 0, 4, 32, "/ip4/10.0.0.2/tcp/51820", "llama",
    );
    replica2.capabilities.cpu_cores = 16;
    replica2.capabilities.reputation = 0.95;

    discovery.add_shard(replica1);
    discovery.add_shard(replica2);

    // Should have 1 shard with 2 replicas
    assert_eq!(discovery.shard_count(), 1);
    assert_eq!(discovery.replica_count(), 2);

    // Best node should be replica2 (better capabilities)
    let best = discovery.get_best_node_for_shard(0).unwrap();
    assert_eq!(best.peer_id, "peer0b");
}

#[test]
fn test_discovery_get_shard_replicas() {
    let mut discovery = KademliaShardDiscovery::new("test-cluster");

    // Add multiple replicas for shard 1
    for i in 0..3 {
        discovery.add_shard(ShardAnnouncement::new(
            &format!("replica-{}", i),
            1,
            4,
            32,
            &format!("/ip4/10.0.0.{}/tcp/51820", i),
            "llama",
        ));
    }

    let replicas = discovery.get_shard_replicas(1).expect("Should have replicas");
    assert_eq!(replicas.len(), 3);
}

#[test]
fn test_discovery_status() {
    let mut discovery = KademliaShardDiscovery::with_expected_shards("test-cluster", 4);

    // Add 2 shards
    discovery.add_shard(ShardAnnouncement::new(
        "peer0", 0, 4, 32, "/ip4/10.0.0.0/tcp/51820", "llama",
    ));
    discovery.add_shard(ShardAnnouncement::new(
        "peer3", 3, 4, 32, "/ip4/10.0.0.3/tcp/51820", "llama",
    ));

    let status = discovery.status();
    
    assert_eq!(status.cluster_name, "test-cluster");
    assert_eq!(status.discovered_shards, 2);
    assert_eq!(status.expected_shards, 4);
    assert_eq!(status.total_replicas, 2);
    assert!(!status.is_complete);
    assert!(status.has_entry); // shard 0 added
    assert!(status.has_exit);  // shard 3 added
    assert_eq!(status.missing_shards, vec![1, 2]);
}

#[test]
fn test_discovery_status_display() {
    let mut discovery = KademliaShardDiscovery::with_expected_shards("test-cluster", 4);
    
    for i in 0..4 {
        discovery.add_shard(ShardAnnouncement::new(
            &format!("peer{}", i),
            i,
            4,
            32,
            &format!("/ip4/10.0.0.{}/tcp/51820", i),
            "llama",
        ));
    }

    let status = discovery.status();
    let display = format!("{}", status);
    
    assert!(display.contains("test-cluster"));
    assert!(display.contains("4/4"));
    assert!(display.contains("complete: true"));
}

// ============================================================================
// DHT Keys Tests
// ============================================================================

#[test]
fn test_dht_keys_cluster_key() {
    let key = dht_keys::cluster_key("llama-8b");
    assert_eq!(key, "/llama-cluster/llama-8b");
}

#[test]
fn test_dht_keys_shard_key() {
    assert_eq!(dht_keys::shard_key("llama-8b", 0), "/llama-cluster/llama-8b/shard/0");
    assert_eq!(dht_keys::shard_key("llama-8b", 3), "/llama-cluster/llama-8b/shard/3");
}

#[test]
fn test_dht_keys_all_shards_key() {
    let key = dht_keys::all_shards_key("my-cluster");
    assert_eq!(key, "/llama-cluster/my-cluster/shards");
}

#[test]
fn test_dht_keys_metadata_key() {
    let key = dht_keys::metadata_key("test-cluster");
    assert_eq!(key, "/llama-cluster/test-cluster/metadata");
}

#[test]
fn test_dht_keys_parse_shard_id() {
    assert_eq!(dht_keys::parse_shard_id("/llama-cluster/test/shard/0"), Some(0));
    assert_eq!(dht_keys::parse_shard_id("/llama-cluster/test/shard/3"), Some(3));
    assert_eq!(dht_keys::parse_shard_id("/llama-cluster/test/shard/42"), Some(42));
    assert_eq!(dht_keys::parse_shard_id("/invalid/path"), None);
}

// ============================================================================
// Pipeline Flow Tests
// ============================================================================

#[test]
fn test_full_pipeline_flow() {
    let mut discovery = KademliaShardDiscovery::with_expected_shards("llama-8b-cluster", 4);

    // Simulate discovering shards one by one
    assert!(!discovery.is_pipeline_complete());

    // Shard 0 (entry)
    discovery.add_shard(ShardAnnouncement::new(
        "node-entry", 0, 4, 32, "/ip4/10.0.0.1/tcp/51820", "llama-8b",
    ));
    assert!(discovery.entry_node().is_some());
    assert!(discovery.exit_node().is_none());

    // Shard 1
    discovery.add_shard(ShardAnnouncement::new(
        "node-middle-1", 1, 4, 32, "/ip4/10.0.0.2/tcp/51820", "llama-8b",
    ));

    // Shard 2
    discovery.add_shard(ShardAnnouncement::new(
        "node-middle-2", 2, 4, 32, "/ip4/10.0.0.3/tcp/51820", "llama-8b",
    ));

    // Still incomplete
    assert!(!discovery.is_pipeline_complete());

    // Shard 3 (exit)
    discovery.add_shard(ShardAnnouncement::new(
        "node-exit", 3, 4, 32, "/ip4/10.0.0.4/tcp/51820", "llama-8b",
    ));

    // Now complete!
    assert!(discovery.is_pipeline_complete());

    // Verify full pipeline
    let pipeline = discovery.get_pipeline();
    assert_eq!(pipeline.len(), 4);

    // Verify navigation
    let entry = discovery.entry_node().unwrap();
    assert_eq!(entry.shard_id, 0);
    assert!(entry.has_embeddings);

    let next = discovery.next_shard(entry.shard_id).unwrap();
    assert_eq!(next.shard_id, 1);

    let next = discovery.next_shard(next.shard_id).unwrap();
    assert_eq!(next.shard_id, 2);

    let exit = discovery.next_shard(next.shard_id).unwrap();
    assert_eq!(exit.shard_id, 3);
    assert!(exit.has_output);

    // No more shards after exit
    assert!(discovery.next_shard(exit.shard_id).is_none());
}

#[test]
fn test_large_model_sharding() {
    // Test with 8 shards for a 64-layer model
    let mut discovery = KademliaShardDiscovery::with_expected_shards("llama-65b", 8);

    for i in 0..8 {
        discovery.add_shard(ShardAnnouncement::new(
            &format!("node-{}", i),
            i,
            8,      // total_shards
            64,     // total_layers
            &format!("/ip4/10.0.0.{}/tcp/51820", i),
            "llama-65b",
        ));
    }

    assert!(discovery.is_pipeline_complete());

    let pipeline = discovery.get_pipeline();
    assert_eq!(pipeline.len(), 8);

    // Each shard should have 8 layers
    for shard in &pipeline {
        assert_eq!(shard.num_layers, 8);
    }

    // First shard: layers 0-8
    assert_eq!(pipeline[0].layer_start, 0);
    assert_eq!(pipeline[0].layer_end, 8);

    // Last shard: layers 56-64
    assert_eq!(pipeline[7].layer_start, 56);
    assert_eq!(pipeline[7].layer_end, 64);
}

