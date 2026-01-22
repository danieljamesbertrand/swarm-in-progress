# Node Internal State Monitoring Guide

## Overview

This guide explains how to monitor and probe the internal state of nodes and the rendezvous server to understand what's happening internally and diagnose issues.

---

## Monitoring Tools

### 1. PowerShell Monitor (`monitor_node_internals.ps1`)

**Basic monitoring via HTTP diagnostics endpoints**

**Usage:**
```powershell
.\monitor_node_internals.ps1
```

**What it does:**
- Queries rendezvous server diagnostics endpoint (`http://eagleoneonline.ca:51821/diagnostics`)
- Shows connection statistics (total, active, failed)
- Displays recent connection events
- Checks for running node processes
- Updates every 5 seconds

**Features:**
- ✅ Lightweight (PowerShell only)
- ✅ No compilation needed
- ✅ Shows rendezvous server state
- ❌ Cannot directly query node internal state
- ❌ Limited to HTTP endpoints

---

### 2. Rust Node Monitor (`node_monitor.rs`)

**Deep internal state probing via P2P**

**Usage:**
```powershell
cargo run --bin node_monitor
```

**What it does:**
- Connects to rendezvous server via P2P
- Queries DHT for all shard announcements (shards 0-7)
- Extracts peer IDs and shard loading status
- Displays real-time swarm readiness analysis
- Updates every 5-10 seconds

**Features:**
- ✅ Direct P2P connection to nodes
- ✅ Queries DHT for actual node state
- ✅ Shows shard loading status per node
- ✅ Identifies which shards are missing/not loaded
- ✅ Real-time swarm readiness analysis
- ❌ Requires compilation
- ❌ More complex setup

---

## What Gets Monitored

### Rendezvous Server State

**Connection Statistics:**
- Total connections attempted
- Active connections
- Failed connections
- Handshake timeouts

**Recent Events:**
- Connection attempts
- Connection established
- Connection closed
- Errors and timeouts

**Access via:**
- HTTP: `http://eagleoneonline.ca:51821/diagnostics`
- PowerShell monitor queries this automatically

---

### Node Internal State

**Per-Node Information:**
- Peer ID
- Shard ID (0-7)
- Shard loaded status (`shard_loaded = true/false`)
- DHT announcement status

**Swarm Readiness:**
- Discovered shards count (X / 8)
- Missing shards list
- Shards not loaded list
- Overall swarm ready status

**Access via:**
- Rust monitor queries DHT directly
- Extracts from shard announcements

---

## Understanding the Output

### PowerShell Monitor Output

```
========================================
  NODE INTERNAL STATE MONITOR
========================================

Rendezvous Server: eagleoneonline.ca:51820
Diagnostics Port: 51821
Update Interval: 5 seconds

[Iteration 1] Probing internal state...

Rendezvous Server Diagnostics:
  Connections:
    Total: 15
    Active: 8
    Failed: 2
  Recent Events (last 5):
    [14:23:45] ConnectionEstablished (peer: 12D3KooW...)
    [14:23:44] ConnectionAttempt (peer: 12D3KooW...)
    ...

Node Processes:
  [OK] Found 8 node-related process(es)

Swarm Readiness Analysis:
    [OK] 8 active connections (expecting 8+)
```

---

### Rust Monitor Output

```
═══════════════════════════════════════════════════════════════
  STATUS REPORT - 14:25:30
═══════════════════════════════════════════════════════════════

Discovered Nodes: 8/8

  Shard Status:
    Shard 0: ✓ [LOADED] (peer: 12D3KooW...)
    Shard 1: ✓ [LOADED] (peer: 12D3KooW...)
    Shard 2: ✓ [LOADED] (peer: 12D3KooW...)
    Shard 3: ✓ [LOADED] (peer: 12D3KooW...)
    Shard 4: ✓ [LOADED] (peer: 12D3KooW...)
    Shard 5: ✓ [LOADED] (peer: 12D3KooW...)
    Shard 6: ✓ [LOADED] (peer: 12D3KooW...)
    Shard 7: ✓ [LOADED] (peer: 12D3KooW...)

  [SWARM] ✓✓✓ SWARM READY FOR INFERENCE ✓✓✓
```

**Or if not ready:**

```
  Shard Status:
    Shard 0: ✓ [LOADED] (peer: 12D3KooW...)
    Shard 1: ✗ [NOT LOADED] (peer: 12D3KooW...)
    Shard 2: ✓ [LOADED] (peer: 12D3KooW...)
    Shard 3: [MISSING]
    ...

  [SWARM] ⚠️  Swarm not ready:
    - Missing 1 shard(s)
    - Shard(s) not loaded: [1]
```

