# Shard Loading and Assignment Analysis

## Current Behavior

### 1. Shard Loading on Startup

**What happens:**
- ✅ Nodes scan `shards_dir` for existing `.gguf` files on startup
- ✅ Nodes seed any found files via torrent (make them available for download)
- ✅ Nodes try to load their assigned shard if it exists locally
- ❌ Nodes do NOT automatically download missing shards on startup
- ⏳ Shards are downloaded on-demand when `LOAD_SHARD` command is received

**Code location:** `src/shard_listener.rs:260-340`
```rust
// On startup:
state.scan_gguf_files();  // Scan and seed existing files
state.load_shard_file(shard_id);  // Try to load assigned shard (if exists)
```

### 2. Current Shard Assignment

**What happens:**
- Nodes are spawned with a fixed `--shard-id` argument
- Coordinator uses `find_next_needed_shard()` which picks **first missing shard in sequence** (0, 1, 2, 3)
- No coordination based on what other nodes have picked
- All nodes spawned simultaneously may try to pick the same shard

**Code location:** `src/pipeline_coordinator.rs:778-785`
```rust
// Pipeline incomplete - find the first missing shard in sequence
for shard_id in 0..total_shards {
    if missing_shards.contains(&shard_id) {
        return Some(shard_id);  // First missing shard
    }
}
```

## Problems with Current Approach

1. **Race Condition**: Multiple nodes spawned simultaneously may all pick shard 0
2. **No Coordination**: Nodes don't know what other nodes are picking
3. **Sequential Only**: Always picks lowest missing shard, not balanced
4. **No Pre-download**: Nodes wait for LOAD_SHARD command before downloading

## Proposed Solution: Coordinated Shard Assignment

### Option 1: Round-Robin Based on Last Node
Pick the next shard after the last node that joined:
- Node 1 joins → picks shard 0
- Node 2 joins → picks shard 1 (next after 0)
- Node 3 joins → picks shard 2 (next after 1)
- Node 4 joins → picks shard 3 (next after 2)

**Benefits:**
- ✅ Sequential assignment
- ✅ No conflicts
- ✅ Predictable pattern

### Option 2: Query DHT for Last Assigned Shard
Before spawning, query DHT to find:
- What shards are already assigned
- What was the last shard assigned
- Pick next shard in sequence

**Benefits:**
- ✅ Works even if nodes join at different times
- ✅ Handles node failures gracefully
- ✅ More robust

### Option 3: Pre-download Shards on Startup
When node is assigned a shard:
1. Check if shard exists locally
2. If not, immediately start torrent download
3. Join DHT once shard is loaded (or join immediately and download in background)

**Benefits:**
- ✅ Faster pipeline completion
- ✅ Nodes ready for inference sooner
- ✅ Better user experience

## Recommended Implementation

Combine all three approaches:
1. **Coordinated Assignment**: Query DHT to find last assigned shard, pick next
2. **Pre-download**: Start downloading assigned shard immediately
3. **Background Loading**: Join DHT immediately, download in background


