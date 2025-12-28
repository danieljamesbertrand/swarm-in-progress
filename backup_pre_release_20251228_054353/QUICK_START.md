# Quick Start Guide

## Start Everything

Run this command:
```powershell
powershell -ExecutionPolicy Bypass -File restart_and_monitor.ps1
```

This will:
1. ✅ Kill all existing processes
2. ✅ Check shard files
3. ✅ Start bootstrap server
4. ✅ Start web server (with monitoring)
5. ✅ Start all 4 shard nodes
6. ✅ Monitor startup and show status

## What Happens

### Bootstrap Server
- Starts on port 51820
- Acts as DHT bootstrap node

### Web Server
- Starts on port 8080 (HTTP) and 8081 (WebSocket)
- Connects to bootstrap
- Discovers shard nodes via DHT
- Logs to `web_server_monitor.log`

### Shard Nodes (4 total)
- Each node handles one shard (0, 1, 2, 3)
- Automatically tries to load `shard-{id}.gguf` on startup
- Joins DHT and announces shard availability
- Waits for inference requests

## Expected Messages

### Shard Nodes Should Show:
```
[SHARD] ✓✓✓ SHARD 0 LOADED BEFORE JOINING NETWORK ✓✓✓
[DHT] ✓✓✓ ANNOUNCED SHARD 0 TO DHT ✓✓✓
```

### Web Server Should Show:
```
[DHT] Discovered shard 0 from ...
[INFERENCE] Pipeline status: 4/4 nodes online
```

## Test Inference

1. Open: http://localhost:8080
2. Wait for "4/4 nodes online"
3. Type: `what do a cat and a snake have in common`
4. Click Send
5. Watch for results!

## Troubleshooting

**If nodes don't load shards:**
- Check `models_cache/shards/` for shard files
- Look for `[SHARD] SHARD X LOADED` messages
- If missing, nodes will download via torrent

**If web server doesn't start:**
- Wait 1-2 minutes for compilation
- Check web server terminal for errors
- Check `web_server_monitor.log` file

**If nodes don't appear:**
- Wait 10-20 seconds for DHT discovery
- Check bootstrap is running
- Check node terminals for DHT messages

