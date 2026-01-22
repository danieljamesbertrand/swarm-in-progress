# Diagnose QUIC Handshake Issues
# Checks both server and client sides

param(
    [string]$RemoteUser = "dbertrand",
    [string]$RemoteHost = "eagleoneonline.ca",
    [string]$RemoteDir = "/home/dbertrand/punch-simple"
)

Write-Host ""
Write-Host "========================================" -ForegroundColor Cyan
Write-Host "  QUIC HANDSHAKE DIAGNOSTICS" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

# Step 1: Check server status
Write-Host "[1/6] Checking server status..." -ForegroundColor Yellow
$serverProcess = ssh -F NUL ${RemoteUser}@${RemoteHost} 'ps aux | grep "./target/release/server" | grep -v grep'
if ($serverProcess) {
    Write-Host "  [OK] Server process found" -ForegroundColor Green
    Write-Host "  $serverProcess" -ForegroundColor Gray
} else {
    Write-Host "  [ERROR] Server process NOT running!" -ForegroundColor Red
}
Write-Host ""

# Step 2: Check server logs for listen address
Write-Host "[2/6] Checking server logs for listen address..." -ForegroundColor Yellow
$serverLogs = ssh -F NUL ${RemoteUser}@${RemoteHost} "tail -50 $RemoteDir/server.log 2>/dev/null | grep -E '(Listening|SERVER|Bootstrap|peer id)' | tail -10"
if ($serverLogs) {
    Write-Host "  Server logs:" -ForegroundColor Gray
    $serverLogs -split "`n" | ForEach-Object { Write-Host "    $_" -ForegroundColor Gray }
} else {
    Write-Host "  [WARNING] No server logs found or server.log is empty" -ForegroundColor Yellow
}
Write-Host ""

# Step 3: Check if UDP port is listening
Write-Host "[3/6] Checking if UDP port 51820 is listening..." -ForegroundColor Yellow
$udpCheck = ssh -F NUL ${RemoteUser}@${RemoteHost} 'ss -ulnp 2>/dev/null | grep 51820 || netstat -ulnp 2>/dev/null | grep 51820 || echo "Port not listening"'
if ($udpCheck -match "51820") {
    Write-Host "  [OK] UDP port 51820 is listening" -ForegroundColor Green
    Write-Host "  $udpCheck" -ForegroundColor Gray
} else {
    Write-Host "  [ERROR] UDP port 51820 is NOT listening!" -ForegroundColor Red
    Write-Host "  $udpCheck" -ForegroundColor Gray
}
Write-Host ""

# Step 4: Check firewall rules
Write-Host "[4/6] Checking firewall rules..." -ForegroundColor Yellow
$userIP = "162.221.207.169"
$firewallRules = ssh -F NUL ${RemoteUser}@${RemoteHost} "sudo ufw status | grep '51820/udp'"
if ($firewallRules -match $userIP) {
    Write-Host "  [OK] Firewall allows UDP 51820 from your IP ($userIP)" -ForegroundColor Green
} else {
    Write-Host "  [WARNING] Firewall may not allow your IP" -ForegroundColor Yellow
    Write-Host "  Current rules:" -ForegroundColor Gray
    $firewallRules -split "`n" | ForEach-Object { Write-Host "    $_" -ForegroundColor Gray }
}
Write-Host ""

# Step 5: Test UDP connectivity
Write-Host "[5/6] Testing UDP connectivity..." -ForegroundColor Yellow
Write-Host "  Attempting UDP connection test..." -ForegroundColor Gray
$udpTest = Test-NetConnection -ComputerName eagleoneonline.ca -Port 51820 -InformationLevel Quiet -WarningAction SilentlyContinue 2>&1
# UDP test is unreliable, but we can check if the port is reachable
Write-Host "  Note: UDP connectivity test is unreliable (UDP is connectionless)" -ForegroundColor Gray
Write-Host ""

# Step 6: Check client-side configuration
Write-Host "[6/6] Client-side configuration check..." -ForegroundColor Yellow
$bootstrapIP = [System.Net.Dns]::GetHostAddresses("eagleoneonline.ca") | Where-Object { $_.AddressFamily -eq 'InterNetwork' } | Select-Object -First 1
if ($bootstrapIP) {
    $bootstrapIPString = $bootstrapIP.IPAddressToString
    Write-Host "  [OK] Resolved eagleoneonline.ca to: $bootstrapIPString" -ForegroundColor Green
    Write-Host "  Client will connect to: /ip4/$bootstrapIPString/udp/51820/quic-v1" -ForegroundColor Gray
} else {
    Write-Host "  [ERROR] Failed to resolve eagleoneonline.ca" -ForegroundColor Red
}
Write-Host ""

# Summary
Write-Host "========================================" -ForegroundColor Cyan
Write-Host "  DIAGNOSIS SUMMARY" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

$issues = @()

if (-not $serverProcess) {
    $issues += "Server process is not running"
}

if ($udpCheck -notmatch "51820") {
    $issues += "UDP port 51820 is not listening"
}

if ($firewallRules -notmatch $userIP) {
    $issues += "Firewall may not allow your IP ($userIP)"
}

if ($issues.Count -eq 0) {
    Write-Host "  [OK] All checks passed!" -ForegroundColor Green
    Write-Host ""
    Write-Host "  If connection still fails, possible causes:" -ForegroundColor Yellow
    Write-Host "    1. Server is listening on wrong interface (check listen_addr)" -ForegroundColor White
    Write-Host "    2. NAT/firewall blocking QUIC handshake packets" -ForegroundColor White
    Write-Host "    3. Server transport type mismatch (server must use 'quic' or 'dual')" -ForegroundColor White
    Write-Host "    4. Client transport type mismatch (client must use 'quic' or 'dual')" -ForegroundColor White
    Write-Host "    5. QUIC handshake timeout (check server logs for errors)" -ForegroundColor White
} else {
    Write-Host "  [ERROR] Issues found:" -ForegroundColor Red
    foreach ($issue in $issues) {
        Write-Host "    - $issue" -ForegroundColor Red
    }
}

Write-Host ""
