# Show Inference Results - Simple Guide
Write-Host ""
Write-Host "========================================" -ForegroundColor Cyan
Write-Host "  INFERENCE TEST - FULL OUTPUT GUIDE" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

# Check processes
Write-Host "[1/3] Checking system status..." -ForegroundColor Yellow
$webServer = Get-Process | Where-Object {$_.ProcessName -eq "web_server"} -ErrorAction SilentlyContinue
$nodes = Get-Process | Where-Object {$_.ProcessName -eq "shard_listener"} -ErrorAction SilentlyContinue
$bootstrap = Get-Process | Where-Object {$_.ProcessName -eq "server"} -ErrorAction SilentlyContinue

if ($webServer) {
    Write-Host "  [OK] Web server running (PID: $($webServer.Id))" -ForegroundColor Green
} else {
    Write-Host "  [WARN] Web server not running" -ForegroundColor Yellow
}

$nodeCount = if ($nodes) { $nodes.Count } else { 0 }
Write-Host "  Shard nodes: $nodeCount" -ForegroundColor $(if ($nodeCount -gt 0) { 'Green' } else { 'Yellow' })
if ($bootstrap) {
    Write-Host "  [OK] Bootstrap server running (PID: $($bootstrap.Id))" -ForegroundColor Green
}

Write-Host ""
Write-Host "[2/3] Testing HTTP endpoint..." -ForegroundColor Yellow
try {
    $response = Invoke-WebRequest -Uri "http://localhost:8080" -TimeoutSec 5 -UseBasicParsing -ErrorAction Stop
    Write-Host "  [OK] HTTP server responding (Status: $($response.StatusCode))" -ForegroundColor Green
} catch {
    Write-Host "  [ERROR] HTTP server not accessible" -ForegroundColor Red
    exit 1
}

Write-Host ""
Write-Host "[3/3] How to See Inference Results" -ForegroundColor Yellow
Write-Host ""
Write-Host "========================================" -ForegroundColor Cyan
Write-Host "  METHOD 1: Browser (Recommended)" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""
Write-Host "1. Open browser: http://localhost:8080" -ForegroundColor White
Write-Host "2. Wait 5-10 seconds for nodes to register" -ForegroundColor White
Write-Host "3. Type: 'what do a cat and a snake have in common'" -ForegroundColor Cyan
Write-Host "4. Click Send or press Enter" -ForegroundColor White
Write-Host "5. Results will appear below the input field" -ForegroundColor White
Write-Host ""
Write-Host "========================================" -ForegroundColor Cyan
Write-Host "  METHOD 2: Check Terminal Logs" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""
Write-Host "Web Server Terminal Output:" -ForegroundColor Yellow
Write-Host "  [WS] Processing query: what do a cat and a snake have in common" -ForegroundColor Gray
Write-Host "  [INFERENCE] Submitting inference request..." -ForegroundColor Gray
Write-Host "  [INFERENCE] Pipeline status: X/Y nodes online" -ForegroundColor Gray
Write-Host "  [INFERENCE] [OK] command_sender is set" -ForegroundColor Green
Write-Host "  [INFERENCE] Sending JSON command to node ..." -ForegroundColor Gray
Write-Host "  [P2P] Sending command EXECUTE_TASK to node ..." -ForegroundColor Gray
Write-Host "  [P2P] Received response" -ForegroundColor Gray
Write-Host "  [P2P] [OK] Matched response to waiting channel" -ForegroundColor Green
Write-Host "  [INFERENCE] Received JSON response from node ..." -ForegroundColor Gray
Write-Host "  [INFERENCE] [OK] Shard 0 completed" -ForegroundColor Green
Write-Host "  [WS] Query processed, sending response" -ForegroundColor Gray
Write-Host ""
Write-Host "Shard Node Terminal Output:" -ForegroundColor Yellow
Write-Host "  [COMMAND] [OK] Validation passed" -ForegroundColor Gray
Write-Host "  [INFERENCE] Processing inference request..." -ForegroundColor Gray
Write-Host "  [RESPONSE] Sending response to peer: ..." -ForegroundColor Gray
Write-Host "  [RESPONSE] [OK] Response sent successfully" -ForegroundColor Green
Write-Host ""
Write-Host "========================================" -ForegroundColor Cyan
Write-Host "  KEY SUCCESS INDICATORS" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""
Write-Host "[OK] [P2P] Matched response to waiting channel" -ForegroundColor Green
Write-Host "    This confirms the RequestId matching fix is working!" -ForegroundColor Gray
Write-Host ""
Write-Host "[OK] [RESPONSE] Response sent successfully" -ForegroundColor Green
Write-Host "    This confirms the shard node processed and sent the response" -ForegroundColor Gray
Write-Host ""
Write-Host "[OK] [INFERENCE] Shard 0 completed" -ForegroundColor Green
Write-Host "    This confirms the coordinator received and processed the response" -ForegroundColor Gray
Write-Host ""
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""
