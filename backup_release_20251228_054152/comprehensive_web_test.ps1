# Comprehensive Web App Testing Script
# Tests every aspect of the web application functionality

Write-Host "╔══════════════════════════════════════════════════════════════╗" -ForegroundColor Cyan
Write-Host "║     COMPREHENSIVE WEB APP FUNCTIONALITY TEST                ║" -ForegroundColor Cyan
Write-Host "╚══════════════════════════════════════════════════════════════╝" -ForegroundColor Cyan
Write-Host ""

$ErrorActionPreference = "Continue"

# Kill any existing processes
Write-Host "[TEST] Killing existing processes..." -ForegroundColor Yellow
Get-Process | Where-Object {$_.ProcessName -like "*punch-simple*" -or $_.ProcessName -like "*shard_listener*" -or $_.ProcessName -like "*web_server*"} | Stop-Process -Force -ErrorAction SilentlyContinue
Start-Sleep -Seconds 2

# Build the project
Write-Host "[TEST] Building project..." -ForegroundColor Yellow
cargo build --release
if ($LASTEXITCODE -ne 0) {
    Write-Host "[TEST] ❌ Build failed!" -ForegroundColor Red
    exit 1
}
Write-Host "[TEST] ✓ Build successful" -ForegroundColor Green

# Start web server
Write-Host ""
Write-Host "[TEST] Starting web server..." -ForegroundColor Yellow
$webServer = Start-Process -FilePath "target\release\web_server.exe" -PassThru -NoNewWindow
Start-Sleep -Seconds 3

# Check if web server is running
if (-not (Get-Process -Id $webServer.Id -ErrorAction SilentlyContinue)) {
    Write-Host "[TEST] ❌ Web server failed to start!" -ForegroundColor Red
    exit 1
}
Write-Host "[TEST] ✓ Web server started (PID: $($webServer.Id))" -ForegroundColor Green

# Start 4 shard listener nodes
Write-Host ""
Write-Host "[TEST] Starting 4 shard listener nodes..." -ForegroundColor Yellow
$nodes = @()
for ($i = 0; $i -lt 4; $i++) {
    $node = Start-Process -FilePath "target\release\shard_listener.exe" `
        -ArgumentList "--bootstrap", "/ip4/127.0.0.1/tcp/51820", "--namespace", "llama-cluster", "--shard-id", $i, "--total-shards", "4" `
        -PassThru -NoNewWindow
    $nodes += $node
    Write-Host "[TEST]   Started node $i (PID: $($node.Id))" -ForegroundColor Gray
    Start-Sleep -Seconds 1
}
Write-Host "[TEST] ✓ All 4 nodes started" -ForegroundColor Green

# Wait for nodes to connect
Write-Host ""
Write-Host "[TEST] Waiting for nodes to connect to DHT..." -ForegroundColor Yellow
Start-Sleep -Seconds 10

# Test 1: Check if web server is listening
Write-Host ""
Write-Host "[TEST 1] Testing web server HTTP endpoint..." -ForegroundColor Cyan
try {
    $response = Invoke-WebRequest -Uri "http://localhost:8080" -Method GET -TimeoutSec 5 -UseBasicParsing
    if ($response.StatusCode -eq 200) {
        Write-Host "[TEST 1] ✓ Web server responding (Status: $($response.StatusCode))" -ForegroundColor Green
    } else {
        Write-Host "[TEST 1] ⚠️  Web server responded with status: $($response.StatusCode)" -ForegroundColor Yellow
    }
} catch {
    Write-Host "[TEST 1] ❌ Web server not responding: $_" -ForegroundColor Red
}

# Test 2: Check if AI console page loads
Write-Host ""
Write-Host "[TEST 2] Testing AI console page..." -ForegroundColor Cyan
try {
    $response = Invoke-WebRequest -Uri "http://localhost:8080/ai-console.html" -Method GET -TimeoutSec 5 -UseBasicParsing
    if ($response.StatusCode -eq 200) {
        Write-Host "[TEST 2] ✓ AI console page loads (Status: $($response.StatusCode))" -ForegroundColor Green
        if ($response.Content -match "queryInput|textarea") {
            Write-Host "[TEST 2] ✓ Input field found in HTML" -ForegroundColor Green
        } else {
            Write-Host "[TEST 2] ⚠️  Input field not found in HTML" -ForegroundColor Yellow
        }
        if ($response.Content -match "nodeLogContainer|node-log") {
            Write-Host "[TEST 2] ✓ Scrolling log container found in HTML" -ForegroundColor Green
        } else {
            Write-Host "[TEST 2] ⚠️  Scrolling log container not found in HTML" -ForegroundColor Yellow
        }
    } else {
        Write-Host "[TEST 2] ❌ AI console page failed (Status: $($response.StatusCode))" -ForegroundColor Red
    }
} catch {
    Write-Host "[TEST 2] ❌ AI console page not accessible: $_" -ForegroundColor Red
}

