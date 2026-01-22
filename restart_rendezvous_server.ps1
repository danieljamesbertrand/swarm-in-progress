# Restart Rendezvous Server with Torrent Seeding
# Restarts the server on eagleoneonline.ca with seed directory enabled

param(
    [string]$RemoteUser = "dbertrand",
    [string]$RemoteHost = "eagleoneonline.ca",
    [string]$RemoteDir = "/home/dbertrand/punch-simple",
    [string]$SeedDir = "/home/dbertrand/punch-simple/shards",
    [string]$ListenAddr = "0.0.0.0",
    [int]$Port = 51820,
    [string]$Transport = "quic"
)

Write-Host ""
Write-Host "========================================" -ForegroundColor Cyan
Write-Host "  RESTART RENDEZVOUS SERVER" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

# Step 1: Stop existing server
Write-Host "[1/3] Stopping existing server..." -ForegroundColor Yellow
$stopCmd = "ssh -F NUL ${RemoteUser}@${RemoteHost} 'pkill -f server'"
$stopResult = Invoke-Expression $stopCmd 2>&1
Start-Sleep -Seconds 2
Write-Host "  [OK] Server stopped" -ForegroundColor Green
Write-Host ""

# Step 2: Verify seed directory exists and has files
Write-Host "[2/3] Verifying seed directory..." -ForegroundColor Yellow
$checkCmd = "ssh -F NUL ${RemoteUser}@${RemoteHost} 'ls -lh $SeedDir/*.gguf 2>/dev/null | wc -l'"
$fileCount = (Invoke-Expression $checkCmd 2>&1 | Out-String).Trim()

if ($fileCount -match '^\d+$' -and [int]$fileCount -gt 0) {
    Write-Host "  [OK] Found $fileCount shard file(s) in seed directory" -ForegroundColor Green
} else {
    Write-Host "  [WARNING] No shard files found in seed directory" -ForegroundColor Yellow
    Write-Host "  Directory: $SeedDir" -ForegroundColor Gray
    Write-Host "  You may need to upload shards first using: .\copy_8_shards_to_rendezvous.ps1" -ForegroundColor Yellow
}
Write-Host ""

# Step 3: Start server with seeding enabled
Write-Host "[3/3] Starting server with torrent seeding..." -ForegroundColor Yellow
$listenAddrPort = "${ListenAddr}:${Port}"
Write-Host "  Listen: $listenAddrPort" -ForegroundColor Gray
Write-Host "  Transport: $Transport" -ForegroundColor Gray
Write-Host "  Seed Dir: $SeedDir" -ForegroundColor Gray
Write-Host ""

$startCmd = "ssh -F NUL ${RemoteUser}@${RemoteHost} 'cd $RemoteDir && nohup ./target/release/server --listen-addr $ListenAddr --port $Port --transport $Transport --seed-dir $SeedDir > server.log 2>&1 &'"
$startResult = Invoke-Expression $startCmd 2>&1

if ($LASTEXITCODE -eq 0) {
    Write-Host "  [OK] Server started in background" -ForegroundColor Green
    Write-Host ""
    Write-Host "Checking server status..." -ForegroundColor Yellow
    Start-Sleep -Seconds 3
    
    $statusCmd = "ssh -F NUL ${RemoteUser}@${RemoteHost} 'ps aux | grep server | grep -v grep'"
    $status = Invoke-Expression $statusCmd 2>&1
    
    if ($status -match 'server') {
        Write-Host "  [OK] Server is running" -ForegroundColor Green
    } else {
        Write-Host "  [WARNING] Server process not found - check logs" -ForegroundColor Yellow
    }
    
    Write-Host ""
    Write-Host "========================================" -ForegroundColor Green
    Write-Host "  SERVER RESTARTED" -ForegroundColor Green
    Write-Host "========================================" -ForegroundColor Green
    Write-Host ""
    Write-Host "To view server logs:" -ForegroundColor Yellow
    Write-Host "  ssh ${RemoteUser}@${RemoteHost}" -ForegroundColor White
    Write-Host "  tail -f $RemoteDir/server.log" -ForegroundColor Gray
    Write-Host ""
    Write-Host "To check if seeding is active, look for:" -ForegroundColor Yellow
    Write-Host "  [TORRENT] Scanned X file(s), loaded Y shard file(s) for sharing" -ForegroundColor Gray
    Write-Host "  [TORRENT] ✓ Torrent seeding enabled" -ForegroundColor Gray
    Write-Host ""
} else {
    Write-Host "  [ERROR] Failed to start server" -ForegroundColor Red
    Write-Host "  Error: $startResult" -ForegroundColor Red
    Write-Host ""
    Write-Host "Troubleshooting:" -ForegroundColor Yellow
    Write-Host "  1. SSH to server: ssh ${RemoteUser}@${RemoteHost}" -ForegroundColor White
    Write-Host "  2. Check if server binary exists: ls -lh $RemoteDir/target/release/server" -ForegroundColor White
    Write-Host "  3. Try starting manually:" -ForegroundColor White
    Write-Host "     cd $RemoteDir" -ForegroundColor Gray
    $manualCmd = './target/release/server --listen-addr ' + $ListenAddr + ' --port ' + $Port + ' --transport ' + $Transport + ' --seed-dir ' + $SeedDir
    Write-Host ('     ' + $manualCmd) -ForegroundColor Gray
    Write-Host ""
    exit 1
}
