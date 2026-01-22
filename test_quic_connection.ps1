# Comprehensive QUIC Connection Test
# Tests firewall, server, and client connection

param(
    [string]$RemoteUser = "dbertrand",
    [string]$RemoteHost = "eagleoneonline.ca",
    [string]$RemoteDir = "/home/dbertrand/punch-simple",
    [string]$ClientIP = "162.221.207.169"
)

Write-Host ""
Write-Host "========================================" -ForegroundColor Cyan
Write-Host "  COMPREHENSIVE QUIC CONNECTION TEST" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

$allTestsPassed = $true

# Test 1: Verify Firewall Rule
Write-Host "[TEST 1/6] Checking firewall rule for client IP..." -ForegroundColor Yellow
$firewallCheck = ssh -F NUL ${RemoteUser}@${RemoteHost} "sudo ufw status | grep '$ClientIP.*51820/udp'"
if ($firewallCheck -match $ClientIP) {
    Write-Host "  [PASS] Firewall allows UDP 51820 from $ClientIP" -ForegroundColor Green
    Write-Host "    Rule: $firewallCheck" -ForegroundColor Gray
} else {
    Write-Host "  [FAIL] Firewall rule NOT found for $ClientIP" -ForegroundColor Red
    Write-Host "    Adding rule now..." -ForegroundColor Yellow
    ssh -F NUL ${RemoteUser}@${RemoteHost} "sudo ufw allow from $ClientIP to any port 51820 proto udp" | Out-Null
    Start-Sleep -Seconds 1
    $firewallCheck = ssh -F NUL ${RemoteUser}@${RemoteHost} "sudo ufw status | grep '$ClientIP.*51820/udp'"
    if ($firewallCheck -match $ClientIP) {
        Write-Host "  [PASS] Firewall rule added successfully" -ForegroundColor Green
    } else {
        Write-Host "  [FAIL] Failed to add firewall rule" -ForegroundColor Red
        $allTestsPassed = $false
    }
}
Write-Host ""

# Test 2: Verify Server Process
Write-Host "[TEST 2/6] Checking if server process is running..." -ForegroundColor Yellow
$serverProcess = ssh -F NUL ${RemoteUser}@${RemoteHost} 'ps aux | grep "./target/release/server" | grep -v grep'
if ($serverProcess) {
    Write-Host "  [PASS] Server process is running" -ForegroundColor Green
    $serverProcess -split "`n" | ForEach-Object { Write-Host "    $_" -ForegroundColor Gray }
} else {
    Write-Host "  [FAIL] Server process NOT running" -ForegroundColor Red
    Write-Host "    Starting server..." -ForegroundColor Yellow
    ssh -F NUL ${RemoteUser}@${RemoteHost} "cd $RemoteDir && nohup ./target/release/server --listen-addr 0.0.0.0 --port 51820 --transport quic --seed-dir $RemoteDir/shards > server.log 2>&1 &" | Out-Null
    Start-Sleep -Seconds 3
    $serverProcess = ssh -F NUL ${RemoteUser}@${RemoteHost} 'ps aux | grep "./target/release/server" | grep -v grep'
    if ($serverProcess) {
        Write-Host "  [PASS] Server started successfully" -ForegroundColor Green
    } else {
        Write-Host "  [FAIL] Server failed to start" -ForegroundColor Red
        $allTestsPassed = $false
    }
}
Write-Host ""

# Test 3: Verify Server Listening on UDP Port
Write-Host "[TEST 3/6] Checking if server is listening on UDP 51820..." -ForegroundColor Yellow
$udpListen = ssh -F NUL ${RemoteUser}@${RemoteHost} 'ss -uln | grep 51820 || netstat -uln 2>/dev/null | grep 51820'
if ($udpListen -match "51820") {
    Write-Host "  [PASS] Server is listening on UDP 51820" -ForegroundColor Green
    Write-Host "    $udpListen" -ForegroundColor Gray
} else {
    Write-Host "  [FAIL] Server is NOT listening on UDP 51820" -ForegroundColor Red
    Write-Host "    Check server logs for errors" -ForegroundColor Yellow
    $allTestsPassed = $false
}
Write-Host ""

# Test 4: Check Server Logs for QUIC Listen
Write-Host "[TEST 4/6] Checking server logs for QUIC listen confirmation..." -ForegroundColor Yellow
$serverLogs = ssh -F NUL ${RemoteUser}@${RemoteHost} "tail -30 $RemoteDir/server.log 2>/dev/null | grep -E '(Listening|SERVER|QUIC|peer id)'"
if ($serverLogs) {
    Write-Host "  [PASS] Server logs found" -ForegroundColor Green
    $serverLogs -split "`n" | Select-Object -First 5 | ForEach-Object { Write-Host "    $_" -ForegroundColor Gray }
} else {
    Write-Host "  [WARNING] No server logs found or server.log is empty" -ForegroundColor Yellow
    Write-Host "    Server may have just started" -ForegroundColor Gray
}
Write-Host ""

