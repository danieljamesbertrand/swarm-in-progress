# System Start Status

## What's Running

The restart script is executing and will:

1. **Kill all processes** - Clean slate
2. **Check shard files** - Verify shard-0.gguf through shard-3.gguf exist
3. **Start bootstrap** - DHT bootstrap server on port 51820
4. **Start web server** - HTTP on 8080, WebSocket on 8081
5. **Start 4 shard nodes** - One for each shard (0, 1, 2, 3)
6. **Monitor** - Watch for errors and status updates

## Terminal Windows

You should see these windows opening:

1. **Bootstrap Server** (minimized) - DHT bootstrap
2. **Web Server** (normal) - Watch for errors and startup messages
3. **Shard Node 0** (normal) - Should show shard loading
4. **Shard Node 1** (normal) - Should show shard loading
5. **Shard Node 2** (normal) - Should show shard loading
6. **Shard Node 3** (normal) - Should show shard loading

## What to Watch For

### ✅ Success Messages:

**Shard Nodes:**
- `[SHARD] ✓✓✓ SHARD X LOADED BEFORE JOINING NETWORK ✓✓✓`
- `[DHT] ✓✓✓ ANNOUNCED SHARD X TO DHT ✓✓✓`

**Web Server:**
- `Web Console: http://localhost:8080`
- `[DHT] Discovered shard X from ...`
- `[INFERENCE] Pipeline status: 4/4 nodes online`

### ❌ Error Messages:

**Watch for:**
- `[ERROR]` or `[error]` messages
- `panic` messages
- `Connection refused`
- `Failed to load`

## Timeline

- **0-30 seconds**: Processes starting, cargo compiling (first time)
- **30-60 seconds**: Web server and nodes compiling/starting
- **60-90 seconds**: Nodes connecting to bootstrap, DHT discovery
- **90-120 seconds**: All nodes registered, pipeline complete

## Next Steps

Once you see "4/4 nodes online" in web server terminal:

1. Open browser: http://localhost:8080
2. Wait 10 seconds for UI to load
3. Type query: `what do a cat and a snake have in common`
4. Click Send
5. Watch for results!

## Check Status

Run this to check what's running:
```powershell
Get-Process | Where-Object {$_.ProcessName -match "server|web_server|shard_listener"}
```

Check web server log:
```powershell
Get-Content web_server_monitor.log -Tail 20
```

