# Post-Restart Script: Create 4 Shard Files and Upload to Rendezvous Server
# Run this after restarting to release file locks

param(
    [string]$SourceModel = "models_cache\mistral-7b-instruct-v0.2.Q4_K_M.gguf",
    [int]$NumShards = 4,
    [string]$RemoteUser = "dbertrand",
    [string]$RemoteHost = "eagleoneonline.ca",
    [string]$RemoteDir = "/home/dbertrand/punch-simple/shards"
)

Write-Host ""
Write-Host "================================================================================" -ForegroundColor Cyan
Write-Host "  CREATE AND UPLOAD 4 SHARD FILES" -ForegroundColor Cyan
Write-Host "================================================================================" -ForegroundColor Cyan
Write-Host ""

# Step 1: Clean up old/corrupted shard files
Write-Host "[1/5] Cleaning up old shard files..." -ForegroundColor Yellow
Remove-Item models_cache\shards\shard-*.gguf* -Force -ErrorAction SilentlyContinue
Remove-Item models_cache\shards\shard-temp-*.gguf -Force -ErrorAction SilentlyContinue
Write-Host "  [OK] Old shard files removed" -ForegroundColor Green

# Step 2: Verify source model exists
Write-Host ""
Write-Host "[2/5] Verifying source model file..." -ForegroundColor Yellow
if (-not (Test-Path $SourceModel)) {
    Write-Host "  [ERROR] Source model not found: $SourceModel" -ForegroundColor Red
    exit 1
}

$modelInfo = Get-Item $SourceModel
$modelSizeGB = [math]::Round($modelInfo.Length / 1GB, 2)
Write-Host "  [OK] Found model: $($modelInfo.Name) ($modelSizeGB GB)" -ForegroundColor Green

# Step 3: Split model into 4 shards
Write-Host ""
Write-Host "[3/5] Splitting model into $NumShards shards..." -ForegroundColor Yellow
Write-Host "  This may take several minutes..." -ForegroundColor Gray

.\split_gguf_shards.ps1 -GgufFile $SourceModel -NumShards $NumShards

# Verify all 4 shards were created
$shards = Get-ChildItem models_cache\shards\shard-*.gguf -ErrorAction SilentlyContinue | Where-Object {$_.Length -gt 1000000}
if ($shards.Count -ne $NumShards) {
    Write-Host "  [ERROR] Expected $NumShards shards, but found $($shards.Count)" -ForegroundColor Red
    Write-Host "  Created shards:" -ForegroundColor Yellow
    $shards | ForEach-Object { Write-Host "    - $($_.Name): $([math]::Round($_.Length / 1MB, 2)) MB" -ForegroundColor Gray }
    exit 1
}

Write-Host ""
Write-Host "  [OK] All $NumShards shards created successfully:" -ForegroundColor Green
$shards | Sort-Object Name | ForEach-Object {
    $sizeMB = [math]::Round($_.Length / 1MB, 2)
    Write-Host "    - $($_.Name): $sizeMB MB" -ForegroundColor White
}

# Step 4: Create remote directory
Write-Host ""
Write-Host "[4/5] Creating remote directory..." -ForegroundColor Yellow
$createCmd = "ssh -F NUL ${RemoteUser}@${RemoteHost} 'mkdir -p $RemoteDir'"
Invoke-Expression $createCmd | Out-Null
Write-Host "  [OK] Remote directory ready" -ForegroundColor Green

# Step 5: Upload shards
Write-Host ""
Write-Host "[5/5] Uploading shards to rendezvous server..." -ForegroundColor Yellow
$uploaded = 0
$failed = 0

foreach ($shard in ($shards | Sort-Object Name)) {
    $sizeMB = [math]::Round($shard.Length / 1MB, 2)
    Write-Host "  Uploading $($shard.Name) ($sizeMB MB)..." -ForegroundColor Gray
    
    $scpCmd = "scp -F NUL '$($shard.FullName)' ${RemoteUser}@${RemoteHost}:$RemoteDir/"
    $result = Invoke-Expression $scpCmd 2>&1
    
    if ($LASTEXITCODE -eq 0) {
        Write-Host "    [OK] Uploaded successfully" -ForegroundColor Green
        $uploaded++
    } else {
        Write-Host "    [ERROR] Upload failed" -ForegroundColor Red
        $failed++
    }
}

# Summary
Write-Host ""
Write-Host "================================================================================" -ForegroundColor Green
Write-Host "  SUMMARY" -ForegroundColor Green
Write-Host "================================================================================" -ForegroundColor Green
Write-Host ""
Write-Host "Shards created: $($shards.Count)/$NumShards" -ForegroundColor $(if ($shards.Count -eq $NumShards) { "Green" } else { "Yellow" })
Write-Host "Shards uploaded: $uploaded/$($shards.Count)" -ForegroundColor $(if ($uploaded -eq $shards.Count) { "Green" } else { "Yellow" })
if ($failed -gt 0) {
    Write-Host "Failed uploads: $failed" -ForegroundColor Red
}

Write-Host ""
Write-Host "Next steps:" -ForegroundColor Yellow
Write-Host "  1. Verify shards on server:" -ForegroundColor White
Write-Host "     ssh ${RemoteUser}@${RemoteHost} 'ls -lh $RemoteDir'" -ForegroundColor Gray
Write-Host ""
Write-Host "  2. Restart rendezvous server with seeding enabled:" -ForegroundColor White
Write-Host "     ssh ${RemoteUser}@${RemoteHost}" -ForegroundColor Gray
Write-Host "     cd /home/dbertrand/punch-simple" -ForegroundColor Gray
Write-Host "     ./target/release/server --listen-addr 0.0.0.0 --port 51820 --transport quic --seed-dir $RemoteDir" -ForegroundColor Gray
Write-Host ""
Write-Host "  3. Start your 4 shard nodes - they will download shards from the server" -ForegroundColor White
Write-Host ""
