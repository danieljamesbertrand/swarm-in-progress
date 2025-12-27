# Shard Files Location

## Default Directory

**Path**: `models_cache/shards/`

This is the default directory where shard GGUF files are stored and expected by the system.

## Configuration

The shard directory can be configured via:
- **Environment Variable**: `LLAMA_SHARDS_DIR`
- **Command Line Argument**: `--shards-dir`
- **Default**: `models_cache/shards`

## Current Shard Files Found

Located in: `C:\Users\dan\punch-simple\models_cache\shards\`

### Shard Files (for 4-shard setup):
- ✅ `shard-0.gguf` (515.9 MB)
- ✅ `shard-1.gguf` (498.6 MB)
- ✅ `shard-2.gguf` (441.0 MB)
- ✅ `shard-3.gguf` (459.9 MB)

### Additional Shard Files:
- `shard-4.gguf` (464.6 MB)
- `shard-5.gguf` (429.2 MB)
- `shard-6.gguf` (511.5 MB)
- `shard-7.gguf` (399.6 MB)
- `shard-00009-of-00010.gguf` (540.5 MB)
- `shard-00010-of-00010.gguf` (107.6 MB)

## How Nodes Find Shard Files

1. **On Startup**:**
   - Node scans `shards_dir` for existing `.gguf` files
   - Creates torrent metadata for any found files
   - Tries to load assigned shard: `shard-{shard_id}.gguf`

2. **File Naming Convention:**
   - Format: `shard-{shard_id}.gguf`
   - Example: `shard-0.gguf`, `shard-1.gguf`, etc.

3. **Code Reference:**
   ```rust
   // src/shard_listener.rs:335
   let shard_filename = format!("shard-{}.gguf", shard_id);
   let shard_path = self.shards_dir.join(&shard_filename);
   ```

## Status

✅ **Shard files exist** for 4-shard setup (shard-0 through shard-3)
✅ **Directory structure is correct**: `models_cache/shards/`
✅ **Files are properly named**: `shard-{id}.gguf`

## Next Steps

The shard files are present and correctly located. Nodes should be able to:
1. Find their assigned shard files on startup
2. Load them if they exist locally
3. Download them via torrent if they don't exist

