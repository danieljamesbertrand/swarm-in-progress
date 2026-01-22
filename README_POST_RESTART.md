# Post-Restart Instructions

After restarting your computer to release file locks, run this script to create and upload the 4 shard files:

## Quick Start

```powershell
.\create_and_upload_shards.ps1
```

This script will:
1. ✅ Clean up old/corrupted shard files
2. ✅ Verify the source model file exists
3. ✅ Split the 4GB model into 4 shards (~1GB each)
4. ✅ Upload all 4 shards to the rendezvous server
5. ✅ Verify uploads completed

## What It Does

- **Source**: `models_cache\mistral-7b-instruct-v0.2.Q4_K_M.gguf` (4.07 GB)
- **Output**: 4 shard files in `models_cache\shards\`:
  - `shard-0.gguf` (~1.02 GB)
  - `shard-1.gguf` (~1.02 GB)
  - `shard-2.gguf` (~1.02 GB)
  - `shard-3.gguf` (~1.02 GB)
- **Upload**: All shards uploaded to `eagleoneonline.ca:/home/dbertrand/punch-simple/shards/`

## After Running

1. Verify shards on server:
   ```bash
   ssh dbertrand@eagleoneonline.ca 'ls -lh /home/dbertrand/punch-simple/shards/'
   ```

2. Restart rendezvous server (if needed):
   ```bash
   ssh dbertrand@eagleoneonline.ca
   cd /home/dbertrand/punch-simple
   ./target/release/server --listen-addr 0.0.0.0 --port 51820 --transport quic --seed-dir /home/dbertrand/punch-simple/shards
   ```

3. Start your 4 shard nodes - they will automatically download their assigned shards from the server!
