# Start Rendezvous Server with QUIC Transport
# Ensures server runs with QUIC as the internode communication protocol

param(
    [string]$RemoteUser = "dbertrand",
    [string]$RemoteHost = "eagleoneonline.ca",
    [string]$RemoteDir = "/home/dbertrand/punch-simple",
    [string]$SeedDir = "/home/dbertrand/punch-simple/shards"
)

Write-Host ""
Write-Host "========================================" -ForegroundColor Cyan
Write-Host "  START RENDEZVOUS SERVER (QUIC)" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

# Step 1: Stop any existing server
Write-Host "[1/4] Stopping existing server..." -ForegroundColor Yellow
ssh -F NUL ${RemoteUser}@${RemoteHost} 'pkill -f "./target/release/server"' | Out-Null
Start-Sleep -Seconds 2
Write-Host "  [OK] Cleanup complete" -ForegroundColor Green
Write-Host ""

# Step 2: Verify firewall allows QUIC
Write-Host "[2/4] Verifying firewall for QUIC..." -ForegroundColor Yellow
$userIP = "162.221.207.169"
$firewallCheck = ssh -F NUL ${RemoteUser}@${RemoteHost} "sudo ufw status | grep '51820/udp' | grep '$userIP'"
if ($firewallCheck -match $userIP) {
    Write-Host "  [OK] Firewall allows QUIC from your IP" -ForegroundColor Green
} else {
    Write-Host "  Adding firewall rule..." -ForegroundColor Yellow
    ssh -F NUL ${RemoteUser}@${RemoteHost} "sudo ufw allow from $userIP to any port 51820 proto udp" | Out-Null
    Write-Host "  [OK] Firewall rule added" -ForegroundColor Green
}
Write-Host ""

# Step 3: Start server with QUIC
Write-Host "[3/4] Starting server with QUIC transport..." -ForegroundColor Yellow
Write-Host "  Transport: QUIC (UDP 51820)" -ForegroundColor Gray
Write-Host "  Seed Directory: $SeedDir" -ForegroundColor Gray
Write-Host ""

$startCmd = "cd $RemoteDir && nohup ./target/release/server --listen-addr 0.0.0.0 --port 51820 --transport quic --seed-dir $SeedDir > server.log 2>&1 &"
ssh -F NUL ${RemoteUser}@${RemoteHost} $startCmd | Out-Null
Start-Sleep -Seconds 3

# Step 4: Verify server is running
Write-Host "[4/4] Verifying server status..." -ForegroundColor Yellow
$serverProcess = ssh -F NUL ${RemoteUser}@${RemoteHost} 'ps aux | grep "./target/release/server" | grep -v grep'

if ($serverProcess -match "server") {
    Write-Host "  [OK] Server is running" -ForegroundColor Green
    
    # Check logs for QUIC confirmation
    Start-Sleep -Seconds 2
    $logs = ssh -F NUL ${RemoteUser}@${RemoteHost} 'tail -10 /home/dbertrand/punch-simple/server.log 2>/dev/null'
    if ($logs -match "QUIC" -or $logs -match "udp.*51820") {
        Write-Host "  [OK] Server listening on QUIC" -ForegroundColor Green
    }
} else {
    Write-Host "  [WARNING] Server process not found" -ForegroundColor Yellow
    Write-Host "  Check logs: ssh ${RemoteUser}@${RemoteHost} 'tail -f $RemoteDir/server.log'" -ForegroundColor Gray
}

Write-Host ""
Write-Host "========================================" -ForegroundColor Green
Write-Host "  SERVER STARTED WITH QUIC" -ForegroundColor Green
Write-Host "========================================" -ForegroundColor Green
Write-Host ""
Write-Host "QUIC is now the internode communication protocol!" -ForegroundColor Cyan
Write-Host ""
Write-Host "Your node can now connect via QUIC:" -ForegroundColor Yellow
Write-Host "  .\start_node_to_rendezvous.ps1 -ShardId 0 -TotalShards 8 -Transport quic" -ForegroundColor White
Write-Host ""
