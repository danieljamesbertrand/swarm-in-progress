//! Integration tests for PipelineCoordinator
//!
//! Tests graceful degradation strategies for handling incomplete pipelines.

use punch_simple::{
    KademliaShardDiscovery, ShardAnnouncement, ShardCapabilities,
    PipelineCoordinator, PipelineStrategy, PipelineError,
    InferenceRequest, InferenceResponse, CoordinatorState,
};
use std::time::Duration;

// ============================================================================
// Helper Functions
// ============================================================================

fn create_test_shard(shard_id: u32, total_shards: u32, memory_mb: u64) -> ShardAnnouncement {
    let mut ann = ShardAnnouncement::new(
        &format!("peer-{}", shard_id),
        shard_id,
        total_shards,
        32,
        &format!("/ip4/10.0.0.{}/tcp/51820", shard_id),
        "llama-test",
    );
    ann.capabilities.memory_available_mb = memory_mb;
    ann.capabilities.memory_total_mb = memory_mb + 4096;
    ann
}

fn create_high_memory_shard(shard_id: u32, total_shards: u32, memory_mb: u64) -> ShardAnnouncement {
    let mut ann = create_test_shard(shard_id, total_shards, memory_mb);
    ann.peer_id = format!("high-mem-peer-{}", shard_id);
    ann
}

// ============================================================================
// Coordinator Creation Tests
// ============================================================================

#[tokio::test]
async fn test_coordinator_creation() {
    let discovery = KademliaShardDiscovery::with_expected_shards("test", 4);
    let coordinator = PipelineCoordinator::new(discovery);
    
    let state = coordinator.state().await;
    assert!(matches!(state, CoordinatorState::Unavailable { .. }));
}

#[tokio::test]
async fn test_coordinator_with_complete_pipeline() {
    let discovery = KademliaShardDiscovery::with_expected_shards("test", 4);
    let coordinator = PipelineCoordinator::new(discovery);

    // Add all shards
    for i in 0..4 {
        coordinator.add_shard(create_test_shard(i, 4, 8192)).await;
    }

    let state = coordinator.state().await;
    assert!(matches!(state, CoordinatorState::Ready));
}

#[tokio::test]
async fn test_coordinator_with_incomplete_pipeline() {
    let discovery = KademliaShardDiscovery::with_expected_shards("test", 4);
    let coordinator = PipelineCoordinator::new(discovery);

    // Add only 2 shards
    coordinator.add_shard(create_test_shard(0, 4, 8192)).await;
    coordinator.add_shard(create_test_shard(2, 4, 8192)).await;

    let state = coordinator.state().await;
    match state {
        CoordinatorState::WaitingForShards { missing } => {
            assert!(missing.contains(&1));
            assert!(missing.contains(&3));
        }
        _ => panic!("Expected WaitingForShards state"),
    }
}

// ============================================================================
// Strategy Tests
// ============================================================================

#[tokio::test]
async fn test_fail_fast_strategy_complete_pipeline() {
    let discovery = KademliaShardDiscovery::with_expected_shards("test", 4);
    let mut coordinator = PipelineCoordinator::new(discovery);
    coordinator.set_strategy(PipelineStrategy::FailFast);

    // Add all shards
    for i in 0..4 {
        coordinator.add_shard(create_test_shard(i, 4, 8192)).await;
    }

    let request = InferenceRequest::new("test prompt");
    let result = coordinator.submit_inference(request).await;

    assert!(result.is_ok());
    let response = result.unwrap();
    assert!(response.success);
    assert_eq!(response.shard_latencies.len(), 4);
}

#[tokio::test]
async fn test_fail_fast_strategy_incomplete_pipeline() {
    let discovery = KademliaShardDiscovery::with_expected_shards("test", 4);
    let mut coordinator = PipelineCoordinator::new(discovery);
    coordinator.set_strategy(PipelineStrategy::FailFast);

    // Add incomplete pipeline
    coordinator.add_shard(create_test_shard(0, 4, 8192)).await;

    let request = InferenceRequest::new("test prompt");
    let result = coordinator.submit_inference(request).await;

    assert!(result.is_err());
    match result.unwrap_err() {
        PipelineError::NoFallback { .. } => {}
        e => panic!("Expected NoFallback error, got: {:?}", e),
    }
}

#[tokio::test]
async fn test_single_node_fallback_with_capable_node() {
    let discovery = KademliaShardDiscovery::with_expected_shards("test", 4);
    let mut coordinator = PipelineCoordinator::new(discovery);
    coordinator.set_strategy(PipelineStrategy::SingleNodeFallback {
        required_memory_mb: 16000,
    });

    // Add one high-memory shard
    coordinator.add_shard(create_high_memory_shard(0, 4, 32000)).await;

    let request = InferenceRequest::new("test prompt");
    let result = coordinator.submit_inference(request).await;

    assert!(result.is_ok());
    let response = result.unwrap();
    assert!(response.success);
    assert_eq!(response.strategy_used, "single_node_fallback");
}

