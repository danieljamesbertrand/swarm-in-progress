# Torrent Seeding Update - Explicit 4 Shard Seeding

## Changes Made

Updated `scan_gguf_files()` in `src/shard_listener.rs` to explicitly seed the 4 primary shard files on startup.

## New Behavior

### On Node Startup:

1. **Explicit Primary Shard Seeding**:
   - Scans for `shard-0.gguf` through `shard-3.gguf`
   - Creates torrent metadata for each found file
   - Logs seeding status for each primary shard
   - Reports how many of the 4 primary shards are available

2. **Additional File Seeding**:
   - Still scans for other `.gguf` files in the directory
   - Seeds them as well (for backward compatibility)
   - Skips primary shards already processed

3. **Enhanced Logging**:
   ```
   [TORRENT] ✓ Seeding primary shard: shard-0.gguf (hash: ...)
   [TORRENT] ✓ Seeding primary shard: shard-1.gguf (hash: ...)
   [TORRENT] ✓ Seeding primary shard: shard-2.gguf (hash: ...)
   [TORRENT] ✓ Seeding primary shard: shard-3.gguf (hash: ...)
   [TORRENT] ═══════════════════════════════════════════════════════════════════════════
   [TORRENT] Torrent seeding complete:
   [TORRENT]   Primary shards (0-3): 4/4 seeded
   [TORRENT]   Additional files: X seeded
   [TORRENT]   Total files available for seeding: Y
   [TORRENT] ═══════════════════════════════════════════════════════════════════════════
   ```

## Benefits

1. **Guaranteed Seeding**: The 4 primary shard files are explicitly checked and seeded
2. **Better Visibility**: Clear logging shows which shards are being seeded
3. **Error Detection**: Warns if any of the 4 primary shard files are missing
4. **Backward Compatible**: Still seeds other GGUF files found in the directory

## File Location

Shard files are expected in: `models_cache/shards/`

Files:
- `shard-0.gguf`
- `shard-1.gguf`
- `shard-2.gguf`
- `shard-3.gguf`

## Verification

When nodes start, you should see:
- `[TORRENT] ✓ Seeding primary shard: shard-X.gguf` for each of the 4 shards
- `[TORRENT] Primary shards (0-3): 4/4 seeded` if all files are present

