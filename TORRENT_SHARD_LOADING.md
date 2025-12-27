# Torrent-Based Shard Loading for Distributed Inference

## Overview

Each node in the distributed inference network acts as both:
1. **Torrent Server**: Seeds all GGUF shard files it has
2. **Torrent Client**: Downloads missing shards on-demand when requested for inference

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    Shard Listener Node                       │
│                                                               │
│  ┌─────────────────┐         ┌──────────────────┐          │
│  │  Torrent Server │         │  Torrent Client  │          │
│  │                 │         │                  │          │
│  │  Seeds all      │         │  Downloads      │          │
│  │  .gguf files    │         │  missing shards  │          │
│  │  in shards_dir  │         │  on-demand       │          │
│  └─────────────────┘         └──────────────────┘          │
│                                                               │
│  ┌──────────────────────────────────────────────────────┐   │
│  │  Loaded Shards Cache                                 │   │
│  │  shard_id -> PathBuf                                 │   │
│  └──────────────────────────────────────────────────────┘   │
│                                                               │
│  ┌──────────────────────────────────────────────────────┐   │
│  │  Command Handlers                                     │   │
│  │  - LOAD_SHARD: Load shard (local or torrent)         │   │
│  │  - LIST_FILES: List available GGUF files             │   │
│  │  - EXECUTE_TASK: Run inference (requires loaded)     │   │
│  └──────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
```

## Workflow

### 1. Node Startup
- Scans `models_cache/shards/` for all `.gguf` files
- Creates torrent metadata (info hash, piece hashes) for each file
- Registers file metadata in DHT
- Seeds all available GGUF files

### 2. On-Demand Shard Loading

When a node is asked to participate in inference for a shard it doesn't have:

```
Request: LOAD_SHARD { shard_id: 5 }
    ↓
Check: Is shard-5.gguf loaded?
    ↓ (No)
Check: Does shard-5.gguf exist locally?
    ↓ (No)
Query DHT: Find peers sharing shard-5.gguf
    ↓
Connect to peers via torrent protocol
    ↓
Download shard-5.gguf via torrent
    ↓
Verify file integrity
    ↓
Load shard into memory
    ↓
Response: { status: "loaded", path: "..." }
```

### 3. Inference Request

```
Request: EXECUTE_TASK { task_type: "ai_inference", ... }
    ↓
Check: Is assigned shard loaded?
    ↓ (No)
Auto-load: load_shard_file(shard_id)
    ↓ (Not found locally)
Error: "Shard not loaded. Use LOAD_SHARD command first."
    ↓ (or trigger automatic torrent download)
```

## Commands

### LOAD_SHARD
Request a node to load a specific shard for inference.

**Request:**
```json
{
  "command": "LOAD_SHARD",
  "request_id": "req-123",
  "from": "coordinator-peer-id",
  "to": "target-node-peer-id",
  "params": {
    "shard_id": 5
  }
}
```

**Response (Success):**
```json
{
  "command": "LOAD_SHARD",
  "request_id": "req-123",
  "status": "success",
  "result": {
    "shard_id": 5,
    "status": "loaded",
    "path": "models_cache/shards/shard-5.gguf"
  }
}
```

**Response (Needs Download):**
```json
{
  "command": "LOAD_SHARD",
  "request_id": "req-123",
  "status": "error",
  "error": "Shard 5 not found. Torrent download required."
}
```

### LIST_FILES
List all GGUF files available for torrent download from this node.

**Request:**
```json
{
  "command": "LIST_FILES",
  "request_id": "req-124",
  "from": "client-peer-id",
  "to": "node-peer-id"
}
```

**Response:**
```json
{
  "command": "LIST_FILES",
  "request_id": "req-124",
  "status": "success",
  "result": {
    "files": [
      {
        "info_hash": "abc123...",
        "filename": "shard-0.gguf",
        "size": 493000000
      },
      {
        "info_hash": "def456...",
        "filename": "shard-1.gguf",
        "size": 476000000
      }
    ]
  }
}
```

## Implementation Status

✅ **Completed:**
- Torrent server infrastructure (seeds GGUF files)
- Shard scanning and metadata creation
- LOAD_SHARD command handler
- LIST_FILES command handler
- Local shard loading
- Shard cache management

🔄 **In Progress:**
- Torrent client integration for downloading shards
- DHT-based peer discovery for shard files
- Automatic torrent download when shard not found

📋 **Next Steps:**
1. Implement torrent client to download shards from peers
2. Query DHT for peers sharing specific shard files
3. Integrate with pipeline coordinator to trigger downloads
4. Add retry logic and error handling for downloads
5. Verify downloaded file integrity before loading

## Usage

### Start a Shard Listener with Torrent Seeding

```bash
cargo run --bin shard_listener -- \
  --bootstrap /ip4/127.0.0.1/tcp/51820 \
  --cluster llama-cluster \
  --shard-id 0 \
  --total-shards 8 \
  --shards-dir models_cache/shards \
  --enable-torrent
```

This will:
- Scan `models_cache/shards/` for all `.gguf` files
- Seed all found files via torrent
- Announce shard 0 in DHT
- Accept LOAD_SHARD commands to load other shards

### Request a Node to Load a Shard

From the pipeline coordinator or another node:

```rust
let cmd = Command::new(commands::LOAD_SHARD, "coordinator", Some("target-node"))
    .with_param("shard_id", json!(5));

// Send command and wait for response
let response = send_command(cmd).await?;
```

## File Structure

```
models_cache/shards/
├── shard-0.gguf    (seeded, can be downloaded)
├── shard-1.gguf    (seeded, can be downloaded)
├── shard-2.gguf    (seeded, can be downloaded)
├── ...
└── shard-7.gguf    (seeded, can be downloaded)
```

Each node:
- Seeds all GGUF files it has in `shards_dir`
- Can download any missing shard from other nodes
- Loads shards on-demand when requested for inference




