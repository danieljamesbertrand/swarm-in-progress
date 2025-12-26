# Torrent File Server Guide

## Overview

Each node in the network can now act as a **torrent file server**, sharing files via a BitTorrent-like protocol over libp2p. Files are distributed through the DHT network, allowing peers to discover and download files from each other.

## Architecture

### Every Node is a Torrent Server

- **DHT Node**: Stores file metadata and routes queries
- **Torrent Server**: Serves file pieces to requesting peers
- **Torrent Client**: Downloads files from other peers

### How It Works

```
Node A (Torrent Server)
  ├── Shares files in ./shared/
  ├── Registers file metadata in DHT
  └── Serves file pieces to peers

Node B (Torrent Client)
  ├── Queries DHT for available files
  ├── Requests file metadata
  └── Downloads file pieces from Node A
```

## Components

### 1. Torrent Server (`torrent_server`)

Serves files from a directory to other peers.

**Usage:**
```bash
cargo run --release --bin torrent_server \
  --bootstrap /ip4/127.0.0.1/tcp/51820 \
  --share-dir ./shared \
  --port 0
```

**Arguments:**
- `--bootstrap`: Bootstrap node address (default: `/ip4/127.0.0.1/tcp/51820`)
- `--share-dir`: Directory containing files to share (default: `./shared`)
- `--port`: Listen port (0 = random, default: `0`)

**What it does:**
1. Scans `--share-dir` for files
2. Creates torrent metadata for each file (info hash, piece hashes)
3. Registers file metadata in DHT
4. Serves file pieces when requested by peers

### 2. Torrent Client (`torrent_client`)

Downloads files from other peers.

**Usage:**
```bash
# List available files
cargo run --release --bin torrent_client \
  --bootstrap /ip4/127.0.0.1/tcp/51820 \
  --download-dir ./downloads

# Download specific file
cargo run --release --bin torrent_client \
  --bootstrap /ip4/127.0.0.1/tcp/51820 \
  --download-dir ./downloads \
  --info-hash <file-info-hash>
```

**Arguments:**
- `--bootstrap`: Bootstrap node address (default: `/ip4/127.0.0.1/tcp/51820`)
- `--download-dir`: Directory to save downloaded files (default: `./downloads`)
- `--info-hash`: Info hash of file to download (optional - lists files if not provided)

**What it does:**
1. Connects to DHT network
2. Queries DHT for available files
3. Requests file metadata from peers
4. Downloads file pieces and assembles file

## File Structure

### Torrent Metadata

Each file has associated metadata stored in DHT:

```rust
struct TorrentMetadata {
    info_hash: String,      // SHA256 hash of filename + size
    filename: String,        // Original filename
    file_size: u64,          // File size in bytes
    piece_size: u64,         // Size of each piece (64 KB)
    pieces: Vec<String>,     // SHA256 hashes of each piece
    announce: Vec<String>,   // Peer addresses sharing file
}
```

### File Pieces

Files are split into **64 KB pieces** for efficient transfer:
- Each piece has a SHA256 hash
- Pieces can be downloaded in parallel
- Pieces can be verified using hashes

## Protocol

### Torrent Protocol Messages

**Request Messages:**
- `ListFiles`: Request list of available files
- `RequestMetadata { info_hash }`: Request file metadata
- `RequestPiece { info_hash, piece_index }`: Request a file piece

**Response Messages:**
- `FileList { files }`: List of available files
- `Metadata { metadata }`: File metadata
- `PieceData { info_hash, piece_index, data }`: File piece data

### Communication Flow

```
Client                    Server
  |                         |
  |-- ListFiles ----------->|
  |<-- FileList ------------|
  |                         |
  |-- RequestMetadata ----->|
  |<-- Metadata ------------|
  |                         |
  |-- RequestPiece[0] ----->|
  |<-- PieceData[0] --------|
  |                         |
  |-- RequestPiece[1] ----->|
  |<-- PieceData[1] --------|
  |                         |
  |-- ... (all pieces)      |
```

## Example Usage

### Step 1: Start Monitor (Bootstrap Node)

```bash
cargo run --release --bin monitor \
  --listen-addr 0.0.0.0 \
  --port 51820 \
  --web-port 8080
```

### Step 2: Start Torrent Server

```bash
# Create shared directory
mkdir shared

# Add a file to share
echo "Hello, P2P World!" > shared/test.txt

# Start torrent server
cargo run --release --bin torrent_server \
  --bootstrap /ip4/127.0.0.1/tcp/51820 \
  --share-dir ./shared
```

**Output:**
```
=== Torrent File Server ===
Share Directory: ./shared
Bootstrap: /ip4/127.0.0.1/tcp/51820

Peer ID: 12D3KooW...

[TORRENT] Loaded 1 file(s) for sharing
✓ DHT bootstrapped!
✓ Registered 1 file(s) in DHT
[TORRENT] Sharing: test.txt (hash: abc123...)
```

