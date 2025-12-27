# Node Spawning for On-Demand Distributed Inference

## Overview

The pipeline coordinator can now **spawn new shard_listener nodes on demand** when shards are missing, enabling true auto-scaling for distributed AI inference.

**Startup Node Spawning**: The web server now automatically spawns nodes for missing shards on startup, ensuring the system is ready for inference immediately.

## Architecture

### Current Capabilities

1. **Dynamic Shard Loading** (existing)
   - Loads missing shards on existing nodes with capacity
   - Uses `LOAD_SHARD` command + torrent downloads

2. **Node Spawning** (new)
   - Spawns new `shard_listener` processes when shards are missing
   - Waits for nodes to come online and join DHT
   - Automatically discovers spawned nodes for inference

3. **Adaptive Strategy** (enhanced)
   - Step 1: Try dynamic shard loading
   - Step 2: Wait for shards to become available
   - Step 3: **Spawn new nodes** (if spawner configured)
   - Step 4: Fallback to single-node full model

## Usage

### Startup Node Spawning (Automatic)

The web server automatically spawns nodes for missing shards on startup:

```bash
cargo run --bin web_server
```

The server will:
1. Check pipeline status after initialization
2. Spawn nodes for any missing shards (typically all 4 shards)
3. Wait for nodes to come online and join the DHT
4. Be ready for inference requests

You'll see output like:
```
[SERVER] Ensuring minimal pipeline is ready...
[COORDINATOR] Pipeline incomplete. Missing shards: [0, 1, 2, 3]
[COORDINATOR] Spawning nodes for missing shards...
[COORDINATOR] ✓ Spawned node for shard 0
[COORDINATOR] ✓ Spawned node for shard 1
[COORDINATOR] ✓ Spawned node for shard 2
[COORDINATOR] ✓ Spawned node for shard 3
[COORDINATOR] ✓ All nodes are online and pipeline is complete!
```

### Basic Setup

```rust
use punch_simple::pipeline_coordinator::{
    PipelineCoordinator, PipelineStrategy, NodeSpawner
};
use punch_simple::kademlia_shard_discovery::KademliaShardDiscovery;

// Create discovery
let discovery = KademliaShardDiscovery::with_expected_shards("llama-cluster", 4);

// Create node spawner
let spawner = NodeSpawner::new(
    "/ip4/127.0.0.1/tcp/51820".to_string(), // bootstrap address
    "llama-cluster".to_string(),            // cluster name
    4,                                       // total_shards
    32,                                      // total_layers
    "llama-8b".to_string(),                  // model_name
    "models_cache/shards".to_string(),       // shards_dir
);

// Create coordinator with spawner
let mut coordinator = PipelineCoordinator::new(discovery)
    .with_node_spawner(spawner);

// Set strategy to spawn nodes
coordinator.set_strategy(PipelineStrategy::SpawnNodes {
    max_nodes_per_request: 4,
    min_memory_per_node_mb: 4096,
    spawn_command_template: "cargo run --bin shard_listener".to_string(),
    node_startup_timeout_secs: 30,
});
```

### Using Adaptive Strategy (Recommended)

The adaptive strategy automatically tries node spawning as step 3:

```rust
coordinator.set_strategy(PipelineStrategy::Adaptive {
    wait_timeout_secs: 30,
    min_memory_for_shard_mb: 4096,
    min_memory_for_full_mb: 16384,
});

// Spawner will be used automatically if configured
```

## How It Works

### 1. Spawn Process

When shards are missing:
- Coordinator calls `spawner.spawn_node_for_shard(shard_id)`
- Spawns `cargo run --bin shard_listener` with appropriate arguments
- Process runs in background
- Process handle stored for later cleanup

### 2. Wait for Node Online

- Coordinator waits for spawned node to:
  1. Start successfully
  2. Connect to DHT bootstrap
  3. Join Kademlia network
  4. Announce shard to DHT
- Polls discovery every 500ms
- Timeout: 30 seconds (configurable)

### 3. Discover and Use

