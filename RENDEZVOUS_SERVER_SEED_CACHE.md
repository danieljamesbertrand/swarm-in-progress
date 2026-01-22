# Rendezvous Server Seed Cache - JSON Command Interface

## Overview

Yes, the rendezvous server **does report what files are in the seed cache** via the JSON command protocol. The server responds to `LIST_FILES` commands with a complete list of all shard files available for seeding.

---

## JSON Command: LIST_FILES

### Request Format

Send a `LIST_FILES` command to the rendezvous server to query available seed files:

```json
{
  "command": "LIST_FILES",
  "request_id": "req-1234567890",
  "from": "your-peer-id",
  "to": "rendezvous-server-peer-id",
  "timestamp": 1234567890,
  "params": {}
}
```

**Note:** The `to` field can be omitted or set to the rendezvous server's PeerId. If omitted, the command will be routed to the best available node (which should be the rendezvous server if it's the bootstrap node).

### Response Format

The server responds with a list of all files in the seed cache:

```json
{
  "command": "LIST_FILES",
  "request_id": "req-1234567890",
  "from": "rendezvous-server-peer-id",
  "to": "your-peer-id",
  "timestamp": 1234567891,
  "status": "success",
  "result": {
    "files": [
      {
        "info_hash": "abc123def456...",
        "filename": "shard-0.gguf",
        "size": 546308096
      },
      {
        "info_hash": "def456ghi789...",
        "filename": "shard-1.gguf",
        "size": 546308096
      },
      {
        "info_hash": "ghi789jkl012...",
        "filename": "shard-2.gguf",
        "size": 546308096
      },
      {
        "info_hash": "jkl012mno345...",
        "filename": "shard-3.gguf",
        "size": 546308096
      },
      {
        "info_hash": "mno345pqr678...",
        "filename": "shard-4.gguf",
        "size": 546308096
      },
      {
        "info_hash": "pqr678stu901...",
        "filename": "shard-5.gguf",
        "size": 546308096
      },
      {
        "info_hash": "stu901vwx234...",
        "filename": "shard-6.gguf",
        "size": 546308096
      },
      {
        "info_hash": "vwx234yza567...",
        "filename": "shard-7.gguf",
        "size": 478412800
      }
    ]
  },
  "error": null
}
```

### Response Fields

Each file in the `files` array contains:

- **`info_hash`**: SHA256 hash identifier for the file (used for torrent requests)
- **`filename`**: Name of the shard file (e.g., `shard-0.gguf`)
- **`size`**: File size in bytes

---

## How It Works

### Server-Side Implementation

When the rendezvous server starts with `--seed-dir`:

1. **Scans directory** on startup (line 153-199 in `src/server.rs`)
2. **Loads all `.gguf` and `.safetensors` files** into memory
3. **Creates torrent metadata** for each file:
   - Calculates info hash (SHA256 of filename + size)
   - Splits file into 64 KB pieces
   - Calculates SHA256 hash for each piece
4. **Stores files** in internal `TorrentServer` structure
5. **Responds to LIST_FILES** requests (line 427-429 in `src/server.rs`)

### Request Handling

When a `LIST_FILES` command is received:

```rust
// Server code (src/server.rs:427-429)
TorrentMessage::ListFiles => {
    let files = torrent_guard.get_file_list();
    TorrentMessage::FileList { files }
}
```

The `get_file_list()` method returns a vector of `TorrentFileInfo` structures containing:
- `info_hash`: String
- `filename`: String  
- `size`: u64

---

## Alternative: Direct Torrent Protocol

The rendezvous server also supports the **torrent protocol directly** (not via JSON commands):

### Torrent Protocol LIST_FILES

```rust
// Direct torrent protocol message (not JSON)
TorrentMessage::ListFiles
```

**Response:**
```rust
TorrentMessage::FileList {
    files: Vec<TorrentFileInfo>
}
```

This is used for direct peer-to-peer torrent communication, while the JSON `LIST_FILES` command is used for the higher-level command protocol.

---

## Command Protocol Integration

The `LIST_FILES` command is part of the standard JSON command protocol defined in `src/command_protocol.rs`:

```rust
pub mod commands {
    pub const LIST_FILES: &str = "LIST_FILES";
    // ... other commands
}
```

### Related Commands

Other file-related commands in the protocol:

- **`GET_FILE_METADATA`**: Get detailed torrent metadata for a specific file
- **`REQUEST_PIECE`**: Request a specific piece of a file
- **`LOAD_SHARD`**: Load a shard file (triggers download if missing)
- **`SYNC_TORRENTS`**: Synchronize torrent file lists between nodes

---

## Usage Examples

### Example 1: Query Rendezvous Server for Available Files

```json
{
  "command": "LIST_FILES",
  "request_id": "check-seed-cache-001",
  "from": "coordinator-peer-id",
  "to": "rendezvous-server-peer-id"
}
```

**Expected Response:**
```json
{
  "command": "LIST_FILES",
  "request_id": "check-seed-cache-001",
  "status": "success",
  "result": {
    "files": [
      {
        "info_hash": "...",
        "filename": "shard-0.gguf",
        "size": 546308096
      },
      // ... 7 more shards
    ]
  }
}
```

### Example 2: Verify All 8 Shards Are Available

After uploading shards, you can verify they're all in the seed cache:

```json
{
  "command": "LIST_FILES",
  "request_id": "verify-8-shards",
  "from": "admin-peer-id",
  "to": "rendezvous-server-peer-id"
}
```

**Verification:**
- Check that `result.files.length == 8`
- Verify filenames: `shard-0.gguf` through `shard-7.gguf`
- Check file sizes are correct (~521 MB each, last one ~456 MB)

---

## Server Logs

When the server scans and loads files, you'll see:

```
[TORRENT] Scanning directory: /home/dbertrand/punch-simple/shards
[TORRENT]   ✓ Loaded: shard-0.gguf (521.00 MB, hash: abc123...)
[TORRENT]   ✓ Loaded: shard-1.gguf (521.00 MB, hash: def456...)
...
[TORRENT] Scanned 8 file(s), loaded 8 shard file(s) for sharing
[TORRENT] ✓ Torrent seeding enabled
```

When a `LIST_FILES` request is received:

```
[TORRENT] 📥 File list requested
```

---

## Summary

✅ **Yes, the rendezvous server reports seed cache files**

- **Command**: `LIST_FILES` (JSON command protocol)
- **Response**: JSON array of file info (info_hash, filename, size)
- **Protocol**: Standard JSON command protocol (`src/command_protocol.rs`)
- **Integration**: Part of the node communication pipeline
- **Usage**: Query available shard files before downloading

The `LIST_FILES` command is fully integrated into the JSON command language and is the standard way to query what files are available in the seed cache.
