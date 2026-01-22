# How Shards Are Uploaded and Distributed

## Overview

The system uses a **two-phase approach**:
1. **Upload**: Shards are uploaded to the rendezvous server via SCP
2. **Distribution**: The server seeds shards via a torrent-like protocol, and nodes download them on-demand

---

## Phase 1: Upload to Rendezvous Server

### Upload Method

The `copy_8_shards_to_rendezvous.ps1` script uploads shards using **SCP (Secure Copy Protocol)**:

```powershell
# For each shard (shard-v2-0.gguf through shard-v2-7.gguf):
scp -F NUL 'shard-v2-X.gguf' dbertrand@eagleoneonline.ca:/home/dbertrand/punch-simple/shards/shard-X.gguf
```

**What happens:**
1. Script finds all `shard-v2-*.gguf` files locally
2. For each file, it uses SCP to copy it to the remote server
3. Files are renamed on the server to standard names (`shard-0.gguf`, `shard-1.gguf`, etc.)
4. All 8 shards are uploaded sequentially (one at a time)

**Upload Details:**
- **Protocol**: SCP (SSH-based file transfer)
- **Destination**: `eagleoneonline.ca:/home/dbertrand/punch-simple/shards/`
- **Naming**: Local `shard-v2-X.gguf` → Remote `shard-X.gguf`
- **Size**: ~0.51 GB per shard (521 MB each, last one is 456 MB)
- **Total**: ~4.07 GB for all 8 shards

---

## Phase 2: Distribution to Nodes

### Server-Side: Torrent Seeding

Once uploaded, the rendezvous server acts as a **torrent seeder**:

```bash
# Server is started with:
./target/release/server \
    --listen-addr 0.0.0.0 \
    --port 51820 \
    --transport quic \
    --seed-dir /home/dbertrand/punch-simple/shards
```

**What the server does:**
1. **Scans seed directory** for `.gguf` files on startup
2. **Creates torrent metadata** for each file:
   - Calculates info hash (SHA256 of filename + size)
   - Splits file into 64 KB pieces
   - Calculates SHA256 hash for each piece
3. **Registers files** in the torrent system
4. **Seeds files** to any node that requests them

**Server Capabilities:**
- Responds to `LIST_FILES` command with available shard files
- Responds to `REQUEST_METADATA` with torrent metadata
- Responds to `REQUEST_PIECE` with file piece data
- Can serve multiple nodes simultaneously

---

### Client-Side: Torrent Download

Nodes download shards using a **torrent-like protocol** over libp2p:

#### Automatic Flow (Coordinator-Driven):

```
1. Node starts without shard files locally
   ↓
2. Node joins DHT (Kademlia) and announces shard ID
   ↓
3. Coordinator discovers node via DHT
   ↓
4. Coordinator checks if node has required shard loaded
   ↓
5. If missing, coordinator sends LOAD_SHARD { shard_id } command
   ↓
6. Node receives LOAD_SHARD command
   ↓
7. Node queries rendezvous server: LIST_FILES
   ↓
8. Server responds with available files (including shard-X.gguf)
   ↓
9. Node requests metadata: REQUEST_METADATA { info_hash }
   ↓
10. Server sends torrent metadata (file size, piece hashes, etc.)
    ↓
11. Node requests pieces: REQUEST_PIECE { info_hash, piece_index }
    ↓
12. Server sends piece data (64 KB chunks)
    ↓
13. Node assembles file from pieces
    ↓
14. Node verifies file integrity using piece hashes
    ↓
15. Node saves file to models_cache/shards/shard-X.gguf
    ↓
16. Node loads shard into memory
    ↓
17. Node broadcasts SHARD_LOADED to update swarm status
    ↓
18. Process repeats until all nodes have required shards
    ↓
19. Once 8 nodes have all shards, swarm becomes ready for inference
```

#### Torrent Protocol Messages:

1. **LIST_FILES**
   - Client → Server: Request list of available files
   - Server → Client: Returns `FileList { files: Vec<TorrentFileInfo> }`
   - Each file info contains: `info_hash`, `filename`, `size`

2. **REQUEST_METADATA**
   - Client → Server: `REQUEST_METADATA { info_hash }`
   - Server → Client: `Metadata { metadata: TorrentMetadata }`
   - Metadata contains: file size, piece size, piece hashes

3. **REQUEST_PIECE**
   - Client → Server: `REQUEST_PIECE { info_hash, piece_index }`
   - Server → Client: `PieceData { info_hash, piece_index, data: Vec<u8> }`
   - Data is 64 KB piece of the file