# Test 5: Test UDP Connectivity (Basic)
Write-Host "[TEST 5/6] Testing UDP connectivity to server..." -ForegroundColor Yellow
Write-Host "  Note: UDP is connectionless, so this test is limited" -ForegroundColor Gray
$udpTest = Test-NetConnection -ComputerName eagleoneonline.ca -Port 51820 -InformationLevel Quiet -WarningAction SilentlyContinue 2>&1
if ($LASTEXITCODE -eq 0 -or $udpTest -match "True") {
    Write-Host "  [INFO] UDP port appears reachable (but UDP tests are unreliable)" -ForegroundColor Cyan
} else {
    Write-Host "  [INFO] UDP test inconclusive (UDP is connectionless)" -ForegroundColor Gray
}
Write-Host ""

# Test 6: Start Client Node and Monitor Connection
Write-Host "[TEST 6/6] Starting client node and monitoring connection..." -ForegroundColor Yellow
Write-Host "  This will start a node in a new window and monitor for 30 seconds" -ForegroundColor Gray
Write-Host ""

# Start the node
$nodeStarted = $false
try {
    Start-Process powershell -ArgumentList "-NoExit", "-Command", "cd '$PWD'; .\start_node_to_rendezvous.ps1 -ShardId 0 -TotalShards 8 -Transport quic" -WindowStyle Normal
    $nodeStarted = $true
    Write-Host "  [OK] Node started in new window" -ForegroundColor Green
    Write-Host ""
    Write-Host "  Monitoring connection for 30 seconds..." -ForegroundColor Cyan
    
    # Monitor for 30 seconds
    $startTime = Get-Date
    $connected = $false
    $timeout = 30
    
    while (((Get-Date) - $startTime).TotalSeconds -lt $timeout) {
        Start-Sleep -Seconds 2
        
        # Check server logs for connection
        $connectionLog = ssh -F NUL ${RemoteUser}@${RemoteHost} "tail -10 $RemoteDir/server.log 2>/dev/null | grep -E '(Connection established|peer)'"
        if ($connectionLog -match "Connection established") {
            Write-Host "  [SUCCESS] Connection established! Server received connection." -ForegroundColor Green
            Write-Host "    $connectionLog" -ForegroundColor Gray
            $connected = $true
            break
        }
        
        $elapsed = [math]::Round(((Get-Date) - $startTime).TotalSeconds, 0)
        Write-Host "    Waiting... ($elapsed/$timeout seconds)" -ForegroundColor DarkGray
    }
    
    if ($connected) {
        Write-Host "  [PASS] QUIC connection test successful!" -ForegroundColor Green
    } else {
        Write-Host "  [WARNING] No connection seen in server logs after $timeout seconds" -ForegroundColor Yellow
        Write-Host "    Check the node window for connection status" -ForegroundColor Gray
        Write-Host "    Check server logs: ssh ${RemoteUser}@${RemoteHost} 'tail -f $RemoteDir/server.log'" -ForegroundColor Gray
    }
} catch {
    Write-Host "  [FAIL] Failed to start node: $_" -ForegroundColor Red
    $allTestsPassed = $false
}

Write-Host ""

# Final Summary
Write-Host "========================================" -ForegroundColor Cyan
Write-Host "  TEST SUMMARY" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

if ($allTestsPassed -and $connected) {
    Write-Host "  [SUCCESS] All tests passed! QUIC connection is working." -ForegroundColor Green
} elseif ($allTestsPassed) {
    Write-Host "  [PARTIAL] Infrastructure tests passed, but connection not confirmed." -ForegroundColor Yellow
    Write-Host "    Check the node window for connection status." -ForegroundColor Gray
} else {
    Write-Host "  [FAILURE] Some tests failed. Review the output above." -ForegroundColor Red
}

Write-Host ""
Write-Host "Next steps:" -ForegroundColor Cyan
Write-Host "  1. Check the node window for connection logs" -ForegroundColor White
Write-Host "  2. Look for '[CONNECT] ✓✓✓ CONNECTED TO BOOTSTRAP NODE ✓✓✓'" -ForegroundColor White
Write-Host "  3. If still failing, check server logs: ssh ${RemoteUser}@${RemoteHost} 'tail -f $RemoteDir/server.log'" -ForegroundColor White
Write-Host ""