#[tokio::test]
async fn test_single_node_fallback_no_capable_node() {
    let discovery = KademliaShardDiscovery::with_expected_shards("test", 4);
    let mut coordinator = PipelineCoordinator::new(discovery);
    coordinator.set_strategy(PipelineStrategy::SingleNodeFallback {
        required_memory_mb: 64000,  // Very high requirement
    });

    // Add low-memory shards
    coordinator.add_shard(create_test_shard(0, 4, 8192)).await;

    let request = InferenceRequest::new("test prompt");
    let result = coordinator.submit_inference(request).await;

    assert!(result.is_err());
    match result.unwrap_err() {
        PipelineError::NoFallback { reason } => {
            assert!(reason.contains("memory"));
        }
        e => panic!("Expected NoFallback error, got: {:?}", e),
    }
}

#[tokio::test]
async fn test_wait_and_retry_timeout() {
    let discovery = KademliaShardDiscovery::with_expected_shards("test", 4);
    let mut coordinator = PipelineCoordinator::new(discovery);
    coordinator.set_strategy(PipelineStrategy::WaitAndRetry {
        timeout_secs: 1,
        retry_interval_ms: 200,
    });

    // Add incomplete pipeline
    coordinator.add_shard(create_test_shard(0, 4, 8192)).await;

    let request = InferenceRequest::new("test prompt");
    let result = coordinator.submit_inference(request).await;

    assert!(result.is_err());
    match result.unwrap_err() {
        PipelineError::Timeout { missing_shards, .. } => {
            assert!(!missing_shards.is_empty());
        }
        e => panic!("Expected Timeout error, got: {:?}", e),
    }
}

#[tokio::test]
async fn test_wait_and_retry_success() {
    let discovery = KademliaShardDiscovery::with_expected_shards("test", 4);
    let mut coordinator = PipelineCoordinator::new(discovery);
    coordinator.set_strategy(PipelineStrategy::WaitAndRetry {
        timeout_secs: 5,
        retry_interval_ms: 100,
    });

    // Add partial pipeline
    coordinator.add_shard(create_test_shard(0, 4, 8192)).await;
    coordinator.add_shard(create_test_shard(1, 4, 8192)).await;

    let coordinator = std::sync::Arc::new(coordinator);
    let coordinator_clone = coordinator.clone();

    // Add remaining shards in background
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(300)).await;
        coordinator_clone.add_shard(create_test_shard(2, 4, 8192)).await;
        tokio::time::sleep(Duration::from_millis(100)).await;
        coordinator_clone.add_shard(create_test_shard(3, 4, 8192)).await;
    });

    let request = InferenceRequest::new("test prompt");
    let result = coordinator.submit_inference(request).await;

    assert!(result.is_ok());
    let response = result.unwrap();
    assert!(response.success);
    assert_eq!(response.strategy_used, "pipeline");
}

// ============================================================================
// Inference Request Tests
// ============================================================================

#[test]
fn test_inference_request_builder() {
    let request = InferenceRequest::new("Hello world")
        .with_max_tokens(500)
        .with_temperature(0.8)
        .with_priority(1);

    assert_eq!(request.prompt, "Hello world");
    assert_eq!(request.max_tokens, 500);
    assert_eq!(request.temperature, 0.8);
    assert_eq!(request.priority, 1);
    assert!(!request.request_id.is_empty());
}

#[test]
fn test_inference_request_defaults() {
    let request = InferenceRequest::new("test");

    assert_eq!(request.max_tokens, 256);
    assert_eq!(request.temperature, 0.7);
    assert_eq!(request.top_p, 0.9);
    assert_eq!(request.priority, 0);
    assert!(request.context.is_none());
}

// ============================================================================
// Statistics Tests
// ============================================================================

#[tokio::test]
async fn test_stats_tracking() {
    let discovery = KademliaShardDiscovery::with_expected_shards("test", 4);
    let coordinator = PipelineCoordinator::new(discovery);

    // Add all shards
    for i in 0..4 {
        coordinator.add_shard(create_test_shard(i, 4, 8192)).await;
    }

    // Process requests
    for _ in 0..3 {
        let request = InferenceRequest::new("test");
        let _ = coordinator.submit_inference(request).await;
    }

    let stats = coordinator.stats().await;
    assert_eq!(stats.total_requests, 3);
    assert_eq!(stats.successful_requests, 3);
    assert_eq!(stats.failed_requests, 0);
    assert!(stats.average_latency_ms > 0.0);
}

#[tokio::test]
async fn test_stats_failure_tracking() {
    let discovery = KademliaShardDiscovery::with_expected_shards("test", 4);
    let mut coordinator = PipelineCoordinator::new(discovery);
    coordinator.set_strategy(PipelineStrategy::FailFast);

    // Incomplete pipeline
    coordinator.add_shard(create_test_shard(0, 4, 8192)).await;

    // Process failing requests
    for _ in 0..2 {
        let request = InferenceRequest::new("test");
        let _ = coordinator.submit_inference(request).await;
    }

    let stats = coordinator.stats().await;
    assert_eq!(stats.total_requests, 2);
    assert_eq!(stats.successful_requests, 0);
    assert_eq!(stats.failed_requests, 2);
}

