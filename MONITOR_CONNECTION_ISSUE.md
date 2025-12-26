# Why Active Connections Shows Zero

## The Problem

The monitor currently only tracks connections **TO the monitor node itself**, not connections between other peers in the network.

### Current Behavior

1. **Monitor acts as bootstrap node** - peers connect to it to join the network
2. **Monitor tracks connections to itself** - when a peer connects to the monitor
3. **Monitor doesn't see peer-to-peer connections** - when peers connect to each other

### Why This Happens

In a Kademlia DHT network:
- Peers connect to the bootstrap node initially
- Then peers discover each other via DHT
- Peers connect directly to each other (not through bootstrap)
- The bootstrap node (monitor) doesn't see these peer-to-peer connections

### Current Code Issue

```rust
// This only tracks connections TO the monitor
SwarmEvent::ConnectionEstablished { peer_id, .. } => {
    // Only fires when someone connects TO the monitor
    state.metrics.active_connections = state.nodes.len();
}
```

## Solutions

### Solution 1: Track Actual Connection Count (Current Fix)

I've updated the code to properly track connection count:
- Increment on `ConnectionEstablished`
- Decrement on `ConnectionClosed`
- This shows connections TO the monitor (bootstrap connections)

### Solution 2: Query DHT for All Peers

The monitor can query the DHT to discover all peers, but still won't see their connections to each other.

### Solution 3: Have Peers Report Connections

Modify peers to report their connections to the monitor via messages.

### Solution 4: Use Swarm Connection Tracking

Query the swarm's connection manager to get actual connection count.

## Current Status After Fix

The monitor now properly tracks:
- ✅ Connections TO the monitor (bootstrap connections)
- ✅ Active connection count (increments/decrements correctly)
- ❌ Connections BETWEEN peers (not visible to monitor)

## Expected Behavior

When you start the network:
1. **Listeners connect to monitor** → Monitor sees 2 connections
2. **Dialers connect to monitor** → Monitor sees 4 connections total
3. **Dialers connect to listeners** → Monitor doesn't see these (peer-to-peer)

So "Active Connections" shows connections to the monitor, not total network connections.

## To See All Connections

You would need to:
1. Have peers send connection reports to monitor
2. Or use a different monitoring architecture
3. Or query each peer's connection state

## Quick Fix Applied

I've fixed the connection counting logic so it properly increments/decrements. Now you should see connections when peers connect to the monitor.






