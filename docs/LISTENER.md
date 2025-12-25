# Listener - Complete Documentation

## Overview

The **Listener** is a passive peer that joins the DHT network and waits for incoming connections and task requests. It executes tasks from other nodes based on their capabilities and maintains reputation.

## Purpose

- **Task Execution**: Executes tasks requested by other nodes
- **Capability Reporting**: Reports CPU, memory, disk, and latency
- **Reputation Maintenance**: Maintains reputation through successful task execution
- **File Serving**: Can serve files if configured as torrent server

## Capabilities

### System Capabilities

The listener automatically collects and reports:

- **CPU**: Number of cores, current usage, speed
- **Memory**: Total and available memory
- **Disk**: Total and available disk space
- **Latency**: Average response latency (measured from requests)
- **Reputation**: Current reputation score (updated after each task)

### Task Execution Capabilities

- **File Sharing**: Serve file pieces for torrent downloads
- **Compute Tasks**: Execute computational tasks (if implemented)
- **Storage Tasks**: Store and retrieve data
- **Custom Tasks**: Execute application-specific tasks

## Usage

### Basic Usage

```bash
cargo run --release --bin listener
```

### With Custom Configuration

```bash
cargo run --release --bin listener \
  --bootstrap /ip4/127.0.0.1/tcp/51820 \
  --namespace my-app
```

### Arguments

- `--bootstrap <ADDR>`: Bootstrap node address (default: `/ip4/127.0.0.1/tcp/51820`)
- `--namespace <NAMESPACE>`: Namespace for peer discovery (default: `simple-chat`)

## JSON Command Protocol

### Commands Handled

#### 1. GET_CAPABILITIES

Request listener capabilities.

**Request:**
```json
{
  "command": "GET_CAPABILITIES",
  "request_id": "req-123",
  "from": "12D3KooW...",
  "to": "listener-peer-id",
  "timestamp": 1234567890,
  "params": {}
}
```

**Response:**
```json
{
  "command": "GET_CAPABILITIES",
  "request_id": "req-123",
  "from": "listener-peer-id",
  "to": "12D3KooW...",
  "timestamp": 1234567890,
  "status": "success",
  "result": {
    "cpu_cores": 4,
    "cpu_usage": 35.2,
    "cpu_speed_ghz": 2.5,
    "memory_total_mb": 8192,
    "memory_available_mb": 4096,
    "disk_total_mb": 500000,
    "disk_available_mb": 250000,
    "latency_ms": 15.3,
    "reputation": 0.92
  }
}
```

#### 2. EXECUTE_TASK

Execute a task on the listener.

**Request:**
```json
{
  "command": "EXECUTE_TASK",
  "request_id": "req-124",
  "from": "12D3KooW...",
  "to": "listener-peer-id",
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

**Response (Success):**
```json
{
  "command": "EXECUTE_TASK",
  "request_id": "req-124",
  "from": "listener-peer-id",
  "to": "12D3KooW...",
  "timestamp": 1234567890,
  "status": "success",
  "result": {
    "piece_data": "base64-encoded-data...",
    "piece_hash": "sha256-hash..."
  }
}
```

**Response (Error):**
```json
{
  "command": "EXECUTE_TASK",
  "request_id": "req-124",
  "from": "listener-peer-id",
  "to": "12D3KooW...",
  "timestamp": 1234567890,
  "status": "error",
  "error": "File not found"
}
```

#### 3. GET_REPUTATION

Get listener reputation.

**Request:**
```json
{
  "command": "GET_REPUTATION",
  "request_id": "req-125",
  "from": "12D3KooW...",
  "to": "listener-peer-id",
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
    "reputation": 0.92,
    "total_requests": 500,
    "successful_requests": 460,
    "failed_requests": 40,
    "average_latency_ms": 18.5
  }
}
```

## Task Types

### file_share

Share a file piece for torrent downloads.

**Task Data:**
```json
{
  "file_hash": "abc123...",
  "piece_index": 0
}
```

**Response:**
```json
{
  "piece_data": "base64-encoded-data",
  "piece_hash": "sha256-hash",
  "piece_size": 65536
}
```

### compute

Execute computational task (if implemented).

**Task Data:**
```json
{
  "computation_type": "map_reduce",
  "input_data": "..."
}
```

### storage

Store or retrieve data.

**Task Data:**
```json
{
  "operation": "store|retrieve",
  "key": "data-key",
  "value": "data-value"
}
```

## Example Interactions

### Example 1: Receiving Task Request

1. **Dialer** sends `EXECUTE_TASK` command
2. **Listener** validates request
3. **Listener** executes task
4. **Listener** returns result
5. **Dialer** updates listener reputation

### Example 2: Capability Query

1. **Client** sends `GET_CAPABILITIES` command
2. **Listener** collects current system metrics
3. **Listener** returns capabilities
4. **Client** uses for weighted selection

### Example 3: Reputation Update

1. **Listener** completes task successfully
2. **Requester** sends `UPDATE_REPUTATION` to DHT
3. **Listener** reputation increases
4. **Listener** more likely to be selected for future tasks

## Weighted Selection

The listener's weighted score is calculated as:

```
Score = 
  (0.20 × CPU_Score) +
  (0.15 × Memory_Score) +
  (0.15 × Disk_Score) +
  (0.25 × Latency_Score) +
  (0.25 × Reputation_Score)
```

**Factors:**
- **CPU**: More cores, lower usage = higher score
- **Memory**: More available = higher score
- **Disk**: More available = higher score
- **Latency**: Lower latency = higher score
- **Reputation**: Higher reputation = higher score

## Reputation Management

### Reputation Updates

After each task:
- **Success**: Reputation increases by 0.01-0.03
- **Failure**: Reputation decreases by 0.05
- **Timeout**: Reputation decreases by 0.10

### Reputation Storage

Reputation stored in DHT:
- **Key**: `reputation:{peer_id}`
- **Value**: JSON with reputation data
- **Replication**: Stored on k closest nodes

## Best Practices

1. **Maintain Resources**: Keep CPU, memory, and disk available
2. **Fast Response**: Minimize latency for better reputation
3. **Reliability**: Ensure high success rate
4. **Monitor Performance**: Track task execution metrics

## Troubleshooting

### Tasks Not Received

- Verify listener is connected to DHT
- Check namespace matches requester
- Verify listener is registered in DHT

### High Latency

- Check network connectivity
- Monitor system resources
- Optimize task execution

### Low Reputation

- Improve task success rate
- Reduce response latency
- Ensure reliable task execution