#[tokio::test]
async fn test_fallback_stats() {
    let discovery = KademliaShardDiscovery::with_expected_shards("test", 4);
    let mut coordinator = PipelineCoordinator::new(discovery);
    coordinator.set_strategy(PipelineStrategy::SingleNodeFallback {
        required_memory_mb: 16000,
    });

    coordinator.add_shard(create_high_memory_shard(0, 4, 32000)).await;

    let request = InferenceRequest::new("test");
    let _ = coordinator.submit_inference(request).await;

    let stats = coordinator.stats().await;
    assert_eq!(stats.fallback_requests, 1);
}

// ============================================================================
// Pipeline Status Tests
// ============================================================================

#[tokio::test]
async fn test_pipeline_status_complete() {
    let discovery = KademliaShardDiscovery::with_expected_shards("test", 4);
    let coordinator = PipelineCoordinator::new(discovery);

    for i in 0..4 {
        coordinator.add_shard(create_test_shard(i, 4, 8192)).await;
    }

    let status = coordinator.pipeline_status().await;
    assert!(status.is_complete);
    assert!(status.has_entry);
    assert!(status.has_exit);
    assert_eq!(status.discovered_shards, 4);
    assert_eq!(status.expected_shards, 4);
    assert!(status.missing_shards.is_empty());
}

#[tokio::test]
async fn test_pipeline_status_incomplete() {
    let discovery = KademliaShardDiscovery::with_expected_shards("test", 4);
    let coordinator = PipelineCoordinator::new(discovery);

    coordinator.add_shard(create_test_shard(0, 4, 8192)).await;
    coordinator.add_shard(create_test_shard(3, 4, 8192)).await;

    let status = coordinator.pipeline_status().await;
    assert!(!status.is_complete);
    assert!(status.has_entry);
    assert!(status.has_exit);
    assert_eq!(status.discovered_shards, 2);
    assert_eq!(status.missing_shards, vec![1, 2]);
}

// ============================================================================
// Adaptive Strategy Tests
// ============================================================================

#[tokio::test]
async fn test_adaptive_strategy_uses_fallback() {
    let discovery = KademliaShardDiscovery::with_expected_shards("test", 4);
    let mut coordinator = PipelineCoordinator::new(discovery);
    coordinator.set_strategy(PipelineStrategy::Adaptive {
        wait_timeout_secs: 1,
        min_memory_for_shard_mb: 4096,
        min_memory_for_full_mb: 16000,
    });

    // Add high-memory node
    coordinator.add_shard(create_high_memory_shard(0, 4, 32000)).await;

    let request = InferenceRequest::new("test");
    let result = coordinator.submit_inference(request).await;

    assert!(result.is_ok());
    let response = result.unwrap();
    assert_eq!(response.strategy_used, "single_node_fallback");
}

// ============================================================================
// Edge Cases
// ============================================================================

#[tokio::test]
async fn test_empty_pipeline() {
    let discovery = KademliaShardDiscovery::with_expected_shards("test", 4);
    let mut coordinator = PipelineCoordinator::new(discovery);
    coordinator.set_strategy(PipelineStrategy::FailFast);

    let request = InferenceRequest::new("test");
    let result = coordinator.submit_inference(request).await;

    assert!(result.is_err());
}

#[tokio::test]
async fn test_pipeline_with_only_middle_shards() {
    let discovery = KademliaShardDiscovery::with_expected_shards("test", 4);
    let coordinator = PipelineCoordinator::new(discovery);

    // Add only middle shards (no entry or exit)
    coordinator.add_shard(create_test_shard(1, 4, 8192)).await;
    coordinator.add_shard(create_test_shard(2, 4, 8192)).await;

    let state = coordinator.state().await;
    match state {
        CoordinatorState::WaitingForShards { missing } => {
            assert!(missing.contains(&0));
            assert!(missing.contains(&3));
        }
        _ => panic!("Expected WaitingForShards"),
    }
}

#[tokio::test]
async fn test_multiple_requests_complete_pipeline() {
    let discovery = KademliaShardDiscovery::with_expected_shards("test", 4);
    let coordinator = PipelineCoordinator::new(discovery);

    for i in 0..4 {
        coordinator.add_shard(create_test_shard(i, 4, 8192)).await;
    }

    // Process multiple requests concurrently
    let mut handles = vec![];
    let coordinator = std::sync::Arc::new(coordinator);

    for i in 0..5 {
        let coord = coordinator.clone();
        handles.push(tokio::spawn(async move {
            let request = InferenceRequest::new(&format!("Request {}", i));
            coord.submit_inference(request).await
        }));
    }

    for handle in handles {
        let result = handle.await.unwrap();
        assert!(result.is_ok());
    }

    let stats = coordinator.stats().await;
    assert_eq!(stats.total_requests, 5);
    assert_eq!(stats.successful_requests, 5);
}







