# Documentation Index

Complete documentation for the P2P network with JSON command protocol, weighted node selection, and reputation tracking.

## Quick Start

- **[Complete Guide](COMPLETE_GUIDE.md)** - Start here for overview
- **[Node Documentation](NODE_DOCUMENTATION.md)** - All nodes overview
- **[External IP Connection Guide](../EXTERNAL_IP_CONNECTION.md)** - Connect peers across the internet

## Node Documentation

### Core Nodes

- **[Server](SERVER.md)** - Bootstrap node documentation
- **[Listener](LISTENER.md)** - Task executor documentation
- **[Dialer](DIALER.md)** - Request router documentation

### Specialized Nodes

- **[Monitor](MONITOR.md)** - Network monitoring dashboard
- **[Torrent Server](TORRENT_SERVER.md)** - File server documentation
- **[Torrent Client](TORRENT_CLIENT.md)** - File client documentation

## Key Concepts

### JSON Command Protocol

All nodes communicate via standardized JSON commands:

```json
{
  "command": "COMMAND_NAME",
  "request_id": "unique-id",
  "from": "requester-peer-id",
  "to": "target-peer-id",
  "timestamp": 1234567890,
  "params": {...}
}
```

See [Complete Guide](COMPLETE_GUIDE.md#json-command-protocol) for details.

### Weighted Node Selection

Nodes are selected based on weighted score considering:
- CPU power (20%)
- Memory availability (15%)
- Disk space (15%)
- Latency (25%)
- Reputation (25%)

See [Complete Guide](COMPLETE_GUIDE.md#weighted-selection) for algorithm.

### Reputation System

Nodes maintain reputation scores (0.0-1.0) based on:
- Success rate
- Response time
- Quality score
- Uptime

See [Complete Guide](COMPLETE_GUIDE.md#reputation-system) for details.

## Examples

### Example 1: Download File Piece

1. Client queries DHT for nodes with file
2. Client requests capabilities from candidates
3. Client calculates weighted scores
4. Client routes request to best node
5. Client updates node reputation

See [Complete Guide](COMPLETE_GUIDE.md#examples) for full examples.

## Architecture

```
Bootstrap Node (Server)
    │
    ├── Listener Nodes (Task Executors)
    ├── Dialer Nodes (Request Routers)
    ├── Torrent Servers (File Servers)
    └── Monitor (Network Dashboard)
```

All nodes:
- Uniquely addressable by PeerId
- Communicate via JSON commands
- Report capabilities (CPU, memory, disk, latency)
- Maintain reputation scores
- Support weighted selection

## Command Reference

| Command | Purpose |
|--------|---------|
| `GET_CAPABILITIES` | Get node capabilities |
| `EXECUTE_TASK` | Execute task on node |
| `GET_REPUTATION` | Get node reputation |
| `UPDATE_REPUTATION` | Update reputation |
| `FIND_NODES` | Find matching nodes |
| `LIST_FILES` | List available files |
| `GET_FILE_METADATA` | Get file metadata |
| `REQUEST_PIECE` | Request file piece |

See [Node Documentation](NODE_DOCUMENTATION.md#json-command-protocol) for full command reference.

## Implementation Status

✅ **Documentation**: Complete for all nodes  
✅ **Command Protocol**: Defined and documented  
✅ **Capability Collection**: System metrics collection  
✅ **Weighted Selection**: Algorithm documented  
✅ **Reputation System**: Tracking system documented  

🚧 **Implementation**: Code implementation in progress

## Next Steps

1. Review [Complete Guide](COMPLETE_GUIDE.md)
2. Read node-specific documentation
3. Review command protocol examples
4. Understand weighted selection algorithm
5. Learn reputation system

## Network Configuration

### External IP Connections

For connecting peers across the internet:
- **[External IP Connection Guide](../EXTERNAL_IP_CONNECTION.md)** - Complete guide for public IP connections
- Bootstrap node setup with public IPs
- NAT traversal and port forwarding
- Troubleshooting connection issues
- Security and performance optimization

### NAT Traversal

- **[Relay Protocol Guide](../RELAY_PROTOCOL_GUIDE.md)** - libp2p relay protocol details
- Automatic NAT traversal
- Relay server configuration

## Support

For questions or issues:
- Review node-specific documentation
- Check [Complete Guide](COMPLETE_GUIDE.md) for examples
- See troubleshooting sections in each node doc
- Review [External IP Connection Guide](../EXTERNAL_IP_CONNECTION.md) for network issues

