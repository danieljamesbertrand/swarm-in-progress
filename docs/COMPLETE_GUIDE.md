# Complete P2P Network Guide - JSON Command Protocol

## Overview

This guide provides complete documentation for the P2P network with JSON command protocol, weighted node selection, and reputation tracking.

## Table of Contents

1. [Architecture](#architecture)
2. [JSON Command Protocol](#json-command-protocol)
3. [Node Capabilities](#node-capabilities)
4. [Weighted Selection](#weighted-selection)
5. [Reputation System](#reputation-system)
6. [Node Documentation](#node-documentation)
7. [Examples](#examples)

---

## Architecture

### Network Topology

```
Bootstrap Node (Server)
    │
    ├── Listener Nodes (Task Executors)
    ├── Dialer Nodes (Request Routers)
    ├── Torrent Servers (File Servers)
    └── Monitor (Network Dashboard)
```

### Communication Flow

```
Requester → DHT Query → Candidate Nodes
    ↓
Request Capabilities → Calculate Scores
    ↓
Route to Best Node → Execute Task
    ↓
Update Reputation → Store in DHT
```

---

## JSON Command Protocol

### Command Format

All commands use this structure:

```json
{
  "command": "COMMAND_NAME",
  "request_id": "unique-id",
  "from": "requester-peer-id",
  "to": "target-peer-id-or-null",
  "timestamp": 1234567890,
  "params": {
    "key": "value"
  }
}
```

### Response Format

```json
{
  "command": "COMMAND_NAME",
  "request_id": "unique-id",
  "from": "executor-peer-id",
  "to": "requester-peer-id",
  "timestamp": 1234567890,
  "status": "success|error|timeout",
  "result": {
    "data": "..."
  },
  "error": "error message if status is error"
}
```

### Standard Commands

| Command | Purpose | Request/Response |
|---------|---------|------------------|
| `GET_CAPABILITIES` | Get node capabilities | Request → Capabilities |
| `EXECUTE_TASK` | Execute task on node | Request → Task Result |
| `GET_REPUTATION` | Get node reputation | Request → Reputation Data |
| `UPDATE_REPUTATION` | Update reputation | Request → Confirmation |
| `FIND_NODES` | Find matching nodes | Request → Node List |
| `LIST_FILES` | List available files | Request → File List |
| `GET_FILE_METADATA` | Get file metadata | Request → Metadata |
| `REQUEST_PIECE` | Request file piece | Request → Piece Data |

---

## Node Capabilities

### Capability Metrics

Each node reports:

1. **CPU**
   - Cores: Number of CPU cores
   - Usage: Current CPU usage (0-100%)
   - Speed: CPU speed in GHz

2. **Memory**
   - Total: Total memory in MB
   - Available: Available memory in MB
   - Usage: Memory usage percentage

3. **Disk**
   - Total: Total disk space in MB
   - Available: Available disk space in MB
   - Usage: Disk usage percentage

4. **Latency**
   - Average: Average response latency in ms
   - Network: Network latency to requester

5. **Reputation**
   - Score: Reputation score (0.0-1.0)
   - Based on historical performance

### Capability Collection

Nodes automatically collect capabilities every 5 seconds:

```rust
let capabilities = NodeCapabilities {
    cpu_cores: get_cpu_cores(),
    cpu_usage: get_cpu_usage(),
    memory_total_mb: get_memory_total(),
    memory_available_mb: get_memory_available(),
    disk_total_mb: get_disk_total(),
    disk_available_mb: get_disk_available(),
    latency_ms: measure_latency(),
    reputation: get_reputation_from_dht(),
};
```

---

## Weighted Selection

### Selection Algorithm

Nodes are selected based on weighted score:

```
Weighted Score = 
  (CPU_Weight × CPU_Score) +
  (Memory_Weight × Memory_Score) +
  (Disk_Weight × Disk_Score) +
  (Latency_Weight × Latency_Score) +
  (Reputation_Weight × Reputation_Score)
```

### Default Weights

```rust
NodeWeights {
    cpu: 0.20,        // 20%
    memory: 0.15,     // 15%
    disk: 0.15,       // 15%
    latency: 0.25,    // 25%
    reputation: 0.25, // 25%
}
```

### Score Calculation

**CPU Score:**
```
CPU_Score = (cores / 16) × (1 - usage / 100)
```

**Memory Score:**
```
Memory_Score = available_memory / total_memory
```

**Disk Score:**
```
Disk_Score = available_disk / total_disk
```

**Latency Score:**
```
Latency_Score = 1 / (1 + latency_ms / 100)
```

**Reputation Score:**
```
Reputation_Score = reputation (0.0-1.0)
```

### Selection Process

1. Query DHT for candidate nodes
2. Request capabilities from candidates
3. Calculate weighted scores
4. Sort by score (descending)
5. Select top N nodes (default: 3)
6. Route request to highest-scoring node
7. Fallback to next node on failure

---

## Reputation System

### Reputation Calculation

Reputation starts at `1.0` and adjusts based on performance:

```
New_Reputation = Old_Reputation × Decay + Performance_Score × (1 - Decay)
```

### Performance Factors

1. **Success Rate**: Percentage of successful requests
2. **Response Time**: Average latency
3. **Quality Score**: Subjective quality (0.0-1.0)
4. **Uptime**: Node availability

### Reputation Updates

After each task:

- **Success**: `+0.01` to `+0.03` (capped at 1.0)
- **Failure**: `-0.05` (capped at 0.0)
- **Timeout**: `-0.10`
- **Quality Bonus**: `+0.02` for high quality

### Reputation Storage

Stored in DHT:
- **Key**: `reputation:{peer_id}`
- **Value**: JSON with reputation data
- **Replication**: Stored on k closest nodes

---

## Node Documentation

### Server (Bootstrap Node)

**Purpose**: Bootstrap and coordinate DHT network

**Commands**:
- `GET_CAPABILITIES`
- `FIND_NODES`
- `GET_REPUTATION`
- `UPDATE_REPUTATION`

**Documentation**: [SERVER.md](SERVER.md)

### Listener

**Purpose**: Execute tasks from other nodes

**Commands**:
- `GET_CAPABILITIES`
- `EXECUTE_TASK`
- `GET_REPUTATION`

**Documentation**: [LISTENER.md](LISTENER.md)

### Dialer

**Purpose**: Route requests to best nodes

**Commands**:
- `FIND_NODES`
- `GET_CAPABILITIES`
- `EXECUTE_TASK`
- `UPDATE_REPUTATION`

**Documentation**: [DIALER.md](DIALER.md)

### Monitor

**Purpose**: Network monitoring dashboard

**Commands**:
- `GET_CAPABILITIES` (from all nodes)
- `GET_REPUTATION` (from all nodes)
- `FIND_NODES`

**Documentation**: [MONITOR.md](MONITOR.md)

### Torrent Server

**Purpose**: Serve files via BitTorrent protocol

**Commands**:
- `GET_CAPABILITIES`
- `LIST_FILES`
- `GET_FILE_METADATA`
- `EXECUTE_TASK` (file piece requests)

**Documentation**: [TORRENT_SERVER.md](TORRENT_SERVER.md)

### Torrent Client

**Purpose**: Download files from peers

**Commands**:
- `FIND_NODES` (finds best file sources)
- `GET_CAPABILITIES`
- `EXECUTE_TASK` (downloads pieces)
- `UPDATE_REPUTATION`

**Documentation**: [TORRENT_CLIENT.md](TORRENT_CLIENT.md)

---

## Examples

### Example 1: Download File Piece

```json
// 1. Client finds nodes
{
  "command": "FIND_NODES",
  "params": {
    "task_type": "file_share",
    "file_hash": "abc123...",
    "min_disk_mb": 1000
  }
}

// 2. Client gets capabilities
{
  "command": "GET_CAPABILITIES",
  "to": "node-a-peer-id"
}

// 3. Client routes to best node
{
  "command": "EXECUTE_TASK",
  "to": "best-node-peer-id",
  "params": {
    "task_type": "file_share",
    "task_data": {
      "file_hash": "abc123...",
      "piece_index": 0
    }
  }
}

// 4. Client updates reputation
{
  "command": "UPDATE_REPUTATION",
  "params": {
    "target_peer": "best-node-peer-id",
    "success": true,
    "latency_ms": 12.5,
    "quality_score": 0.98
  }
}
```

### Example 2: Find Compute Node

```json
// Find node with high CPU
{
  "command": "FIND_NODES",
  "params": {
    "task_type": "compute",
    "min_cpu_cores": 8,
    "max_cpu_usage": 50
  }
}
```

### Example 3: Load Balance Requests

```json
// Dialer distributes across top 3 nodes
{
  "command": "EXECUTE_TASK",
  "to": "node-1-peer-id",  // Score: 0.87
  "params": {"task": "task-1"}
}

{
  "command": "EXECUTE_TASK",
  "to": "node-2-peer-id",  // Score: 0.82
  "params": {"task": "task-2"}
}

{
  "command": "EXECUTE_TASK",
  "to": "node-3-peer-id",  // Score: 0.78
  "params": {"task": "task-3"}
}
```

---

## Best Practices

1. **Capability Refresh**: Refresh capabilities every 30 seconds
2. **Reputation Updates**: Update reputation after each task
3. **Fallback Strategy**: Always have backup nodes ready
4. **Load Balancing**: Distribute requests across multiple nodes
5. **Error Handling**: Handle timeouts and failures gracefully

---

## Troubleshooting

### No Nodes Found

- Verify DHT is bootstrapped
- Check namespace matches
- Wait for network to populate

### Poor Selection

- Verify capability accuracy
- Check reputation data
- Adjust weights if needed

### High Latency

- Check network connectivity
- Use closer nodes
- Consider latency weight

---

## Summary

✅ **JSON Commands**: All nodes communicate via JSON  
✅ **Weighted Selection**: Requests routed to best nodes  
✅ **Reputation Tracking**: Nodes maintain reputation scores  
✅ **Capability Reporting**: Nodes report CPU, memory, disk, latency  
✅ **Unique Addressing**: Each node uniquely addressable by PeerId  

The network provides intelligent, capability-based request routing with reputation tracking for optimal performance!