---

## Diagnosing Issues

### Issue: Swarm Not Ready

**Check 1: Are all nodes discovered?**
- Look for: `Discovered Nodes: 8/8`
- If less than 8: Nodes may still be starting or connection issues

**Check 2: Are all shards loaded?**
- Look for: `Shard X: ✓ [LOADED]` for all 0-7
- If any show `✗ [NOT LOADED]`: That node doesn't have its shard file
- If any show `[MISSING]`: Node not discovered yet

**Check 3: Rendezvous server connections**
- Look for: `Active: 8` connections
- If less: Some nodes may not be connected

---

### Issue: Nodes Not Connecting

**Check rendezvous server diagnostics:**
- `Failed connections` count
- Recent error events
- Connection timeout messages

**Check node processes:**
- Are 8 node processes running?
- Check node windows for connection errors

---

### Issue: Shards Not Loading

**Check shard status:**
- Which shards show `✗ [NOT LOADED]`?
- Check those node windows for: `[SHARD] ASSIGNED SHARD X NOT FOUND`
- Verify shard files exist: `Test-Path "models_cache\shards\shard-X.gguf"`

**Solution:**
- Copy missing shard files to `models_cache\shards\`
- Restart affected nodes

---

## Advanced Usage

### Custom Query Interval

**PowerShell:**
```powershell
.\monitor_node_internals.ps1 -IntervalSeconds 10
```

**Rust:**
Edit `src/bin/node_monitor.rs`:
```rust
let mut query_interval = time::interval(Duration::from_secs(10)); // Change 5 to 10
```

---

### Query Specific Rendezvous Server

**PowerShell:**
```powershell
.\monitor_node_internals.ps1 -RendezvousHost "192.168.1.100" -DiagnosticsPort 51821
```

**Rust:**
```powershell
cargo run --bin node_monitor -- 192.168.1.100:51820
```

---

### Continuous vs Single Run

**PowerShell:**
```powershell
# Continuous (default)
.\monitor_node_internals.ps1

# Single run
.\monitor_node_internals.ps1 -Continuous:$false
```

---

## Integration with Other Tools

### Combine with Node Windows

**Best practice:**
1. Run monitor in one window
2. Watch node windows for detailed logs
3. Cross-reference monitor output with node logs

**Example:**
- Monitor shows: `Shard 3: ✗ [NOT LOADED]`
- Check node 3 window: Look for `[SHARD] ASSIGNED SHARD 3 NOT FOUND`
- Verify file exists: `Test-Path "models_cache\shards\shard-3.gguf"`

---

### Combine with Diagnostics Dashboard

**Rendezvous server web dashboard:**
- URL: `http://eagleoneonline.ca:51821/`
- Shows real-time connection statistics
- Displays recent events and errors

**Use monitor to:**
- Get programmatic access to diagnostics
- Automate checks
- Log historical data

---

## Troubleshooting Monitor Itself

### PowerShell Monitor Not Connecting

**Error: "Failed to query rendezvous diagnostics"**

**Check:**
1. Is rendezvous server running?
2. Is diagnostics port (51821) accessible?
3. Firewall blocking HTTP connections?

**Test manually:**
```powershell
Invoke-RestMethod -Uri "http://eagleoneonline.ca:51821/diagnostics"
```

---

### Rust Monitor Not Discovering Nodes

**Error: "No nodes discovered yet"**

**Check:**
1. Are nodes actually running?
2. Are nodes connected to rendezvous server?
3. Have nodes announced to DHT? (check node windows)

**Wait:**
- DHT queries take 5-10 seconds
- Nodes need time to announce (5-10 seconds after startup)

---

## Summary

**Use PowerShell monitor for:**
- Quick checks
- Rendezvous server state
- No compilation needed

**Use Rust monitor for:**
- Deep node state analysis
- Swarm readiness diagnosis
- Real-time shard loading status

**Both tools together:**
- Comprehensive monitoring
- Full system visibility
- Faster issue diagnosis

---

## Next Steps

1. **Start monitoring:**
   ```powershell
   .\monitor_node_internals.ps1
   ```

2. **Watch for issues:**
   - Missing shards
   - Shards not loaded
   - Connection failures

3. **Take action:**
   - Fix missing files
   - Restart nodes
   - Check connections

4. **Verify fix:**
   - Monitor should show all green
   - Swarm ready message appears
