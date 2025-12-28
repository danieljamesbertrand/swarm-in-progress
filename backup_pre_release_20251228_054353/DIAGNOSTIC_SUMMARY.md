# Missing Node Diagnostic Summary

## Current Status
- **Expected**: 4 shard nodes (shards 0, 1, 2, 3)
- **Running**: Check with `Get-Process | Where-Object {$_.ProcessName -eq "shard_listener"}`

## How to Identify Missing Shard

### Method 1: Check Web Server Console
Look in the web server PowerShell window for:
- `[COORDINATOR] ✗ Failed to spawn node for shard X` - Shows which shard failed
- `[COORDINATOR] ⚠️  Shard X node did not come online in time` - Shows timeout
- `Missing shard IDs: [X]` - Shows which shard is still missing
- `[COORDINATOR] ⚠️  Summary: X nodes spawned successfully, Y failed (shards: [X])`

### Method 2: Check Web Interface
1. Open http://localhost:8080
2. Look at the pipeline visualization
3. Shard stages showing red/error = missing shard
4. "Nodes Online" counter shows X/4

### Method 3: Check Process List
```powershell
Get-Process | Where-Object {$_.ProcessName -eq "shard_listener"}
```
Should show 4 processes if all are running.

## Common Causes

1. **Still Compiling** (Most Common)
   - First run: 30-60 seconds per node
   - Subsequent runs: 5-10 seconds
   - Check web server console for compilation progress

2. **Spawn Failure**
   - Check for: `[SPAWNER] Failed to spawn node for shard X`
   - Possible causes:
     - Cargo not in PATH
     - Compilation error
     - Process limit reached

3. **Node Crashed**
   - Node spawned but exited immediately
   - Check for error messages in web server console
   - Check Windows Event Viewer for crashes

4. **DHT Discovery Timeout**
   - Node running but not discovered
   - Check bootstrap server is running
   - Check network connectivity

## Enhanced Diagnostics

The updated code now provides:
- Detailed spawn success/failure per shard
- Online status per shard
- Summary of which shards are missing
- Possible reasons for missing shards

## Next Steps

1. **Wait 60-90 seconds** for first-time compilation
2. **Check web server console** for detailed diagnostics
3. **Check web interface** at http://localhost:8080
4. **If still missing after 90s**, check console for specific error messages


