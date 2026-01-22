# Deploy Shards to Rendezvous Server

This guide explains how to deploy shard files to `eagleoneonline.ca` and configure the rendezvous server to seed them.

## Overview

The rendezvous server can now act as a torrent seeder for shard files. This allows:
1. Centralized storage of shard files on the server
2. Automatic seeding to all connecting nodes
3. Clients download missing shards via torrent
4. Distributed inference begins once 4 nodes have all required shards

## Step 1: Prepare Shard Files Locally

If you have safetensors files, map them to GGUF naming:

```powershell
.\target\release\shard_loader.exe map `
    --metadata-dir "E:\rust\llamaModels\shards" `
    --safetensors-dir "E:\rust\llamaModels\shards_f16" `
    --target-dir "models_cache\shards"
```

This creates `shard-0.gguf`, `shard-1.gguf`, etc. in `models_cache/shards/`.

## Step 2: Upload Shards to Server

### Option A: Using PowerShell Script (Recommended)

```powershell
.\deploy_shards_to_server.ps1
```

### Option B: Using Upload Binary

```powershell
.\target\release\upload_shards.exe `
    --source-dir "models_cache\shards" `
    --remote-user "dbertrand" `
    --remote-host "eagleoneonline.ca" `
    --remote-dir "/home/dbertrand/punch-simple/shards"
```

### Option C: Manual SCP

```powershell
scp -F NUL models_cache\shards\shard-*.gguf dbertrand@eagleoneonline.ca:/home/dbertrand/punch-simple/shards/
```

## Step 3: Configure Rendezvous Server

SSH to the server and restart it with the seed directory:

```bash
ssh dbertrand@eagleoneonline.ca
cd ~/punch-simple

# Stop existing server
pkill -f "server --listen"

# Start server with torrent seeding
./target/release/server \
    --listen-addr 0.0.0.0 \
    --port 51820 \
    --transport quic \
    --seed-dir /home/dbertrand/punch-simple/shards
```

The server will:
- Scan the seed directory for `.gguf` and `.safetensors` files
- Create torrent metadata for each file
- Seed files to any node that requests them

## Step 4: How Clients Download Shards

### Automatic Flow:

1. **Shard nodes start** without shard files locally
2. **Nodes join DHT** and announce their shard IDs
3. **Coordinator discovers** nodes via DHT
4. **Coordinator sends LOAD_SHARD** commands to nodes missing shards
5. **Nodes download** from rendezvous server (or other peers) via torrent
6. **Nodes broadcast SHARD_LOADED** to update the swarm
7. **Once 4 nodes have all shards**, swarm becomes ready for inference

### Manual Trigger:

If a coordinator is running, it will automatically send `LOAD_SHARD` commands. You can also trigger manually by having nodes query the server:

```rust
// Nodes can query server for available files
LIST_FILES command → Server responds with available shard files
Nodes can then request pieces via REQUEST_PIECE
```

## Step 5: Verify Deployment

### On Server:

```bash
# Check files are present
ls -lh ~/punch-simple/shards/*.gguf

# Check server is seeding
# Look for "[TORRENT] Loaded X file(s) for sharing" in server logs
```

### On Client:

```powershell
# Validate shards are available
.\target\release\shard_loader.exe validate --target-dir "models_cache\shards" --expected-shards 4

# Check shard nodes can see files
# Look for "[TORRENT] Found file: shard-X.gguf" in shard node logs
```

## How It Works

### Torrent Protocol Flow:

1. **Client requests file list**: `LIST_FILES` → Server responds with available files
2. **Client requests metadata**: `REQUEST_METADATA { info_hash }` → Server sends file metadata
3. **Client requests pieces**: `REQUEST_PIECE { info_hash, piece_index }` → Server sends piece data
4. **Client assembles file**: Once all pieces downloaded, file is saved locally

### Shard Loading Flow:

1. Node starts without shard file
2. Node joins network and announces shard ID
3. Coordinator discovers node and checks if shard is loaded
4. If not loaded, coordinator sends `LOAD_SHARD { shard_id }` command
5. Node receives command and starts torrent download from server
6. Once downloaded, node loads shard and broadcasts `SHARD_LOADED`
7. Process repeats until all 4 nodes have all required shards

## Troubleshooting

### Server not seeding files:

- Check `--seed-dir` path is correct
- Verify files exist in the directory
- Check server logs for "[TORRENT] Loaded X file(s)" message
- Ensure files are `.gguf` or `.safetensors` format

### Clients can't download:

- Verify firewall allows connections (ufw should allow 170.203.207.66)
- Check server is running with `--seed-dir` argument
- Verify nodes can connect to server (check for connection logs)
- Check node logs for torrent download progress

### Shards not loading:

- Ensure coordinator is running to send LOAD_SHARD commands
- Check nodes are announcing to DHT
- Verify torrent download completes (check node logs)
- Validate downloaded files exist in `models_cache/shards/`

## Next Steps

Once shards are deployed and seeded:

1. **Start shard nodes** - They will connect to rendezvous server
2. **Start coordinator** - It will discover nodes and trigger shard loading
3. **Monitor progress** - Watch logs for shard downloads and loading
4. **Wait for swarm ready** - Once 4 nodes have all shards, inference can begin