- Once node appears in DHT discovery, it's available for inference
- Coordinator processes request through spawned nodes
- Nodes can download shards via torrent if needed

### 4. Cleanup (Optional)

```rust
// Terminate specific node
spawner.terminate_node(shard_id).await?;

// Terminate all spawned nodes
spawner.terminate_all().await;
```

## Configuration

### NodeSpawner Parameters

- `bootstrap_addr`: DHT bootstrap node address
- `cluster_name`: Cluster identifier for shard discovery
- `total_shards`: Total number of shards in pipeline
- `total_layers`: Total layers in model
- `model_name`: Model identifier
- `shards_dir`: Directory containing GGUF shard files

### SpawnNodes Strategy Parameters

- `max_nodes_per_request`: Maximum nodes to spawn per inference request
- `min_memory_per_node_mb`: Minimum memory required (for future use)
- `spawn_command_template`: Command template (currently uses hardcoded cargo command)
- `node_startup_timeout_secs`: Timeout for node to come online

## Testing

### Manual Test

1. Start bootstrap/DHT server:
   ```bash
   cargo run --bin server
   ```

2. Run test example:
   ```bash
   cargo run --example test_node_spawning
   ```

3. Observe:
   - Nodes being spawned
   - Nodes joining DHT
   - Inference processing through spawned nodes

### Integration Test

The adaptive strategy automatically uses node spawning:
- If dynamic loading fails (no capacity)
- If waiting times out (no nodes appear)
- Then tries spawning nodes
- Finally falls back to single-node mode

## Statistics

The coordinator tracks:
- `nodes_spawned`: Total number of nodes spawned
- `dynamic_loads`: Shards loaded on existing nodes
- `successful_requests`: Requests completed successfully

## Limitations and Future Work

### Current Limitations

1. **Process-based spawning only**: Currently spawns local processes
   - Future: Support Docker containers, Kubernetes pods, cloud instances

2. **No automatic cleanup**: Spawned nodes persist until manually terminated
   - Future: Auto-terminate idle nodes after timeout

3. **No resource limits**: Doesn't check system resources before spawning
   - Future: Check available memory/CPU before spawning

4. **Single machine**: Spawns on local machine only
   - Future: Distributed spawning across multiple machines

### Future Enhancements

1. **Container orchestration**: Spawn Docker containers or Kubernetes pods
2. **Cloud integration**: Spawn EC2 instances, GCP VMs, etc.
3. **Resource monitoring**: Check system resources before spawning
4. **Auto-scaling policies**: Scale up/down based on load
5. **Node lifecycle management**: Auto-terminate idle nodes

## Example: Full Workflow

```rust
// 1. Create coordinator with spawner
let spawner = NodeSpawner::new(...);
let mut coordinator = PipelineCoordinator::new(discovery)
    .with_node_spawner(spawner);

// 2. Set adaptive strategy (includes spawning)
coordinator.set_strategy(PipelineStrategy::Adaptive { ... });

// 3. Submit request
let request = InferenceRequest::new("What is AI?");
let response = coordinator.submit_inference(request).await?;

// Coordinator will:
// - Try dynamic loading (if nodes have capacity)
// - Wait for shards (if nodes are starting)
// - Spawn nodes (if shards still missing)
// - Process inference through spawned nodes
// - Return response

// 4. Cleanup (optional)
spawner.terminate_all().await;
```

## Troubleshooting

### Nodes Not Coming Online

- Check bootstrap server is running
- Verify DHT connectivity
- Check spawned process logs
- Increase `node_startup_timeout_secs`

### Spawn Fails

- Ensure `cargo` is in PATH
- Check `shard_listener` binary exists
- Verify shards directory exists
- Check system resources (memory, file descriptors)

### Nodes Not Discovered

- Wait longer (increase timeout)
- Check DHT bootstrap connectivity
- Verify cluster name matches
- Check network connectivity

## See Also

- `TORRENT_SHARD_LOADING.md` - Shard downloading via torrent
- `LLAMA_DISTRIBUTED_PROCESSING.md` - Distributed inference architecture
- `src/pipeline_coordinator.rs` - Implementation details

