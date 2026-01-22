# Node State Reporting and Control

## Overview

Nodes now support comprehensive state reporting and control commands that allow external tools to probe their internal state and understand what's happening.

---

## New Command: `GET_NODE_STATUS`

### Purpose

Returns comprehensive internal state of a node, including:
- Local shard loading status
- Swarm readiness status
- Discovery state (which shards are found)
- Per-shard loading status
- Node capabilities
- Request statistics

### Request

```json
{
  "command": "GET_NODE_STATUS",
  "request_id": "req-123",
  "from": "12D3KooW...",
  "to": "12D3KooW...",
  "timestamp": 1234567890,
  "params": {}
}
```

### Response

```json
{
  "command": "GET_NODE_STATUS",
  "request_id": "req-123",
  "from": "12D3KooW...",
  "to": "12D3KooW...",
  "timestamp": 1234567890,
  "status": "success",
  "result": {
    "peer_id": "12D3KooW...",
    "shard_id": 0,
    "local_shard_loaded": true,
    "local_shard_path": "models_cache/shards/shard-0.gguf",
    "swarm_ready": true,
    "discovered_shards": 8,
    "expected_shards": 8,
    "is_complete": true,
    "all_shards_loaded": true,
    "missing_shards": [],
    "shard_statuses": {
      "0": {
        "discovered": true,
        "shard_loaded": true,
        "peer_id": "12D3KooW...",
        "is_local": true
      },
      "1": {
        "discovered": true,
        "shard_loaded": true,
        "peer_id": "12D3KooW...",
        "is_local": false
      },
      ...
    },
    "active_requests": 0,
    "total_requests": 100,
    "successful_requests": 95,
    "capabilities": {
      "cpu_cores": 8,
      "memory_total_mb": 16384,
      "memory_available_mb": 8192,
      "gpu_available": false,
      "gpu_memory_mb": 0,
      "shard_loaded": true
    }
  }
}
```

---

## What This Enables

### 1. Direct Node State Querying

**Before:**
- Had to query DHT and parse announcements
- Couldn't see local node's internal state
- No way to know if node thinks swarm is ready

**After:**
- Send `GET_NODE_STATUS` command directly to any node
- Get complete internal state
- See exactly what the node knows about swarm readiness

---

### 2. Comprehensive Monitoring

**Monitor can now:**
- Query each node directly for status
- See per-shard loading status from node's perspective
- Identify which nodes think swarm is ready
- Find discrepancies between nodes

---

### 3. Debugging Swarm Readiness

**Can now see:**
- Which shards each node has discovered
- Which shards each node thinks are loaded
- Why a node thinks swarm is/isn't ready
- Local shard file status

---

## Using GET_NODE_STATUS

### From Monitoring Tools

**Rust Monitor (`node_monitor.rs`):**
```rust
// Send GET_NODE_STATUS to a node
let cmd = Command::new(commands::GET_NODE_STATUS, &monitor_peer_id, Some(&target_peer_id));
let response = send_command(&swarm, &target_peer_id, cmd).await?;

// Parse response
let status = response.result.unwrap();
println!("Node {} status:", target_peer_id);
println!("  Local shard loaded: {}", status["local_shard_loaded"]);
println!("  Swarm ready: {}", status["swarm_ready"]);
println!("  Discovered shards: {}/{}", 
    status["discovered_shards"], 
    status["expected_shards"]
);
```

---

### From PowerShell Scripts

**Query a node's status:**
```powershell
# Would need to use a Rust tool or HTTP endpoint
# For now, use the Rust monitor which can query nodes
cargo run --bin node_monitor
```

---

## Enhanced Monitoring Workflow

### Step 1: Query All Nodes

**For each discovered node:**
1. Send `GET_NODE_STATUS` command
2. Collect responses
3. Aggregate state

### Step 2: Analyze State

**Compare across nodes:**
- Do all nodes see the same discovered shards?
- Do all nodes agree on which shards are loaded?
- Why do some nodes think swarm is ready and others don't?

### Step 3: Identify Blockers

**From status responses:**
- Find nodes with `local_shard_loaded: false`
- Find nodes missing shards in `shard_statuses`
- Find discrepancies in `swarm_ready` status

---

## Example: Diagnosing Swarm Not Ready

### Scenario

Swarm isn't ready. You query all 8 nodes with `GET_NODE_STATUS`.

### Node 0 Response:
```json
{
  "local_shard_loaded": true,
  "swarm_ready": false,
  "discovered_shards": 8,
  "all_shards_loaded": false,
  "shard_statuses": {
    "0": { "shard_loaded": true },
    "1": { "shard_loaded": true },
    "2": { "shard_loaded": true },
    "3": { "shard_loaded": false },  // ← Blocker!
    ...
  }
}
```

### Diagnosis:
- Node 0 sees all 8 shards discovered ✅
- But shard 3 is not loaded ❌
- Node 3 needs to load its shard file

### Action:
- Check node 3's `local_shard_loaded` status
- If false, copy shard-3.gguf to node 3
- Restart node 3

---

## Integration with Existing Commands

### Command Hierarchy

**Discovery:**
- `SWARM_STATUS` - Quick swarm status check
- `GET_NODE_STATUS` - Comprehensive node state

**Control:**
- `LOAD_SHARD` - Request node to load a shard
- `GET_CAPABILITIES` - Get node capabilities

**Notifications:**
- `SHARD_LOADED` - Node notifies others it loaded a shard
- `SWARM_READY` - Node notifies others swarm is ready

---

## Benefits

### For Monitoring Tools

✅ **Direct access** to node internal state
✅ **No DHT parsing** needed for node state
✅ **Real-time status** from node's perspective
✅ **Detailed diagnostics** for troubleshooting

### For Debugging

✅ **See exactly** what each node knows
✅ **Identify discrepancies** between nodes
✅ **Find blockers** quickly
✅ **Verify fixes** immediately

### For Automation

✅ **Programmatic queries** of node state
✅ **Automated health checks**
✅ **State aggregation** across nodes
✅ **Alerting** based on status

---

## Next Steps

1. **Update monitoring tools** to use `GET_NODE_STATUS`
2. **Query all nodes** periodically
3. **Aggregate state** across nodes
4. **Display comprehensive** status dashboard
5. **Alert on discrepancies** or blockers

---

## Summary

**Added:**
- `GET_NODE_STATUS` command to nodes
- Comprehensive state reporting
- Per-shard status details
- Local and swarm-level status

**Enables:**
- Direct node state querying
- Better monitoring tools
- Faster debugging
- Automated health checks

**Result:**
- Full visibility into node internal state
- Ability to diagnose issues quickly
- Better understanding of swarm readiness
