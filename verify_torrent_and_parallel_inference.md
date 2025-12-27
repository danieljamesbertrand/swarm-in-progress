# Verify Torrent Files and Parallel Inference

## Goal
Verify that:
1. Torrent server contains all 4 .gguf files
2. Nodes are loading them for parallel inference

## Current Architecture

### Torrent Server
Each **shard_listener node** acts as a torrent server:
- Scans `models_cache/shards/` for .gguf files on startup
- Seeds all found files (all 4 shard files)
- Registers files in DHT for auto-propagation
- Responds to `LIST_FILES` command with available files

### Shard Loading
Each node:
- Loads its assigned shard (shard-X.gguf where X = shard_id)
- Can load additional shards if needed
- Uses loaded shards for parallel inference

## Verification Methods

### Method 1: Check Node Console Logs

**For Torrent Files:**
Look for these messages in each node's console:
```
[TORRENT] ✓ Seeding primary shard: shard-0.gguf (hash: ...)
[TORRENT] ✓ Seeding primary shard: shard-1.gguf (hash: ...)
[TORRENT] ✓ Seeding primary shard: shard-2.gguf (hash: ...)
[TORRENT] ✓ Seeding primary shard: shard-3.gguf (hash: ...)
[TORRENT] Primary shards (0-3): 4/4 seeded
[TORRENT] Total files available for seeding: 4
```

**For DHT Registration:**
```
[TORRENT] Registering 4 torrent file(s) in DHT for auto-propagation...
[TORRENT] ✓ Registered torrent file in DHT: shard-0.gguf (hash: ...)
[TORRENT] ✓ Registered torrent file in DHT: shard-1.gguf (hash: ...)
[TORRENT] ✓ Registered torrent file in DHT: shard-2.gguf (hash: ...)
[TORRENT] ✓ Registered torrent file in DHT: shard-3.gguf (hash: ...)
[TORRENT] ✓ All torrent files registered in DHT - auto-propagation enabled
```

**For Shard Loading:**
```
[SHARD] ✓✓✓ SHARD 0 LOADED BEFORE JOINING NETWORK ✓✓✓
[SHARD]   Path: models_cache/shards/shard-0.gguf
```

OR (if loaded via LOAD_SHARD command):
```
[LOAD_SHARD] ✓ Loaded shard 0 from local directory
[SHARD] ✓✓✓ SHARD 0 LOADED ✓✓✓
```

### Method 2: Query Nodes via Commands

**LIST_FILES Command:**
Query each node to get list of available torrent files:
```json
{
  "command": "LIST_FILES",
  "request_id": "check-files-1",
  "from": "coordinator-peer-id",
  "to": "node-peer-id"
}
```

Expected response:
```json
{
  "command": "LIST_FILES",
  "status": "success",
  "result": {
    "files": [
      {
        "info_hash": "...",
        "filename": "shard-0.gguf",
        "size": 515931456
      },
      {
        "info_hash": "...",
        "filename": "shard-1.gguf",
        "size": 498632640
      },
      {
        "info_hash": "...",
        "filename": "shard-2.gguf",
        "size": 440993760
      },
      {
        "info_hash": "...",
        "filename": "shard-3.gguf",
        "size": 459900896
      }
    ]
  }
}
```

**GET_CAPABILITIES Command:**
Check if shard is loaded:
```json
{
  "command": "GET_CAPABILITIES",
  "request_id": "check-capabilities-1",
  "from": "coordinator-peer-id",
  "to": "node-peer-id"
}
```

Expected response includes:
```json
{
  "result": {
    "shard_id": 0,
    "capabilities": {
      "shard_loaded": true,
      ...
    }
  }
}
```

### Method 3: Check File System

**Verify files exist:**
```powershell
Get-ChildItem models_cache/shards/shard-*.gguf | Select-Object Name, Length
```

Expected:
- shard-0.gguf (492 MB)
- shard-1.gguf (475.5 MB)
- shard-2.gguf (420.6 MB)
- shard-3.gguf (438.6 MB)

## Parallel Inference Verification

### Expected Behavior

When inference request comes in:
1. **Coordinator** routes request to nodes based on shard assignment
2. **Node 0** processes layers 0-7 (shard-0.gguf)
3. **Node 1** processes layers 8-15 (shard-1.gguf)
4. **Node 2** processes layers 16-23 (shard-2.gguf)
5. **Node 3** processes layers 24-31 (shard-3.gguf)
6. **Coordinator** aggregates results

### Check Inference Logs

Look for:
```
[EXECUTE_TASK] Processing inference for shard 0
[EXECUTE_TASK] Processing inference for shard 1
[EXECUTE_TASK] Processing inference for shard 2
[EXECUTE_TASK] Processing inference for shard 3
```

## Current Status

### Files on Disk
- ✅ shard-0.gguf: Present
- ✅ shard-1.gguf: Present
- ✅ shard-2.gguf: Present
- ✅ shard-3.gguf: Present

### Torrent Seeding
- ✅ Code implemented to seed all 4 files
- ⏳ Need to verify nodes are actually seeding (check console logs)

### Shard Loading
- ✅ Code implemented to load assigned shard
- ⏳ Need to verify nodes have loaded shards (check console logs or query)

### Parallel Inference
- ✅ Code implemented for distributed inference
- ❌ Cannot test until DHT discovery works (0 nodes found)

## Next Steps

1. **Check Node Console Windows** for torrent seeding messages
2. **Query Nodes** via LIST_FILES command to verify files available
3. **Query Nodes** via GET_CAPABILITIES to verify shards loaded
4. **Fix DHT Discovery** to enable parallel inference testing

