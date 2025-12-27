# Network Monitor - Quick Start

## Overview

The Network Monitor provides a comprehensive web-based dashboard to monitor your Kademlia P2P network in real-time.

## Quick Start

### 1. Start the Monitor

```bash
cargo run --release --bin monitor
```

This starts:
- **P2P Bootstrap Node** on `0.0.0.0:51820`
- **Web Dashboard** on `http://localhost:8080`

### 2. Open the Dashboard

Open your browser to:
```
http://localhost:8080
```

### 3. Start Peers

In other terminals, start peers that bootstrap to the monitor:

```bash
# Terminal 2
cargo run --release --bin listener -- --bootstrap /ip4/127.0.0.1/tcp/51820

# Terminal 3
cargo run --release --bin dialer -- --bootstrap /ip4/127.0.0.1/tcp/51820
```

### 4. Watch the Dashboard

You'll see in real-time:
- ✅ Nodes appearing as they connect
- 📊 Network metrics updating
- 📈 Growth chart showing network expansion
- 🔗 Connection events logged
- 📁 Namespaces being tracked

## Features

### Real-Time Metrics
- Total nodes in network
- Active connections
- Total connections (lifetime)
- DHT records
- System uptime

### Node Tracking
- All connected peers
- Peer IDs and details
- Connection history
- First/last seen timestamps

### Connection Events
- Real-time connection/disconnection logs
- Direction (inbound/outbound)
- Timestamped events

### Network Growth Chart
- Live line chart
- Tracks nodes over time
- Tracks connections over time

## Custom Ports

```bash
# Custom P2P port and web port
cargo run --release --bin monitor \
  --port 9000 \
  --web-port 3000
```

Then access: `http://localhost:3000`

## API Endpoints

- `GET /api/state` - Complete network state (JSON)
- `GET /api/metrics` - Current metrics (JSON)
- `GET /api/nodes` - List of all nodes (JSON)
- `WS /ws` - WebSocket for real-time updates

## Example: Full Testbed

```bash
# Terminal 1: Monitor
cargo run --release --bin monitor

# Terminal 2: Listener 1
cargo run --release --bin listener -- --namespace room1

# Terminal 3: Listener 2
cargo run --release --bin listener -- --namespace room1

# Terminal 4: Dialer
cargo run --release --bin dialer -- --namespace room1
```

Open `http://localhost:8080` and watch all 3 peers appear!

## Troubleshooting

**Dashboard not loading?**
- Check monitor is running: `curl http://localhost:8080/api/state`
- Verify `web/index.html` exists
- Check browser console for errors

**No nodes showing?**
- Ensure peers bootstrap to monitor: `--bootstrap /ip4/127.0.0.1/tcp/51820`
- Wait 10-30 seconds for DHT to populate
- Check monitor logs for connection events

**WebSocket not connecting?**
- Check browser DevTools → Network → WS
- Verify WebSocket URL: `ws://localhost:8080/ws`
- Check firewall allows port 8080

## Next Steps

See `MONITORING_GUIDE.md` for:
- Detailed architecture
- API documentation
- Customization options
- Advanced features













