# Copy 4 Shards to Rendezvous Server
# Simple script to upload shard-0.gguf through shard-3.gguf

param(
    [string]$SourceDir = "models_cache\shards",
    [string]$RemoteUser = "dbertrand",
    [string]$RemoteHost = "eagleoneonline.ca",
    [string]$RemoteDir = "/home/dbertrand/punch-simple/shards"
)

Write-Host ""
Write-Host "Copying 4 shards to rendezvous server..." -ForegroundColor Cyan
Write-Host ""

# Find the 4 shard files (shard-0 through shard-3)
$shards = @()
for ($i = 0; $i -lt 4; $i++) {
    $shardPath = Join-Path $SourceDir "shard-$i.gguf"
    if (Test-Path $shardPath) {
        $file = Get-Item $shardPath
        if ($file.Length -gt 1000000) {  # At least 1MB
            $shards += $file
        }
    }
}

if ($shards.Count -eq 0) {
    Write-Host "ERROR: No shard files found (shard-0.gguf through shard-3.gguf)" -ForegroundColor Red
    exit 1
}

Write-Host "Found $($shards.Count) shard file(s):" -ForegroundColor Green
foreach ($shard in $shards) {
    $sizeGB = [math]::Round($shard.Length / 1GB, 2)
    Write-Host "  - $($shard.Name): $sizeGB GB" -ForegroundColor White
}
Write-Host ""

# Create remote directory
Write-Host "Creating remote directory..." -ForegroundColor Yellow
$createCmd = "ssh -F NUL ${RemoteUser}@${RemoteHost} 'mkdir -p $RemoteDir'"
Invoke-Expression $createCmd | Out-Null
Write-Host "  Remote directory ready" -ForegroundColor Green
Write-Host ""

# Upload each shard
$uploaded = 0
$failed = 0

foreach ($shard in $shards) {
    $sizeGB = [math]::Round($shard.Length / 1GB, 2)
    Write-Host "Uploading $($shard.Name) ($sizeGB GB)..." -ForegroundColor Yellow
    Write-Host "  This may take several minutes for large files..." -ForegroundColor Gray
    
    $scpCmd = "scp -F NUL '$($shard.FullName)' ${RemoteUser}@${RemoteHost}:$RemoteDir/"
    $result = Invoke-Expression $scpCmd 2>&1
    
    if ($LASTEXITCODE -eq 0) {
        Write-Host "  [OK] Uploaded successfully" -ForegroundColor Green
        $uploaded++
    } else {
        Write-Host "  [ERROR] Upload failed" -ForegroundColor Red
        Write-Host "  Error: $result" -ForegroundColor Red
        $failed++
    }
    Write-Host ""
}

# Summary
Write-Host "================================================================================" -ForegroundColor Cyan
Write-Host "SUMMARY" -ForegroundColor Cyan
Write-Host "================================================================================" -ForegroundColor Cyan
Write-Host "Shards uploaded: $uploaded/$($shards.Count)" -ForegroundColor $(if ($uploaded -eq $shards.Count) { "Green" } else { "Yellow" })
if ($failed -gt 0) {
    Write-Host "Failed uploads: $failed" -ForegroundColor Red
}
Write-Host ""

if ($uploaded -eq $shards.Count) {
    Write-Host "[OK] All shards copied successfully!" -ForegroundColor Green
    Write-Host ""
    Write-Host "Next step: Restart the rendezvous server with:" -ForegroundColor Yellow
    Write-Host "  ssh ${RemoteUser}@${RemoteHost}" -ForegroundColor White
    Write-Host "  cd /home/dbertrand/punch-simple" -ForegroundColor White
    Write-Host "  ./target/release/server --listen-addr 0.0.0.0 --port 51820 --transport quic --seed-dir $RemoteDir" -ForegroundColor White
} else {
    Write-Host "[WARNING] Some uploads failed. Please check the errors above." -ForegroundColor Yellow
}
