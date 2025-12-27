# Network Monitoring System Guide

A comprehensive web-based monitoring and management system for the Kademlia P2P network.

## Overview

The monitoring system provides:
- **Real-time Network Visualization**: Watch nodes join and leave
- **Live Metrics Dashboard**: Track network health and growth
- **Node Tracking**: Monitor all connected peers
- **Communication Logging**: Track all connection events
- **Namespace Management**: See which namespaces are active
- **WebSocket Updates**: Real-time updates without page refresh

## Features

### 📊 Metrics Dashboard
- Total nodes in network
- Active connections
- Total connections (lifetime)
- DHT records count
- System uptime

### 🖥️ Node Management
- List all connected nodes
- View node details (Peer ID, agent, addresses)
- Track connection history per node
- See first/last seen timestamps

### 🔗 Connection Tracking
- Real-time connection events
- Connection/disconnection logs
- Inbound/outbound direction tracking
- Timestamped events

### 📁 Namespace Monitoring
- Active namespaces
- Node count per namespace
- Namespace-based grouping

### 📈 Network Growth Chart
- Real-time line chart
- Tracks total nodes over time
- Tracks active connections over time
- Last 50 data points displayed

## Installation

### Dependencies

The monitor requires additional dependencies (already added to `Cargo.toml`):
- `axum` - Web framework
- `futures-util` - Async utilities
- `serde` - Serialization (already included)

### Build

```bash
cargo build --release --bin monitor
```

## Usage

### Start the Monitor

```bash
# Default (P2P on 0.0.0.0:51820, Web on 8080)
cargo run --release --bin monitor

# Custom ports
cargo run --release --bin monitor \
  --listen-addr 0.0.0.0 \
  --port 51820 \
  --web-port 8080
```

### Access the Dashboard

Open your browser to:
```
http://localhost:8080
```

## Architecture

### Components

1. **P2P Monitor Node** (`src/monitor.rs`)
   - Acts as bootstrap node for the network
   - Tracks all network events
   - Maintains network state in memory
   - Broadcasts updates via WebSocket

2. **Web Server** (Axum)
   - Serves HTML dashboard
   - REST API endpoints
   - WebSocket server for real-time updates

3. **Frontend** (`web/index.html`)
   - Real-time dashboard
   - Chart.js for visualizations
   - WebSocket client for live updates
   - Responsive design

### Data Flow

```
P2P Network Events
    ↓
Monitor Node (tracks events)
    ↓
Network State (in-memory)
    ↓
WebSocket Broadcast
    ↓
Web Dashboard (real-time updates)
```

## API Endpoints

### REST API

**GET `/api/state`**
- Returns complete network state
- Includes all nodes, connections, metrics
- Response: `NetworkState` JSON

**GET `/api/metrics`**
- Returns current metrics only
- Response: `NetworkMetrics` JSON

**GET `/api/nodes`**
- Returns list of all nodes
- Response: Array of `NodeInfo` JSON

### WebSocket

**WS `/ws`**
- Real-time updates
- Sends complete `NetworkState` JSON every second
- Auto-reconnects on disconnect

## Network State Structure

```rust
{
  "bootstrap_peer_id": "12D3KooW...",
  "nodes": {
    "12D3KooW...": {
      "peer_id": "12D3KooW...",
      "first_seen": 1234567890,
      "last_seen": 1234567890,
      "connection_count": 5,
      "addresses": ["/ip4/127.0.0.1/tcp/12345"],
      "agent": "simple-listener/1.0",
      "protocol": "ipfs/0.1.0"
    }
  },
  "connections": [
    {
      "timestamp": 1234567890,
      "event_type": "connected",
      "peer_id": "12D3KooW...",
      "direction": "inbound"
    }
  ],
  "metrics": {
    "total_nodes": 10,
    "active_connections": 8,
    "total_connections": 25,
    "dht_records": 5,
    "uptime_seconds": 3600,
    "messages_sent": 0,
    "messages_received": 0
  },
  "namespaces": {
    "test-room": 5,
    "chat-room": 3
  }
}
```

## Usage Scenarios

### Scenario 1: Monitor Network Growth

1. Start monitor:
   ```bash
   cargo run --release --bin monitor
   ```

2. Open dashboard: `http://localhost:8080`

