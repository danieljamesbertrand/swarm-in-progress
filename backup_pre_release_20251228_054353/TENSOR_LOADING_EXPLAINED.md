# Tensor Loading Explained

## How Shard Nodes Load Tensors

### Current Implementation

The shard nodes **track shard file paths** but don't actually load tensors into memory until inference is performed. Here's the flow:

### 1. Node Startup (Automatic Loading)

When a shard node starts:
1. **Scans** `models_cache/shards/` for `shard-{shard_id}.gguf`
2. **If found**: 
   - Prints: `[SHARD] ✓✓✓ SHARD X LOADED BEFORE JOINING NETWORK ✓✓✓`
   - Stores file path in `loaded_shards` map
   - Marks `capabilities.shard_loaded = true`
3. **If not found**:
   - Prints: `[SHARD] ⚠️  ASSIGNED SHARD X NOT FOUND LOCALLY ⚠️`
   - Node still joins network
   - Will download via torrent when LOAD_SHARD command received

### 2. LOAD_SHARD Command

When coordinator sends `LOAD_SHARD` command:
1. Node checks if shard already loaded
2. If not, tries to load from local directory
3. If not found locally, starts torrent download
4. Prints: `[LOAD_SHARD] ✓ Loaded shard X from: ...`

### 3. EXECUTE_TASK Command (Inference)

When inference is requested:
1. Node checks if shard is loaded
2. If not, tries to auto-load: `load_shard_file(shard_id)`
3. If file exists, loads it and prints: `[INFERENCE] Loaded shard X from: ...`
4. Processes inference (currently simulated, not actual tensor loading)

## What "Loading Tensors" Means

**Current State:**
- Nodes **track file paths** to GGUF files
- Files are **not loaded into memory** until inference
- The actual tensor loading would happen in llama.cpp or candle integration

**Future Implementation:**
- When inference runs, llama.cpp would:
  1. Open the GGUF file
  2. Read tensor metadata
  3. Load tensors into GPU/CPU memory
  4. Process inference through the model layers

## Messages to Look For

### ✅ Shard File Found and Tracked:
```
[SHARD] ✓✓✓ SHARD 0 LOADED BEFORE JOINING NETWORK ✓✓✓
[SHARD]   Path: models_cache/shards/shard-0.gguf
[SHARD]   Shard will be available for inference immediately
```

### ✅ Shard Loaded via Command:
```
[LOAD_SHARD] ✓ Loaded shard 0 from local directory
[INFERENCE] Loaded shard 0 from: models_cache/shards/shard-0.gguf
```

### ⚠️ Shard Not Found:
```
[SHARD] ⚠️  ASSIGNED SHARD 0 NOT FOUND LOCALLY ⚠️
[SHARD]   Node will join the network and download shard when LOAD_SHARD command is received.
```

### ✅ Shard Announced to DHT:
```
[DHT] ✓✓✓ ANNOUNCED SHARD 0 TO DHT ✓✓✓
```

## Ensuring Nodes Load Shards

### Method 1: Shard Files Exist Locally
- Place `shard-0.gguf`, `shard-1.gguf`, etc. in `models_cache/shards/`
- Nodes will automatically load them on startup
- Look for `[SHARD] SHARD X LOADED` message

### Method 2: Coordinator Sends LOAD_SHARD
- Coordinator detects missing shards
- Sends `LOAD_SHARD` command to nodes
- Nodes load shards (local or via torrent)
- Look for `[LOAD_SHARD] ✓ Loaded shard X` message

### Method 3: Auto-Load on Inference
- When `EXECUTE_TASK` is received
- Node checks if shard is loaded
- Auto-loads if file exists locally
- Look for `[INFERENCE] Loaded shard X` message

## Current Status

The system **tracks shard file paths** but doesn't actually load tensors into memory yet. The actual tensor loading would require:
- llama.cpp integration for GGUF file loading
- Or candle integration for model loading
- GPU/CPU memory allocation for tensors

For now, nodes:
- ✅ Track which shard files they have
- ✅ Can download missing shards via torrent
- ✅ Announce shard availability to DHT
- ⚠️ Don't actually load tensors into memory (simulated inference)

## Next Steps for Full Tensor Loading

1. Integrate llama.cpp or candle for actual model loading
2. Load GGUF file and parse tensor metadata
3. Allocate memory for tensors
4. Load tensor weights into memory
5. Process inference through actual model layers

