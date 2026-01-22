# Manual Fix for shard-0.gguf

The file `models_cache\shards\shard-0.gguf` is currently locked by another process and cannot be automatically replaced.

## Quick Fix (Recommended)

1. **Close any programs that might be using the file:**
   - Any running shard nodes
   - File Explorer windows showing the shards folder
   - Antivirus software scanning the file
   - Any Rust/Punch processes

2. **Manually replace the file:**
   ```powershell
   # Delete the old oversized file
   Remove-Item models_cache\shards\shard-0.gguf -Force
   
   # Rename the correct file
   Rename-Item models_cache\shards\shard-0-new.gguf models_cache\shards\shard-0.gguf
   ```

3. **Verify the fix:**
   ```powershell
   Get-ChildItem models_cache\shards\shard-[0-7].gguf | Select-Object Name, @{Name="SizeMB";Expression={[math]::Round($_.Length / 1MB, 0)}}
   ```
   
   All shards should be ~521 MB (except shard-7 which is ~456 MB).

## Alternative: Restart Computer

If the file remains locked, restart your computer and then run:
```powershell
Remove-Item models_cache\shards\shard-0.gguf -Force
Rename-Item models_cache\shards\shard-0-new.gguf models_cache\shards\shard-0.gguf
```

## Current Status

- ✅ Shards 1-7: Correctly sized (~521 MB each)
- ❌ Shard-0: Still oversized (12.98 GB) - needs manual fix
- ✅ shard-0-new.gguf: Correct file (521 MB) ready to use