3. Start peers in other terminals:
   ```bash
   # Terminal 2
   cargo run --release --bin listener -- --bootstrap /ip4/127.0.0.1/tcp/51820
   
   # Terminal 3
   cargo run --release --bin dialer -- --bootstrap /ip4/127.0.0.1/tcp/51820
   ```

4. Watch dashboard update in real-time as peers join!

### Scenario 2: Track Node Communications

1. Monitor shows all connection events
2. See which nodes connect/disconnect
3. Track connection direction (inbound/outbound)
4. View connection history per node

### Scenario 3: Namespace Analysis

1. Start peers in different namespaces
2. Dashboard shows namespace distribution
3. See how many nodes per namespace
4. Track namespace growth over time

## Dashboard Features

### Real-Time Updates
- WebSocket connection for live data
- Auto-reconnects if connection drops
- Updates every second

### Network Growth Chart
- Line chart showing node count over time
- Active connections over time
- Last 50 data points
- Smooth animations

### Node List
- All connected nodes
- Peer ID (truncated for display)
- Connection count
- First/last seen timestamps
- Agent and protocol info

### Connection Events
- Last 10 connection events
- Color-coded (green=connected, red=disconnected)
- Timestamp and direction
- Real-time updates

## Customization

### Modify Metrics

Edit `src/monitor.rs` to track additional metrics:

```rust
struct NetworkMetrics {
    // Add your custom metrics here
    custom_metric: u64,
}
```

### Add New API Endpoints

```rust
.route("/api/custom", get({
    let state = state.clone();
    move || async move {
        // Your custom logic
        Json(custom_data)
    }
}))
```

### Customize Dashboard

Edit `web/index.html` to:
- Change colors/styling
- Add new visualizations
- Modify layout
- Add new sections

## Performance Considerations

- **Memory**: Stores all connection history (consider limiting)
- **WebSocket**: Broadcasts to all connected clients
- **Updates**: 1 second interval (configurable)
- **Scalability**: Tested with 10-50 nodes (should scale higher)

## Troubleshooting

### Dashboard Not Loading

1. Check web server is running:
   ```bash
   curl http://localhost:8080/api/state
   ```

2. Verify `web/index.html` exists

3. Check browser console for errors

### No Real-Time Updates

1. Check WebSocket connection in browser DevTools
2. Verify WebSocket endpoint: `ws://localhost:8080/ws`
3. Check monitor logs for errors

### Missing Nodes

1. Ensure peers bootstrap to monitor node
2. Check that monitor is on same network
3. Verify firewall allows connections

## Future Enhancements

Potential additions:
- [ ] Message tracking (sent/received)
- [ ] Network topology visualization
- [ ] DHT routing table visualization
- [ ] Historical data storage
- [ ] Export metrics to CSV/JSON
- [ ] Alert system for anomalies
- [ ] Node filtering and search
- [ ] Performance metrics (latency, bandwidth)
- [ ] Multi-bootstrap node support
- [ ] Authentication for web dashboard

## Security Notes

⚠️ **Current Implementation:**
- No authentication on web dashboard
- All network data exposed via API
- WebSocket has no access control

**For Production:**
- Add authentication (JWT, basic auth)
- Restrict API access
- Use HTTPS/WSS
- Rate limit endpoints
- Sanitize node data

## Example Workflow

1. **Start Monitor:**
   ```bash
   cargo run --release --bin monitor --web-port 8080
   ```

2. **Open Dashboard:**
   ```
   http://localhost:8080
   ```

3. **Start Network:**
   ```bash
   # Terminal 2: Listener
   cargo run --release --bin listener -- --bootstrap /ip4/127.0.0.1/tcp/51820
   
   # Terminal 3: Another Listener
   cargo run --release --bin listener -- --bootstrap /ip4/127.0.0.1/tcp/51820
   
   # Terminal 4: Dialer
   cargo run --release --bin dialer -- --bootstrap /ip4/127.0.0.1/tcp/51820
   ```

3. **Watch Dashboard:**
   - See nodes appear in real-time
   - Watch connection events
   - Monitor network growth chart
   - Track namespaces

## Integration

The monitor can be integrated with:
- **Prometheus**: Export metrics
- **Grafana**: Advanced visualizations
- **ELK Stack**: Log aggregation
- **Custom Tools**: Via REST API

## Conclusion

The monitoring system provides comprehensive visibility into your Kademlia P2P network, making it easy to:
- Debug network issues
- Monitor network health
- Track network growth
- Analyze node behavior
- Understand network topology

Perfect for development, testing, and production monitoring!














