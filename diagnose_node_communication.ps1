# Diagnose Node Communication
# Checks if nodes are connected and communicating

Write-Host ""
Write-Host "================================================================================" -ForegroundColor Cyan
Write-Host "  NODE COMMUNICATION DIAGNOSTIC" -ForegroundColor Cyan
Write-Host "================================================================================" -ForegroundColor Cyan
Write-Host ""

# Check if nodes are running
Write-Host "[1/5] Checking if nodes are running..." -ForegroundColor Yellow
$nodes = Get-Process | Where-Object {$_.ProcessName -eq "node"} -ErrorAction SilentlyContinue
if ($nodes) {
    Write-Host "  [OK] Found $($nodes.Count) node process(es)" -ForegroundColor Green
    $nodes | ForEach-Object {
        Write-Host "    PID: $($_.Id) | Started: $($_.StartTime)" -ForegroundColor Gray
    }
} else {
    Write-Host "  [ERROR] No node processes found!" -ForegroundColor Red
    exit 1
}

# Check bootstrap server
Write-Host ""
Write-Host "[2/5] Checking bootstrap server..." -ForegroundColor Yellow
$bootstrapHost = "eagleoneonline.ca"
$bootstrapPort = 51820

try {
    $bootstrapIP = [System.Net.Dns]::GetHostAddresses($bootstrapHost) | Where-Object { $_.AddressFamily -eq 'InterNetwork' } | Select-Object -First 1
    $bootstrapIPString = $bootstrapIP.IPAddressToString
    Write-Host "  [OK] Bootstrap server resolved: ${bootstrapHost} -> ${bootstrapIPString}" -ForegroundColor Green
    
    # Check if server is reachable
    $testConnection = Test-NetConnection -ComputerName $bootstrapIPString -Port $bootstrapPort -WarningAction SilentlyContinue -ErrorAction SilentlyContinue
    if ($testConnection.TcpTestSucceeded) {
        Write-Host "  [OK] Bootstrap server is reachable on port ${bootstrapPort}" -ForegroundColor Green
    } else {
        Write-Host "  [WARNING] Cannot reach bootstrap server on port ${bootstrapPort}" -ForegroundColor Yellow
    }
} catch {
    Write-Host "  [ERROR] Failed to resolve bootstrap server: $_" -ForegroundColor Red
}

# Check network connections
Write-Host ""
Write-Host "[3/5] Checking network connections..." -ForegroundColor Yellow
$connections = netstat -ano | Select-String "51820|51821|51822|51823|51824" | Select-Object -First 20
if ($connections) {
    Write-Host "  [OK] Found network connections on relevant ports:" -ForegroundColor Green
    $connections | ForEach-Object {
        Write-Host "    $_" -ForegroundColor Gray
    }
} else {
    Write-Host "  [WARNING] No connections found on ports 51820-51824" -ForegroundColor Yellow
}

# Check if nodes are listening
Write-Host ""
Write-Host "[4/5] Checking if nodes are listening..." -ForegroundColor Yellow
$listening = netstat -ano | Select-String "LISTENING" | Select-String "51821|51822|51823|51824"
if ($listening) {
    Write-Host "  [OK] Nodes are listening on ports:" -ForegroundColor Green
    $listening | ForEach-Object {
        Write-Host "    $_" -ForegroundColor Gray
    }
} else {
    Write-Host "  [WARNING] No nodes listening on ports 51821-51824" -ForegroundColor Yellow
    Write-Host "    Nodes may still be starting up..." -ForegroundColor Gray
}

# Check DHT discovery status (via status reports if available)
Write-Host ""
Write-Host "[5/5] Expected Communication Flow:" -ForegroundColor Yellow
Write-Host "  [INFO] Nodes should:" -ForegroundColor Gray
Write-Host "    1. Connect to bootstrap server (eagleoneonline.ca:51820)" -ForegroundColor Gray
Write-Host "    2. Bootstrap to DHT (Kademlia)" -ForegroundColor Gray
Write-Host "    3. Announce themselves to DHT" -ForegroundColor Gray
Write-Host "    4. Discover other nodes via DHT" -ForegroundColor Gray
Write-Host "    5. Send SWARM_READY messages when all shards loaded" -ForegroundColor Gray
Write-Host "    6. Send SHARD_LOADED messages when shards are loaded" -ForegroundColor Gray
Write-Host ""
Write-Host "  [INFO] Check node terminal windows for:" -ForegroundColor Gray
Write-Host "    - '[CONNECT] ✓✓✓ CONNECTED TO BOOTSTRAP NODE ✓✓✓'" -ForegroundColor Cyan
Write-Host "    - '[DHT] ✓✓✓ ANNOUNCED SHARD X TO DHT ✓✓✓'" -ForegroundColor Cyan
Write-Host "    - '[STATUS] System Status Report' (every 30 seconds)" -ForegroundColor Cyan
Write-Host "    - '[SWARM] 📢 Broadcasted SWARM_READY'" -ForegroundColor Cyan
Write-Host ""

Write-Host "================================================================================" -ForegroundColor Cyan
Write-Host "  DIAGNOSTIC COMPLETE" -ForegroundColor Cyan
Write-Host "================================================================================" -ForegroundColor Cyan
Write-Host ""
Write-Host "If you don't see connection messages, nodes may still be:" -ForegroundColor Yellow
Write-Host "  - Compiling (first run takes time)" -ForegroundColor Gray
Write-Host "  - Connecting to bootstrap (can take 10-30 seconds)" -ForegroundColor Gray
Write-Host "  - Bootstrapping to DHT (can take 30-60 seconds)" -ForegroundColor Gray
Write-Host ""
Write-Host "Wait 60 seconds and check the node terminal windows for status reports." -ForegroundColor Yellow
Write-Host ""