4. **File Assembly**
   - Client receives all pieces
   - Verifies each piece using SHA256 hash
   - Assembles pieces in order
   - Saves complete file to disk

---

## Distribution Architecture

### Centralized Initial Upload
```
Your Computer
    ↓ (SCP upload)
Rendezvous Server (eagleoneonline.ca)
    /home/dbertrand/punch-simple/shards/
    ├── shard-0.gguf (521 MB)
    ├── shard-1.gguf (521 MB)
    ├── ...
    └── shard-7.gguf (456 MB)
```

### Decentralized Distribution
```
Rendezvous Server (Seeder)
    ↓ (Torrent protocol)
    ├──→ Node 1 (downloads shard-0.gguf)
    ├──→ Node 2 (downloads shard-1.gguf)
    ├──→ Node 3 (downloads shard-2.gguf)
    └──→ ... (up to 8 nodes)

Once nodes have shards, they can also seed to other nodes:
Node 1 (has shard-0) ←→ Node 2 (has shard-1) [peer-to-peer sharing]
```

---

## Key Features

### 1. On-Demand Download
- Nodes don't need shards at startup
- Shards are downloaded only when needed (via LOAD_SHARD command)
- Reduces initial storage requirements

### 2. Piece-Based Transfer
- Files are split into 64 KB pieces
- Each piece is verified using SHA256 hash
- Allows resumable downloads
- Enables parallel piece requests

### 3. DHT Discovery
- Torrent metadata is registered in Kademlia DHT
- Nodes can discover available files without querying server directly
- Enables peer-to-peer file sharing between nodes

### 4. Coordinator Orchestration
- Coordinator monitors which nodes have which shards
- Automatically triggers downloads for missing shards
- Ensures all required nodes have shards before inference

### 5. Integrity Verification
- Each piece is hashed (SHA256)
- Complete file is verified before loading
- Prevents corrupted shard files

---

## Example: Uploading 8 Shards

```powershell
# Run the upload script
.\copy_8_shards_to_rendezvous.ps1

# What happens:
# 1. Finds shard-v2-0.gguf through shard-v2-7.gguf locally
# 2. Uploads each via SCP:
#    scp shard-v2-0.gguf → server:/shards/shard-0.gguf
#    scp shard-v2-1.gguf → server:/shards/shard-1.gguf
#    ... (8 files total)
# 3. All files uploaded to server
# 4. Server can now seed them
```

---

## Example: Node Downloading a Shard

```
Node receives: LOAD_SHARD { shard_id: 3 }

1. Node checks: Does shard-3.gguf exist locally? → No
2. Node connects to rendezvous server
3. Node sends: LIST_FILES
4. Server responds: [shard-0.gguf, shard-1.gguf, ..., shard-7.gguf]
5. Node finds shard-3.gguf in list (info_hash: abc123...)
6. Node sends: REQUEST_METADATA { info_hash: "abc123..." }
7. Server responds: { file_size: 521MB, piece_size: 64KB, pieces: [...] }
8. Node calculates: 521MB / 64KB = ~8,192 pieces needed
9. Node requests pieces: REQUEST_PIECE { info_hash, piece_index: 0 }
10. Server sends: PieceData { piece_index: 0, data: [64KB] }
11. Node verifies piece hash, stores piece
12. Repeat for pieces 1, 2, 3, ... 8,191
13. Node assembles all pieces → shard-3.gguf
14. Node verifies complete file
15. Node saves: models_cache/shards/shard-3.gguf
16. Node loads shard into memory
17. Node responds: SHARD_LOADED { shard_id: 3 }
```

---

## Summary

**Upload:**
- ✅ **Method**: SCP (SSH-based file transfer)
- ✅ **Destination**: Rendezvous server at `eagleoneonline.ca`
- ✅ **Files**: 8 shards (~4.07 GB total)
- ✅ **Naming**: `shard-v2-X.gguf` (local) → `shard-X.gguf` (server)

**Distribution:**
- ✅ **Method**: Torrent-like protocol over libp2p
- ✅ **Server Role**: Torrent seeder (serves pieces on demand)
- ✅ **Client Role**: Downloads pieces, assembles file
- ✅ **Orchestration**: Coordinator triggers downloads automatically
- ✅ **Discovery**: DHT for finding available files
- ✅ **Verification**: SHA256 hashes for integrity

**Result:**
- All 8 shards available on rendezvous server
- Nodes download shards on-demand via torrent protocol
- Once 8 nodes have all shards, distributed inference can begin
