# Torrent Auto-Propagation via DHT

## Overview

Torrent file metadata is now **automatically propagated** to the Kademlia DHT when nodes start up. This allows any node in the network to discover available torrent files without needing to directly query other nodes.

## How It Works

### 1. Node Startup
- Node scans `models_cache/shards/` for GGUF files
- Creates torrent metadata (info hash, filename, size) for each file
- Stores metadata in local `torrent_files` HashMap

### 2. DHT Registration (Auto-Propagation)
When the node receives a `RoutingUpdated` event (DHT routing table populated):

```rust
// Register each torrent file in DHT
for (info_hash, filename, size) in torrent_files {
    let file_info = json!({
        "info_hash": info_hash,
        "filename": filename,
        "size": size,
        "peer_id": peer_id,
    });
    
    // Use info_hash as DHT key
    let key = kad::RecordKey::new(&info_hash);
    let record = kad::Record::new(key, serde_json::to_vec(&file_info)?);
    kademlia.put_record(record, Quorum::One)?;
}
```

### 3. File Discovery
Other nodes can discover files by:
1. **DHT Query**: Query DHT for a specific info_hash
2. **Get Record**: Receive `FoundRecord` event with file metadata
3. **Connect**: Connect to the peer_id from the record
4. **Download**: Request file pieces via torrent protocol

## Benefits

1. **Automatic Discovery**: Files are discoverable via DHT without direct node queries
2. **Decentralized**: No central registry needed - files propagate through DHT
3. **Efficient**: Nodes can find files without knowing which node has them
4. **Scalable**: Works with any number of nodes

## Implementation

**File**: `src/shard_listener.rs:789-818`

**Key Features**:
- Registers torrent files in DHT after first `RoutingUpdated` event
- Uses info_hash as DHT key for efficient lookups
- Includes peer_id in metadata so nodes know where to connect
- Only registers once per node startup (prevents duplicate registrations)

## Example Flow

```
Node A (has shard-0.gguf):
  1. Scans files → creates torrent metadata
  2. RoutingUpdated event → registers in DHT
  3. DHT stores: {info_hash: "abc123...", filename: "shard-0.gguf", peer_id: "NodeA"}

Node B (needs shard-0.gguf):
  1. Calculates info_hash for "shard-0.gguf"
  2. Queries DHT: get_record(info_hash)
  3. Receives: {info_hash, filename, peer_id: "NodeA"}
  4. Connects to NodeA
  5. Downloads shard-0.gguf via torrent
```

## Status

✅ **Implemented**: Torrent files are automatically registered in DHT
✅ **Tested**: Code compiles successfully
⏳ **Pending**: Full system test to verify propagation works

## Next Steps

1. Test that nodes can discover files via DHT queries
2. Verify that file downloads work after DHT discovery
3. Check that multiple nodes can seed the same file

