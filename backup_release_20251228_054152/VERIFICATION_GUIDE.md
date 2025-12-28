# Verification Guide: Torrent Files & Parallel Inference

## Goal
Verify that:
1. ✅ Torrent server contains all 4 .gguf files
2. ✅ Nodes are loading them for parallel inference

## Current Architecture

### Each Node is a Torrent Server
- **Scans** `models_cache/shards/` for .gguf files on startup
- **Seeds** all found files (all 4 shard files: shard-0 through shard-3)
- **Registers** files in DHT for auto-propagation
- **Responds** to `LIST_FILES` command with available files

### Shard Loading for Parallel Inference
- Each node **loads its assigned shard** (shard-X.gguf where X = shard_id)
- Node 0 loads shard-0.gguf → processes layers 0-7
- Node 1 loads shard-1.gguf → processes layers 8-15
- Node 2 loads shard-2.gguf → processes layers 16-23
- Node 3 loads shard-3.gguf → processes layers 24-31
- **Parallel inference** uses all 4 nodes simultaneously

## Verification Methods

### Method 1: Check Node Console Logs (Easiest)

**For Torrent Files (4 per node = 16 total):**
```
[TORRENT] ✓ Seeding primary shard: shard-0.gguf (hash: ...)
[TORRENT] ✓ Seeding primary shard: shard-1.gguf (hash: ...)
[TORRENT] ✓ Seeding primary shard: shard-2.gguf (hash: ...)
[TORRENT] ✓ Seeding primary shard: shard-3.gguf (hash: ...)
[TORRENT] Primary shards (0-3): 4/4 seeded
[TORRENT] Total files available for seeding: 4
```

**For Shard Loading (1 per node = 4 total):**
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
Query each node to verify it has all 4 files:
```json
{
  "command": "LIST_FILES",
  "request_id": "check-files",
  "from": "coordinator",
  "to": "node-peer-id"
}
```

**Expected Response:**
```json
{
  "status": "success",
  "result": {
    "files": [
      {"info_hash": "...", "filename": "shard-0.gguf", "size": 515931456},
      {"info_hash": "...", "filename": "shard-1.gguf", "size": 498632640},
      {"info_hash": "...", "filename": "shard-2.gguf", "size": 440993760},
      {"info_hash": "...", "filename": "shard-3.gguf", "size": 459900896}
    ]
  }
}
```

**GET_CAPABILITIES Command:**
Check if shard is loaded:
```json
{
  "command": "GET_CAPABILITIES",
  "request_id": "check-capabilities",
  "from": "coordinator",
  "to": "node-peer-id"
}
```

**Expected Response:**
```json
{
  "status": "success",
  "result": {
    "shard_id": 0,
    "capabilities": {
      "shard_loaded": true
    }
  }
}
```

### Method 3: Check File System

**Verify files exist:**
```powershell
Get-ChildItem models_cache/shards/shard-[0-3].gguf
```

**Expected:**
- shard-0.gguf (492 MB)
- shard-1.gguf (475.5 MB)
- shard-2.gguf (420.6 MB)
- shard-3.gguf (438.6 MB)

## Expected Behavior Summary

### Torrent Server (Each Node)
- ✅ Scans `models_cache/shards/` on startup
- ✅ Seeds all 4 shard files (shard-0 through shard-3)
- ✅ Registers files in DHT
- ✅ Responds to LIST_FILES with all 4 files

### Shard Loading (Each Node)
- ✅ Node 0: Loads shard-0.gguf
- ✅ Node 1: Loads shard-1.gguf
- ✅ Node 2: Loads shard-2.gguf
- ✅ Node 3: Loads shard-3.gguf

### Parallel Inference
- ✅ Coordinator routes request to all 4 nodes
- ✅ Each node processes its assigned layer range
- ✅ Results aggregated for final output

## Current Status

### Files on Disk
- ✅ shard-0.gguf: Present (492 MB)
- ✅ shard-1.gguf: Present (475.5 MB)
- ✅ shard-2.gguf: Present (420.6 MB)
- ✅ shard-3.gguf: Present (438.6 MB)

### Torrent Seeding
- ✅ Code implemented to seed all 4 files
- ⏳ **Need to verify**: Check node console logs

### Shard Loading
- ✅ Code implemented to load assigned shard
- ⏳ **Need to verify**: Check node console logs or query nodes

### Parallel Inference
- ✅ Code implemented for distributed inference
- ❌ **Cannot test**: DHT discovery broken (0 nodes found)

## Quick Check

**Run this command:**
```powershell
Get-ChildItem models_cache/shards/shard-[0-3].gguf | Select-Object Name, @{Name="SizeMB";Expression={[math]::Round($_.Length/1MB,1)}}
```

**Check node console windows for:**
- `[TORRENT] ✓ Seeding primary shard` (4 messages per node)
- `[SHARD] ✓✓✓ SHARD X LOADED` (1 message per node)

