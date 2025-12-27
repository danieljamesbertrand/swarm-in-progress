# Server (Bootstrap Node) - Complete Documentation

## Overview

The **Server** is a bootstrap node that helps peers join the Kademlia DHT network. It acts as an entry point for the decentralized network and provides coordination services.

## Purpose

- **Bootstrap Coordination**: Helps new peers discover and join the DHT network
- **Relay Server**: Acts as a relay node for NAT traversal
- **Node Registry**: Maintains registry of node capabilities
- **Reputation Tracking**: Tracks node reputation scores

## Capabilities

### System Capabilities

The server reports its capabilities:

- **CPU**: Number of cores, usage percentage, speed
- **Memory**: Total and available memory
- **Disk**: Total and available disk space
- **Latency**: Average response latency
- **Reputation**: Current reputation score (starts at 1.0)

### Network Capabilities

- **DHT Bootstrap**: Coordinates DHT network bootstrap
- **Relay Service**: Provides relay for NAT traversal
- **Node Discovery**: Helps peers discover each other
- **Capability Registry**: Stores node capabilities in DHT

## Usage

### Basic Usage

```bash
cargo run --release --bin server
```

### With Custom Address

```bash
cargo run --release --bin server \
  --listen-addr 0.0.0.0 \
  --port 51820
```

### Arguments

- `--listen-addr <ADDR>`: Address to listen on (default: `0.0.0.0`)
- `--port <PORT>`: Port to listen on (default: `51820`)

## JSON Command Protocol

### Commands Handled

#### 1. GET_CAPABILITIES

Request server capabilities.

**Request:**
```json
{
  "command": "GET_CAPABILITIES",
  "request_id": "req-123",
  "from": "12D3KooW...",
  "to": "server-peer-id",
  "timestamp": 1234567890,
  "params": {}
}
```

**Response:**
```json
{
  "command": "GET_CAPABILITIES",
  "request_id": "req-123",
  "from": "server-peer-id",
  "to": "12D3KooW...",
  "timestamp": 1234567890,
  "status": "success",
  "result": {
    "cpu_cores": 8,
    "cpu_usage": 25.5,
    "cpu_speed_ghz": 2.5,
    "memory_total_mb": 16384,
    "memory_available_mb": 8192,
    "disk_total_mb": 1000000,
    "disk_available_mb": 500000,
    "latency_ms": 5.2,
    "reputation": 1.0
  }
}
```

#### 2. FIND_NODES

Find nodes matching criteria.

**Request:**
```json
{
  "command": "FIND_NODES",
  "request_id": "req-124",
  "from": "12D3KooW...",
  "to": "server-peer-id",
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
  "request_id": "req-124",
  "from": "server-peer-id",
  "to": "12D3KooW...",
  "timestamp": 1234567890,
  "status": "success",
  "result": {
    "nodes": [
      {
        "peer_id": "12D3KooW...",
        "capabilities": {
          "cpu_cores": 8,
          "memory_available_mb": 8192,
          "disk_available_mb": 500000,
          "latency_ms": 12.5,
          "reputation": 0.95
        },
        "weighted_score": 0.87
      }
    ]
  }
}
```

#### 3. GET_REPUTATION

Get node reputation.

**Request:**
```json
{
  "command": "GET_REPUTATION",
  "request_id": "req-125",
  "from": "12D3KooW...",
  "to": "server-peer-id",
  "timestamp": 1234567890,
  "params": {
    "target_peer": "12D3KooW..."
  }
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

#### 4. UPDATE_REPUTATION

Update node reputation.

**Request:**
```json
{
  "command": "UPDATE_REPUTATION",
  "request_id": "req-126",
  "from": "12D3KooW...",
  "to": "server-peer-id",
  "timestamp": 1234567890,
  "params": {
    "target_peer": "12D3KooW...",
    "success": true,
    "latency_ms": 12.5,
    "quality_score": 0.98
  }
}
```

**Response:**
```json
{
  "command": "UPDATE_REPUTATION",
  "request_id": "req-126",
  "status": "success",
  "result": {
    "updated_reputation": 0.96
  }
}
```

## Example Interactions

### Example 1: Peer Joining Network

1. **Peer** connects to server
2. **Peer** requests bootstrap: `kademlia.bootstrap()`
3. **Server** provides DHT routing information
4. **Peer** joins DHT network

### Example 2: Finding Capable Nodes

1. **Client** sends `FIND_NODES` command
2. **Server** queries DHT for matching nodes
3. **Server** calculates weighted scores
4. **Server** returns sorted list of best nodes

### Example 3: Reputation Update

1. **Node A** completes task for **Node B**
2. **Node B** sends `UPDATE_REPUTATION` to server
3. **Server** updates reputation in DHT
4. **Server** confirms update

## Integration

The server integrates with:

- **Kademlia DHT**: Provides bootstrap and routing
- **Relay Protocol**: Enables NAT traversal
- **Identify Protocol**: Exchanges peer information
- **Command Protocol**: Handles JSON commands

## Monitoring

Monitor server status:

- **Connections**: Number of connected peers
- **Bootstrap Requests**: Number of bootstrap operations
- **Relay Circuits**: Number of active relay circuits
- **DHT Queries**: Number of DHT queries handled

## Best Practices

1. **Deploy on Public IP**: Server should be accessible from internet
2. **High Availability**: Run multiple bootstrap nodes for redundancy
3. **Monitor Resources**: Track CPU, memory, and bandwidth usage
4. **Update Reputation**: Regularly update reputation data in DHT

## Troubleshooting

### Server Not Accessible

- Check firewall rules
- Verify listen address (use `0.0.0.0` for remote access)
- Check port availability

### Bootstrap Fails

- Verify server is running
- Check network connectivity
- Verify DHT is properly initialized

### High Resource Usage

- Monitor CPU and memory
- Limit concurrent connections if needed
- Consider running on dedicated hardware













