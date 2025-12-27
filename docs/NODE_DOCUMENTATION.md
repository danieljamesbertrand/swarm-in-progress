# Complete Node Documentation

## Overview

This document provides comprehensive documentation for all nodes in the P2P network. Each node is uniquely addressable and communicates via JSON commands, with intelligent request routing based on node capabilities.

## Table of Contents

1. [Architecture Overview](#architecture-overview)
2. [JSON Command Protocol](#json-command-protocol)
3. [Node Capabilities](#node-capabilities)
4. [Weighted Node Selection](#weighted-node-selection)
5. [Reputation System](#reputation-system)
6. [Individual Node Documentation](#individual-node-documentation)
   - [Server (Bootstrap Node)](#server-bootstrap-node)
   - [Listener](#listener)
   - [Dialer](#dialer)
   - [Monitor](#monitor)
   - [Torrent Server](#torrent-server)
   - [Torrent Client](#torrent-client)

---

## Architecture Overview

### Node Types

All nodes in the network share common characteristics:

- **Uniquely Addressable**: Each node has a unique PeerId
- **JSON Commands**: All inter-node communication uses JSON
- **Capability Reporting**: Nodes report CPU, memory, disk, latency
- **Reputation Tracking**: Nodes maintain reputation scores
- **Weighted Selection**: Requests routed to best nodes

### Communication Flow

```
Node A (Requester)
  ├── Queries DHT for capable nodes
  ├── Receives node capabilities
  ├── Calculates weighted scores
  └── Routes request to best node

Node B (Executor)
  ├── Receives JSON command
  ├── Validates request
  ├── Executes command
  └── Returns JSON response
```

---

## JSON Command Protocol

### Command Structure

All commands follow this structure:

```json
{
  "command": "COMMAND_NAME",
  "request_id": "unique-request-id",
  "from": "peer-id-of-requester",
  "to": "peer-id-of-target",
  "timestamp": 1234567890,
  "params": {
    "param1": "value1",
    "param2": "value2"
  }
}
```

### Response Structure

```json
{
  "command": "COMMAND_NAME",
  "request_id": "unique-request-id",
  "from": "peer-id-of-executor",
  "to": "peer-id-of-requester",
  "timestamp": 1234567890,
  "status": "success|error",
  "result": {
    "data": "response data"
  },
  "error": "error message if status is error"
}
```

### Standard Commands

#### 1. `GET_CAPABILITIES`

Request node capabilities.

**Request:**
```json
{
  "command": "GET_CAPABILITIES",
  "request_id": "req-123",
  "from": "12D3KooW...",
  "to": "12D3KooW...",
  "timestamp": 1234567890,
  "params": {}
}
```

**Response:**
```json
{
  "command": "GET_CAPABILITIES",
  "request_id": "req-123",
  "from": "12D3KooW...",
  "to": "12D3KooW...",
  "timestamp": 1234567890,
  "status": "success",
  "result": {
    "cpu_cores": 8,
    "cpu_usage": 45.2,
    "memory_total": 16384,
    "memory_available": 8192,
    "disk_total": 1000000,
    "disk_available": 500000,
    "latency_ms": 12.5,
    "reputation": 0.95
  }
}
```

#### 2. `EXECUTE_TASK`

Execute a task on the target node.

**Request:**
```json
{
  "command": "EXECUTE_TASK",
  "request_id": "req-124",
  "from": "12D3KooW...",
  "to": "12D3KooW...",
  "timestamp": 1234567890,
  "params": {
    "task_type": "file_share|compute|storage",
    "task_data": {...}
  }
}
```

#### 3. `GET_REPUTATION`

Get node reputation score.

**Request:**
```json
{
  "command": "GET_REPUTATION",
  "request_id": "req-125",
  "from": "12D3KooW...",
  "to": "12D3KooW...",
  "timestamp": 1234567890,
  "params": {}
}
```

**Response:**
```json
{
  "command": "GET_REPUTATION",
  "request_id": "req-125",
  "status": "success",
  "result": {
    "reputation": 0.95,
    "total_requests": 1000,
    "successful_requests": 950,
    "failed_requests": 50,
    "average_latency_ms": 15.2
  }
}
```

#### 4. `UPDATE_REPUTATION`

Update node reputation (called after task completion).

**Request:**
```json
{
  "command": "UPDATE_REPUTATION",
  "request_id": "req-126",
  "from": "12D3KooW...",
  "to": "12D3KooW...",
  "timestamp": 1234567890,
  "params": {
    "target_peer": "12D3KooW...",
    "success": true,
    "latency_ms": 12.5,
    "quality_score": 0.98
  }
}
```

#### 5. `FIND_NODES`

Find nodes matching criteria.

**Request:**
```json
{
  "command": "FIND_NODES",
  "request_id": "req-127",
  "from": "12D3KooW...",
  "to": "12D3KooW...",
  "timestamp": 1234567890,
  "params": {
    "min_cpu_cores": 4,
    "min_memory_mb": 4096,
    "min_disk_mb": 100000,
    "max_latency_ms": 100
  }
}
```

**Response:**
```json
{
  "command": "FIND_NODES",
  "request_id": "req-127",
  "status": "success",
  "result": {
    "nodes": [
      {
        "peer_id": "12D3KooW...",
        "capabilities": {...},
        "weighted_score": 0.87
      }
    ]
  }
}
```

---

## Node Capabilities

### Capability Metrics

Each node reports:

1. **CPU Power**
   - CPU cores (integer)
   - CPU usage percentage (0-100)
   - CPU speed (GHz)

2. **Memory**
   - Total memory (MB)
   - Available memory (MB)
   - Memory usage percentage

3. **Disk Space**
   - Total disk space (MB)
   - Available disk space (MB)
   - Disk usage percentage

4. **Latency**
   - Average response latency (ms)
   - Network latency to requester (ms)

5. **Reputation**
   - Reputation score (0.0-1.0)
   - Based on historical performance

### Capability Collection

Nodes automatically collect capabilities:

```rust
struct NodeCapabilities {
    cpu_cores: u32,
    cpu_usage: f64,
    memory_total_mb: u64,
    memory_available_mb: u64,
    disk_total_mb: u64,
    disk_available_mb: u64,
    latency_ms: f64,
    reputation: f64,
}
```

---

## Weighted Node Selection

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

### Score Calculation

**CPU Score:**
```
CPU_Score = (available_cores / max_cores) × (1 - cpu_usage / 100)
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

### Default Weights

```rust
const WEIGHTS: NodeWeights = NodeWeights {
    cpu: 0.20,        // 20% weight
    memory: 0.15,     // 15% weight
    disk: 0.15,       // 15% weight
    latency: 0.25,    // 25% weight
    reputation: 0.25, // 25% weight
};
```

### Selection Process

1. **Query DHT** for nodes matching criteria
2. **Request capabilities** from candidate nodes
3. **Calculate weighted scores** for each node
4. **Select top N nodes** (default: 3)
5. **Route request** to highest-scoring node
6. **Fallback** to next node if first fails

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

Reputation updated after each task:

- **Success**: `+0.01` (capped at 1.0)
- **Failure**: `-0.05` (capped at 0.0)
- **Timeout**: `-0.10`
- **Quality Bonus**: `+0.02` for high quality

### Reputation Storage

Reputation stored in DHT:
- **Key**: `reputation:{peer_id}`
- **Value**: JSON with reputation data
- **Replication**: Stored on k closest nodes

---

## Individual Node Documentation

### Server (Bootstrap Node)

**Purpose**: Bootstrap node for DHT network

**Capabilities**:
- DHT bootstrap coordination
- Relay server for NAT traversal
- Node capability registry
- Reputation tracking

**Commands Handled**:
- `GET_CAPABILITIES`
- `FIND_NODES`
- `GET_REPUTATION`
- `UPDATE_REPUTATION`

**Usage**:
```bash
cargo run --release --bin server \
  --listen-addr 0.0.0.0 \
  --port 51820
```

**JSON Commands**:
```json
// Get server capabilities
{
  "command": "GET_CAPABILITIES",
  "to": "server-peer-id"
}

// Find nodes with specific capabilities
{
  "command": "FIND_NODES",
  "params": {
    "min_cpu_cores": 4,
    "min_memory_mb": 4096
  }
}
```

---

### Listener

**Purpose**: Passive peer that waits for connections and executes tasks

**Capabilities**:
- Accepts incoming connections
- Executes tasks from other nodes
- Reports capabilities
- Maintains reputation

**Commands Handled**:
- `GET_CAPABILITIES`
- `EXECUTE_TASK`
- `GET_REPUTATION`
- `UPDATE_REPUTATION`

**Usage**:
```bash
cargo run --release --bin listener \
  --bootstrap /ip4/127.0.0.1/tcp/51820 \
  --namespace my-app
```

**JSON Commands**:
```json
// Request listener to execute task
{
  "command": "EXECUTE_TASK",
  "to": "listener-peer-id",
  "params": {
    "task_type": "file_share",
    "task_data": {
      "file_hash": "abc123...",
      "piece_index": 0
    }
  }
}
```

---

### Dialer

**Purpose**: Active peer that discovers nodes and routes requests

**Capabilities**:
- Discovers nodes via DHT
- Routes requests to best nodes
- Calculates weighted scores
- Manages request distribution

**Commands Handled**:
- `GET_CAPABILITIES` (queries other nodes)
- `FIND_NODES`
- `EXECUTE_TASK` (routes to best node)
- `UPDATE_REPUTATION`

**Usage**:
```bash
cargo run --release --bin dialer \
  --bootstrap /ip4/127.0.0.1/tcp/51820 \
  --namespace my-app
```

**JSON Commands**:
```json
// Dialer finds best node and routes request
{
  "command": "EXECUTE_TASK",
  "from": "dialer-peer-id",
  "params": {
    "task_type": "compute",
    "task_data": {...},
    "auto_route": true  // Dialer finds best node
  }
}
```

---

### Monitor

**Purpose**: Network monitoring and management dashboard

**Capabilities**:
- Real-time network monitoring
- Node capability tracking
- Reputation visualization
- Request routing analytics
- Web dashboard

**Commands Handled**:
- `GET_CAPABILITIES` (from all nodes)
- `GET_REPUTATION` (from all nodes)
- `FIND_NODES`
- `UPDATE_REPUTATION`

**Usage**:
```bash
cargo run --release --bin monitor \
  --listen-addr 0.0.0.0 \
  --port 51820 \
  --web-port 8080
```

**JSON Commands**:
```json
// Monitor queries all nodes for capabilities
{
  "command": "GET_CAPABILITIES",
  "to": "monitor-peer-id"
}

// Monitor updates reputation
{
  "command": "UPDATE_REPUTATION",
  "to": "monitor-peer-id",
  "params": {
    "target_peer": "12D3KooW...",
    "success": true,
    "latency_ms": 12.5
  }
}
```

**Web Dashboard**: `http://localhost:8080`

---

### Torrent Server

**Purpose**: Serves files via BitTorrent-like protocol

**Capabilities**:
- File sharing
- Piece serving
- High disk availability
- File metadata management

**Commands Handled**:
- `GET_CAPABILITIES`
- `EXECUTE_TASK` (file piece requests)
- `GET_REPUTATION`
- `LIST_FILES`
- `GET_FILE_METADATA`
- `REQUEST_PIECE`

**Usage**:
```bash
cargo run --release --bin torrent_server \
  --bootstrap /ip4/127.0.0.1/tcp/51820 \
  --share-dir ./shared
```

**JSON Commands**:
```json
// Request file list
{
  "command": "LIST_FILES",
  "to": "torrent-server-peer-id"
}

// Request file piece
{
  "command": "EXECUTE_TASK",
  "to": "torrent-server-peer-id",
  "params": {
    "task_type": "file_share",
    "task_data": {
      "file_hash": "abc123...",
      "piece_index": 0
    }
  }
}
```

---

### Torrent Client

**Purpose**: Downloads files from peers

**Capabilities**:
- File discovery
- Piece downloading
- Node selection for downloads
- Download management

**Commands Handled**:
- `GET_CAPABILITIES` (queries servers)
- `FIND_NODES` (finds best file sources)
- `EXECUTE_TASK` (downloads pieces)
- `UPDATE_REPUTATION` (rates servers)

**Usage**:
```bash
cargo run --release --bin torrent_client \
  --bootstrap /ip4/127.0.0.1/tcp/51820 \
  --download-dir ./downloads
```

**JSON Commands**:
```json
// Client finds best nodes for file
{
  "command": "FIND_NODES",
  "from": "torrent-client-peer-id",
  "params": {
    "task_type": "file_share",
    "file_hash": "abc123...",
    "min_disk_mb": 1000
  }
}

// Client requests piece from best node
{
  "command": "EXECUTE_TASK",
  "to": "best-torrent-server-peer-id",
  "params": {
    "task_type": "file_share",
    "task_data": {
      "file_hash": "abc123...",
      "piece_index": 0
    }
  }
}
```

---

## Request Routing Example

### Scenario: Download File Piece

1. **Client** needs piece 0 of file `abc123...`

2. **Client** queries DHT:
   ```json
   {
     "command": "FIND_NODES",
     "params": {
       "task_type": "file_share",
       "file_hash": "abc123...",
       "min_disk_mb": 1000
     }
   }
   ```

3. **DHT** returns candidate nodes: `[NodeA, NodeB, NodeC]`

4. **Client** requests capabilities:
   ```json
   {
     "command": "GET_CAPABILITIES",
     "to": "NodeA"
   }
   ```

5. **Client** calculates weighted scores:
   - NodeA: 0.87 (high disk, low latency, good reputation)
   - NodeB: 0.72 (medium disk, medium latency)
   - NodeC: 0.65 (low disk, high latency)

6. **Client** routes request to NodeA:
   ```json
   {
     "command": "EXECUTE_TASK",
     "to": "NodeA",
     "params": {
       "task_type": "file_share",
       "task_data": {
         "file_hash": "abc123...",
         "piece_index": 0
       }
     }
   }
   ```

7. **NodeA** executes task and responds:
   ```json
   {
     "command": "EXECUTE_TASK",
     "status": "success",
     "result": {
       "piece_data": "..."
     }
   }
   ```

8. **Client** updates NodeA reputation:
   ```json
   {
     "command": "UPDATE_REPUTATION",
     "params": {
       "target_peer": "NodeA",
       "success": true,
       "latency_ms": 12.5,
       "quality_score": 0.98
     }
   }
   ```

---

## Next Steps

See individual node documentation files:
- [Server Documentation](SERVER.md)
- [Listener Documentation](LISTENER.md)
- [Dialer Documentation](DIALER.md)
- [Monitor Documentation](MONITOR.md)
- [Torrent Server Documentation](TORRENT_SERVER.md)
- [Torrent Client Documentation](TORRENT_CLIENT.md)












