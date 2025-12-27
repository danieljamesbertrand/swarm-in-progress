# Dialer - Complete Documentation

## Overview

The **Dialer** is an active peer that discovers nodes via DHT and routes requests to the best nodes based on weighted selection. It acts as a smart router, finding optimal nodes for each task.

## Purpose

- **Node Discovery**: Discovers nodes via Kademlia DHT
- **Weighted Selection**: Selects best nodes based on capabilities
- **Request Routing**: Routes tasks to optimal nodes
- **Reputation Management**: Tracks and updates node reputation

## Capabilities

### Selection Capabilities

- **Multi-Node Discovery**: Finds multiple candidate nodes
- **Capability Querying**: Requests capabilities from nodes
- **Score Calculation**: Calculates weighted scores
- **Optimal Routing**: Routes to highest-scoring nodes
- **Fallback Handling**: Falls back to next-best node on failure

### Network Capabilities

- **DHT Queries**: Queries DHT for node discovery
- **Capability Caching**: Caches node capabilities
- **Reputation Tracking**: Maintains reputation database
- **Load Balancing**: Distributes requests across nodes

## Usage

### Basic Usage

```bash
cargo run --release --bin dialer
```

### With Custom Configuration

```bash
cargo run --release --bin dialer \
  --bootstrap /ip4/127.0.0.1/tcp/51820 \
  --namespace my-app
```

### Arguments

- `--bootstrap <ADDR>`: Bootstrap node address (default: `/ip4/127.0.0.1/tcp/51820`)
- `--namespace <NAMESPACE>`: Namespace for peer discovery (default: `simple-chat`)

## JSON Command Protocol

### Commands Sent

#### 1. FIND_NODES

Find nodes matching criteria.

**Request:**
```json
{
  "command": "FIND_NODES",
  "request_id": "req-123",
  "from": "dialer-peer-id",
  "to": null,
  "timestamp": 1234567890,
  "params": {
    "task_type": "file_share",
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
  "request_id": "req-123",
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

#### 2. GET_CAPABILITIES

Request capabilities from candidate nodes.

**Request:**
```json
{
  "command": "GET_CAPABILITIES",
  "request_id": "req-124",
  "from": "dialer-peer-id",
  "to": "candidate-peer-id",
  "timestamp": 1234567890,
  "params": {}
}
```

#### 3. EXECUTE_TASK

Route task to best node.

**Request:**
```json
{
  "command": "EXECUTE_TASK",
  "request_id": "req-125",
  "from": "dialer-peer-id",
  "to": "best-node-peer-id",
  "timestamp": 1234567890,
  "params": {
    "task_type": "file_share",
    "task_data": {
      "file_hash": "abc123...",
      "piece_index": 0
    }
  }
}
```

#### 4. UPDATE_REPUTATION

Update node reputation after task completion.

**Request:**
```json
{
  "command": "UPDATE_REPUTATION",
  "request_id": "req-126",
  "from": "dialer-peer-id",
  "to": "server-peer-id",
  "timestamp": 1234567890,
  "params": {
    "target_peer": "executor-peer-id",
    "success": true,
    "latency_ms": 12.5,
    "quality_score": 0.98
  }
}
```

## Weighted Selection Algorithm

### Selection Process

1. **Query DHT** for nodes matching criteria
2. **Request Capabilities** from candidate nodes
3. **Calculate Scores** for each node:

```
Score = 
  (0.20 × CPU_Score) +
  (0.15 × Memory_Score) +
  (0.15 × Disk_Score) +
  (0.25 × Latency_Score) +
  (0.25 × Reputation_Score)
```

4. **Sort Nodes** by weighted score
5. **Select Top N** nodes (default: 3)
6. **Route Request** to highest-scoring node
7. **Fallback** to next node if first fails

### Score Calculation Details

**CPU Score:**
```
CPU_Score = (cores / 16) × (1 - usage / 100)
```

**Memory Score:**
```
Memory_Score = available / total
```

**Disk Score:**
```
Disk_Score = available / total
```

**Latency Score:**
```
Latency_Score = 1 / (1 + latency_ms / 100)
```

**Reputation Score:**
```
Reputation_Score = reputation (0.0-1.0)
```

## Example Workflows

### Example 1: Downloading File Piece

1. **Dialer** needs piece 0 of file `abc123...`
2. **Dialer** queries DHT: `FIND_NODES` with file criteria
3. **DHT** returns candidate nodes: `[NodeA, NodeB, NodeC]`
4. **Dialer** requests capabilities from each node
5. **Dialer** calculates scores:
   - NodeA: 0.87 (high disk, low latency, good reputation)
   - NodeB: 0.72 (medium capabilities)
   - NodeC: 0.65 (lower capabilities)
6. **Dialer** routes request to NodeA: `EXECUTE_TASK`
7. **NodeA** executes and returns piece data
8. **Dialer** updates NodeA reputation: `UPDATE_REPUTATION`

### Example 2: Finding Compute Node

1. **Dialer** needs compute node for task
2. **Dialer** queries: `FIND_NODES` with `min_cpu_cores: 8`
3. **DHT** returns compute-capable nodes
4. **Dialer** requests capabilities
5. **Dialer** selects node with highest CPU score
6. **Dialer** routes compute task

### Example 3: Load Balancing

1. **Dialer** has multiple similar requests
2. **Dialer** finds top 3 nodes
3. **Dialer** distributes requests across nodes
4. **Dialer** tracks performance
5. **Dialer** adjusts future routing based on results

## Reputation Management

### Reputation Updates

After each task:
- **Success**: Update reputation with `+0.01` to `+0.03`
- **Failure**: Update reputation with `-0.05`
- **Timeout**: Update reputation with `-0.10`

### Reputation Storage

Dialer maintains local reputation cache:
- **Key**: PeerId
- **Value**: ReputationData
- **Sync**: Updates DHT periodically

## Caching

### Capability Cache

Dialer caches node capabilities:
- **TTL**: 30 seconds
- **Refresh**: On request or TTL expiry
- **Invalidation**: On reputation update

### Node Cache

Dialer caches discovered nodes:
- **Key**: Task criteria
- **Value**: List of candidate nodes
- **TTL**: 60 seconds

## Best Practices

1. **Query Optimization**: Cache DHT queries when possible
2. **Capability Refresh**: Regularly refresh node capabilities
3. **Reputation Tracking**: Maintain accurate reputation data
4. **Fallback Strategy**: Always have backup nodes ready
5. **Load Balancing**: Distribute requests across multiple nodes

## Troubleshooting

### No Nodes Found

- Verify DHT is properly bootstrapped
- Check namespace matches
- Wait for network to populate
- Verify nodes are registered

### Poor Node Selection

- Check capability collection accuracy
- Verify reputation data is current
- Adjust weights if needed
- Monitor selection performance

### High Latency

- Check network connectivity
- Verify node capabilities
- Consider latency weight adjustment
- Use closer nodes when possible