### Step 3: Start Torrent Client

```bash
# Create download directory
mkdir downloads

# List available files
cargo run --release --bin torrent_client \
  --bootstrap /ip4/127.0.0.1/tcp/51820 \
  --download-dir ./downloads
```

**Output:**
```
=== Torrent File Client ===
Download Directory: ./downloads
Bootstrap: /ip4/127.0.0.1/tcp/51820

Peer ID: 12D3KooW...

✓ DHT bootstrapped!
[TORRENT] 📁 Available files:
  - test.txt (18 bytes, hash: abc123...)
```

### Step 4: Download File

```bash
# Download specific file (use hash from list)
cargo run --release --bin torrent_client \
  --bootstrap /ip4/127.0.0.1/tcp/51820 \
  --download-dir ./downloads \
  --info-hash abc123def456...
```

**Output:**
```
[DOWNLOAD] 📥 Starting download: test.txt
  Size: 18 bytes
  Pieces: 1
[DOWNLOAD] ✓ Received piece 0 (18 bytes)
✓ Download complete: test.txt
```

## Integration with Existing Network

### All Nodes Can Serve Files

Every node in the network (listener, dialer, monitor) can:
- Share files via torrent protocol
- Download files from other peers
- Store file metadata in DHT
- Route file queries for other peers

### DHT Integration

File metadata is stored in the DHT:
- **Key**: File info hash
- **Value**: Serialized `TorrentFileInfo`
- **Replication**: Stored on k closest nodes

This allows:
- Decentralized file discovery
- No central tracker needed
- Automatic peer discovery

## Features

### Current Implementation

✅ **File Sharing**
- Share files from directory
- Automatic file scanning
- Piece-based transfer (64 KB pieces)

✅ **File Discovery**
- Query DHT for available files
- List files from peers
- Get file metadata

✅ **File Download**
- Request file pieces
- Verify piece integrity (SHA256)
- Assemble file from pieces

✅ **DHT Integration**
- Store file metadata in DHT
- Query DHT for files
- Route queries for other peers

### Future Enhancements

- [ ] Parallel piece downloads from multiple peers
- [ ] Piece verification and retry on failure
- [ ] Resume interrupted downloads
- [ ] Bandwidth limiting
- [ ] Piece prioritization
- [ ] Seeding (continue sharing after download)
- [ ] .torrent file generation
- [ ] Magnet link support

## File Verification

### Info Hash Calculation

```rust
let mut hasher = Sha256::new();
hasher.update(filename.as_bytes());
hasher.update(file_size.to_le_bytes());
let info_hash = format!("{:x}", hasher.finalize());
```

### Piece Hash Calculation

```rust
let mut piece_hasher = Sha256::new();
piece_hasher.update(piece_data);
let piece_hash = format!("{:x}", piece_hasher.finalize());
```

## Security Considerations

1. **File Verification**: All pieces are verified using SHA256 hashes
2. **Info Hash**: Files are identified by cryptographic hash
3. **No Authentication**: Currently no authentication (add if needed)
4. **No Encryption**: File data is not encrypted (add if needed)

## Performance

### Piece Size

- **Current**: 64 KB per piece
- **Trade-off**: Smaller pieces = more overhead, larger pieces = less granularity
- **Optimal**: Depends on network conditions

### Parallel Downloads

Currently downloads pieces sequentially. Future enhancement:
- Download multiple pieces in parallel
- Download from multiple peers simultaneously
- Prioritize rare pieces

## Troubleshooting

### Files Not Appearing

1. Check that torrent server is running
2. Verify files are in `--share-dir`
3. Check DHT bootstrap status
4. Verify file registration in DHT

### Download Fails

1. Check peer connection
2. Verify info hash is correct
3. Check file exists on server
4. Verify piece hashes match

### DHT Not Finding Files

1. Wait for DHT bootstrap to complete
2. Check bootstrap node is accessible
3. Verify file metadata was registered
4. Try querying DHT directly

## Example: Complete Setup

```bash
# Terminal 1: Start monitor
cargo run --release --bin monitor -- --listen-addr 0.0.0.0 --port 51820

# Terminal 2: Start torrent server
mkdir shared
echo "Test file content" > shared/test.txt
cargo run --release --bin torrent_server -- --share-dir ./shared

# Terminal 3: Start torrent client
mkdir downloads
cargo run --release --bin torrent_client -- --download-dir ./downloads
```

## Summary

✅ **Every node is a torrent server** - Can share files
✅ **Every node is a torrent client** - Can download files  
✅ **DHT-based discovery** - No central tracker needed
✅ **Piece-based transfer** - Efficient file sharing
✅ **Automatic verification** - SHA256 hashes for integrity

The torrent functionality integrates seamlessly with the existing Kademlia DHT network, allowing decentralized file sharing across all nodes!






