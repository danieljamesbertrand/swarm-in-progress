# System Monitoring Guide

## What to Monitor

### Web Server Terminal
Watch for these key messages:

**✅ Success Indicators:**
- `[SERVER] Inference engine initialized`
- `[DHT] ✓ Started Kademlia bootstrap`
- `[P2P] ✓ Matched response to waiting channel`
- `[INFERENCE] ✓ Shard X completed`

**❌ Error Indicators:**
- `[ERROR]` or `[error]` messages
- `[P2P] ⚠️  No waiting channel found`
- `[INFERENCE] ❌` messages
- `panic` messages
- Connection refused errors

### Shard Node Terminals
Watch for tensor/model loading:

**✅ Success Indicators:**
- `Loading model shard X`
- `Tensor loaded` or similar messages
- `[DHT] ✓✓✓ ANNOUNCED SHARD X TO DHT ✓✓✓`
- `[RESPONSE] ✓ Response sent successfully`

**❌ Error Indicators:**
- `Failed to load model`
- `File not found`
- `Out of memory`
- `panic` messages

## Key Messages to Look For

### Tensor Loading
- `Loading tensors...`
- `Model loaded successfully`
- `Shard X loaded`
- `Tensor shape: ...`

### Pipeline Status
- `Pipeline: X/4 shards online`
- `Pipeline is complete`
- `Missing shards: [...]`

### Inference Processing
- `[INFERENCE] Processing inference request...`
- `[INFERENCE] Sending JSON command to node`
- `[INFERENCE] Received JSON response from node`
- `[INFERENCE] ✓ Shard X completed`

## Common Issues

### Nodes Not Loading Tensors
**Symptoms:**
- Nodes start but don't load models
- No "Loading model" messages

**Check:**
1. Shard files exist in `models_cache/shards/`
2. Files are not corrupted
3. Sufficient memory available
4. Check node terminal for errors

### Web Server Not Responding
**Symptoms:**
- Connection refused
- Port 8080 not listening

**Check:**
1. Web server terminal for compilation errors
2. Port 8080 not in use by another process
3. Wait for compilation to complete (1-2 minutes)

### Nodes Not Registering
**Symptoms:**
- Web UI shows 0/4 or 1/4 nodes
- Nodes running but not discovered

**Check:**
1. Bootstrap server is running
2. Nodes connected to bootstrap
3. DHT discovery completed (wait 10-20 seconds)
4. Check node terminals for DHT messages

## Monitoring Commands

### Check Processes
```powershell
Get-Process | Where-Object {$_.ProcessName -match "web_server|shard_listener|server"}
```

### Check Ports
```powershell
netstat -ano | findstr ":8080"
netstat -ano | findstr ":51820"
```

### Check Log File
```powershell
Get-Content web_server_monitor.log -Tail 50
```

### Check for Errors
```powershell
Get-Content web_server_monitor.log | Select-String -Pattern "error|ERROR|failed"
```

## Expected Startup Sequence

1. **Bootstrap Server** (5 seconds)
   - Starts listening on port 51820

2. **Web Server** (30-60 seconds)
   - Compiles (first time)
   - Starts HTTP server on port 8080
   - Connects to bootstrap
   - Starts DHT discovery

3. **Shard Nodes** (30-60 seconds each)
   - Compile (first time)
   - Connect to bootstrap
   - Load model tensors (if files exist)
   - Announce to DHT
   - Register with pipeline

4. **Pipeline Ready** (10-20 seconds after nodes start)
   - All nodes discovered via DHT
   - Pipeline shows 4/4 nodes online
   - Ready for inference

## Total Startup Time

- **First Run**: 2-3 minutes (compilation)
- **Subsequent Runs**: 30-60 seconds (no compilation)