# Test 3: Check if admin panel exists
Write-Host ""
Write-Host "[TEST 3] Testing admin panel page..." -ForegroundColor Cyan
try {
    $response = Invoke-WebRequest -Uri "http://localhost:8080/admin.html" -Method GET -TimeoutSec 5 -UseBasicParsing
    if ($response.StatusCode -eq 200) {
        Write-Host "[TEST 3] ✓ Admin panel page loads (Status: $($response.StatusCode))" -ForegroundColor Green
    } else {
        Write-Host "[TEST 3] ⚠️  Admin panel responded with status: $($response.StatusCode)" -ForegroundColor Yellow
    }
} catch {
    Write-Host "[TEST 3] ⚠️  Admin panel not accessible (may not exist): $_" -ForegroundColor Yellow
}

# Test 4: Check WebSocket endpoint
Write-Host ""
Write-Host "[TEST 4] Testing WebSocket endpoint..." -ForegroundColor Cyan
try {
    # Try to connect to WebSocket (basic check)
    $wsTest = Test-NetConnection -ComputerName localhost -Port 8081 -InformationLevel Quiet
    if ($wsTest) {
        Write-Host "[TEST 4] ✓ WebSocket port 8081 is open" -ForegroundColor Green
    } else {
        Write-Host "[TEST 4] ❌ WebSocket port 8081 is not accessible" -ForegroundColor Red
    }
} catch {
    Write-Host "[TEST 4] ⚠️  Could not test WebSocket: $_" -ForegroundColor Yellow
}

# Test 5: Check node processes are still running
Write-Host ""
Write-Host "[TEST 5] Verifying all processes are running..." -ForegroundColor Cyan
$webServerRunning = Get-Process -Id $webServer.Id -ErrorAction SilentlyContinue
if ($webServerRunning) {
    Write-Host "[TEST 5] ✓ Web server still running" -ForegroundColor Green
} else {
    Write-Host "[TEST 5] ❌ Web server crashed!" -ForegroundColor Red
}

$runningNodes = 0
foreach ($node in $nodes) {
    if (Get-Process -Id $node.Id -ErrorAction SilentlyContinue) {
        $runningNodes++
    }
}
Write-Host "[TEST 5] ✓ $runningNodes/4 nodes still running" -ForegroundColor $(if ($runningNodes -eq 4) { "Green" } else { "Yellow" })

# Test 6: Check for compilation errors
Write-Host ""
Write-Host "[TEST 6] Checking for compilation errors..." -ForegroundColor Cyan
cargo check --bin web_server 2>&1 | Out-Null
if ($LASTEXITCODE -eq 0) {
    Write-Host "[TEST 6] ✓ No compilation errors in web_server" -ForegroundColor Green
} else {
    Write-Host "[TEST 6] ❌ Compilation errors found!" -ForegroundColor Red
    cargo check --bin web_server 2>&1 | Select-String "error\[" | ForEach-Object { Write-Host "  $_" -ForegroundColor Red }
}

# Summary
Write-Host ""
Write-Host "╔══════════════════════════════════════════════════════════════╗" -ForegroundColor Cyan
Write-Host "║                    TEST SUMMARY                              ║" -ForegroundColor Cyan
Write-Host "╚══════════════════════════════════════════════════════════════╝" -ForegroundColor Cyan
Write-Host ""
Write-Host "Web Server: http://localhost:8080" -ForegroundColor White
Write-Host "WebSocket:  ws://localhost:8081" -ForegroundColor White
Write-Host "AI Console: http://localhost:8080/ai-console.html" -ForegroundColor White
Write-Host ""
Write-Host "Next Steps:" -ForegroundColor Yellow
Write-Host "  1. Open http://localhost:8080/ai-console.html in browser" -ForegroundColor White
Write-Host "  2. Check browser console (F12) for WebSocket connection" -ForegroundColor White
Write-Host "  3. Verify input field is visible" -ForegroundColor White
Write-Host "  4. Verify scrolling log area is visible" -ForegroundColor White
Write-Host "  5. Submit a test query and watch for:" -ForegroundColor White
Write-Host "     - Preload messages" -ForegroundColor Gray
Write-Host "     - Node inference requests in scrolling log" -ForegroundColor Gray
Write-Host "     - Pipeline stage updates" -ForegroundColor Gray
Write-Host "     - Response display" -ForegroundColor Gray
Write-Host ""
Write-Host "Press Ctrl+C to stop all processes" -ForegroundColor Yellow

# Keep processes running
Write-Host ""
Write-Host "Processes will continue running. Press Ctrl+C to stop..." -ForegroundColor Yellow
try {
    while ($true) {
        Start-Sleep -Seconds 5
        # Check if processes are still running
        if (-not (Get-Process -Id $webServer.Id -ErrorAction SilentlyContinue)) {
            Write-Host "[MONITOR] ⚠️  Web server stopped!" -ForegroundColor Yellow
            break
        }
    }
} catch {
    Write-Host "[MONITOR] Stopping..." -ForegroundColor Yellow
} finally {
    Write-Host ""
    Write-Host "[CLEANUP] Stopping all processes..." -ForegroundColor Yellow
    Get-Process | Where-Object {$_.ProcessName -like "*punch-simple*" -or $_.ProcessName -like "*shard_listener*" -or $_.ProcessName -like "*web_server*"} | Stop-Process -Force -ErrorAction SilentlyContinue
    Write-Host "[CLEANUP] ✓ All processes stopped" -ForegroundColor Green
}


