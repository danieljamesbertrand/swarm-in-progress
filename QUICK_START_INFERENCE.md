# Quick Start: Inference Test

## Automatic Setup

Run this command to start everything and wait for the web server:

```powershell
powershell -ExecutionPolicy Bypass -File wait_and_open_web_server.ps1
```

This script will:
1. ✅ Wait for cargo compilation to complete
2. ✅ Wait for web server to start listening
3. ✅ Test HTTP connection
4. ✅ Open browser automatically when ready

## Manual Setup

If you prefer to start manually:

### 1. Start Bootstrap Server
```powershell
cargo run --bin server -- --listen-addr 0.0.0.0 --port 51820
```

### 2. Start Shard Node (in new terminal)
```powershell
$env:LLAMA_SHARD_ID="0"
cargo run --bin shard_listener -- --bootstrap /ip4/127.0.0.1/tcp/51820 --cluster llama-cluster --shard-id 0 --total-shards 4 --total-layers 32 --model-name llama-8b --port 51821 --shards-dir models_cache/shards
```

### 3. Start Web Server (in new terminal)
```powershell
$env:BOOTSTRAP="/ip4/127.0.0.1/tcp/51820"
cargo run --bin web_server
```

### 4. Wait and Open Browser
- Wait for "Web Console: http://localhost:8080" message
- Open http://localhost:8080 in browser

## Testing Inference

1. **Wait 10-15 seconds** for shard node to register
2. **Type query**: `what do a cat and a snake have in common`
3. **Click Send** or press Enter
4. **Watch for results** in the response area

## Success Indicators

### In Browser:
- Pipeline status shows 1/4 nodes online
- Response appears below query input

### In Terminal Logs:
- `[P2P] [OK] Matched response to waiting channel` ← Fix confirmed!
- `[RESPONSE] [OK] Response sent successfully`
- `[INFERENCE] [OK] Shard 0 completed`

## Troubleshooting

**Connection Refused:**
- Web server is still compiling (wait 1-2 minutes)
- Check web server terminal for compilation progress

**No Response:**
- Check shard node is running and registered
- Check terminal logs for error messages
- Verify shard-0.gguf exists in models_cache/shards/

**Port Already in Use:**
- Kill process using port 8080: `netstat -ano | findstr :8080`
- Or use different port in web_server code

