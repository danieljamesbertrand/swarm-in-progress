# Intensive 16-Node Kademlia Network Test

This test runs 16 P2P nodes (8 listeners + 8 dialers) plus 1 monitor node, all discovering and connecting to each other via the Kademlia DHT.

## Quick Start

### Windows
```powershell
.\start_intensive_16.ps1
```

### Linux/Mac
```bash
chmod +x start_intensive_16.sh
./start_intensive_16.sh
```

## What Happens

1. **Monitor starts** (Bootstrap node + Web Dashboard)
   - Listens on `0.0.0.0:51820`
   - Web dashboard at `http://localhost:8080`

2. **8 Listeners start** (staggered, 500ms apart)
   - Each connects to bootstrap
   - Bootstraps to DHT
   - Registers peer info in DHT
   - Queries DHT to discover other peers
   - Waits for connections

3. **8 Dialers start** (staggered, 500ms apart)
   - Each connects to bootstrap
   - Bootstraps to DHT
   - Actively queries DHT for peers
   - Connects to discovered peers

## Network Topology

```
Monitor (Bootstrap)
  ├── Listener 1 ──┐
  ├── Listener 2 ──┤
  ├── Listener 3 ──┤
  ├── Listener 4 ──┤
  ├── Listener 5 ──┤
  ├── Listener 6 ──┤
  ├── Listener 7 ──┤
  ├── Listener 8 ──┤
  ├── Dialer 1 ────┼── Discovers & connects to all listeners
  ├── Dialer 2 ────┤
  ├── Dialer 3 ────┤
  ├── Dialer 4 ────┤
  ├── Dialer 5 ────┤
  ├── Dialer 6 ────┤
  ├── Dialer 7 ────┤
  └── Dialer 8 ────┘
```

## Expected Timeline

- **0-5s**: Monitor starts, first nodes begin connecting
- **5-15s**: All nodes bootstrap to DHT
- **15-30s**: Listeners register in DHT, start discovering peers
- **30-60s**: Dialers discover and connect to listeners
- **60s+**: Full mesh network established

## What to Watch

### Monitor Window
- Connection events: `[MONITOR] ✓ Connection established`
- Should see 16 connections (one per node)

### Listener Windows
- Registration: `✓ Registered in DHT!`
- Peer discovery: `[VERBOSE] ✓ Found X peer(s) in DHT`
- Peer connections: `✓✓✓ Peer connected: <peer_id>`

### Dialer Windows
- Bootstrap: `✓ DHT bootstrapped!`
- Discovery: `[VERBOSE] ✓ Found X peer(s) in DHT`
- Connections to listeners

### Web Dashboard (http://localhost:8080)
- **Total Nodes**: Should reach 16
- **Active Connections**: Should show 16 (connections to monitor)
- **Connection Events**: Real-time log of all connections
- **Network Growth Chart**: Visual representation of network growth

## Key Features

### Enhanced Discovery
- **Listeners** now actively query DHT after registration
- **All nodes** can discover each other, not just dialers → listeners
- **Mesh network** forms as nodes discover and connect

### Intensive Testing
- **16 nodes** stress-tests the DHT
- **Rapid startup** (500ms intervals) tests concurrent connections
- **Full discovery** tests Kademlia's peer finding capabilities

## Troubleshooting

### No Connections Showing
- Wait 30-60 seconds for full discovery
- Check monitor window for connection events
- Verify all nodes are using same namespace (`intensive-test`)

### Dashboard Shows Fewer Nodes
- Nodes may still be bootstrapping
- Check individual node windows for errors
- Refresh dashboard (F5)

### Too Many Windows
- Consider minimizing some windows
- Monitor window is most important
- Dashboard shows overall status

## Performance Notes

- **CPU**: 16 nodes + monitor = moderate CPU usage
- **Memory**: ~50-100MB per node
- **Network**: All localhost, minimal bandwidth
- **Startup**: ~10-15 seconds for all nodes to start

## Stopping the Test

1. Close all PowerShell/Terminal windows, OR
2. Press Ctrl+C in each window, OR
3. Kill processes:
   ```powershell
   Get-Process | Where-Object {$_.ProcessName -like "*cargo*" -or $_.ProcessName -like "*monitor*" -or $_.ProcessName -like "*listener*" -or $_.ProcessName -like "*dialer*"} | Stop-Process -Force
   ```

## Next Steps

After running this test, you can:
- Modify node count in the script
- Change namespace to test isolation
- Add more dialers vs listeners
- Monitor network behavior over time
- Test network resilience (kill nodes, watch recovery)












