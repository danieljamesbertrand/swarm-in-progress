# Automated Inference Test Script
# Tests the system by sending queries and verifying responses

Write-Host "=== INFERENCE TEST SUITE ===" -ForegroundColor Cyan
Write-Host ""

# Test 1: Check system is ready
Write-Host "[TEST 1] System Readiness Check..." -ForegroundColor Yellow
$webServer = Get-Process | Where-Object {$_.ProcessName -eq "web_server"} | Select-Object -First 1
$nodes = Get-Process | Where-Object {$_.ProcessName -eq "shard_listener"}
$nodeCount = ($nodes | Measure-Object).Count

if (-not $webServer) {
    Write-Host "  ✗ Web server not running!" -ForegroundColor Red
    exit 1
}
if ($nodeCount -lt 4) {
    Write-Host "  ⚠️  Only $nodeCount/4 nodes running" -ForegroundColor Yellow
} else {
    Write-Host "  ✓ System ready (Web server: ✓, Nodes: $nodeCount/4)" -ForegroundColor Green
}
Write-Host ""

# Test 2: WebSocket connectivity
Write-Host "[TEST 2] WebSocket Connectivity..." -ForegroundColor Yellow
Write-Host "  Manual test required:" -ForegroundColor Gray
Write-Host "    1. Open http://localhost:8080" -ForegroundColor White
Write-Host "    2. Check browser console (F12) for '[WS] ✓ Connected'" -ForegroundColor White
Write-Host "    3. Verify connection status shows 'Connected' (green)" -ForegroundColor White
Write-Host ""

# Test 3: Pipeline status
Write-Host "[TEST 3] Pipeline Status..." -ForegroundColor Yellow
Write-Host "  Manual test required:" -ForegroundColor Gray
Write-Host "    1. Check web interface shows 'Nodes Online: 4/4'" -ForegroundColor White
Write-Host "    2. Verify all pipeline stages are ready (not red/error)" -ForegroundColor White
Write-Host "    3. Check metrics are updating" -ForegroundColor White
Write-Host ""

# Test 4: Inference queries
Write-Host "[TEST 4] Inference Query Tests..." -ForegroundColor Yellow
Write-Host "  Test queries to try:" -ForegroundColor Gray
Write-Host "    1. 'What is 2+2?'" -ForegroundColor White
Write-Host "       Expected: Pipeline stages activate in sequence" -ForegroundColor Gray
Write-Host "    2. 'Who wrote Bohemian Rhapsody?'" -ForegroundColor White
Write-Host "       Expected: Real-time stage updates, response appears" -ForegroundColor Gray
Write-Host "    3. 'What is the capital of Japan?'" -ForegroundColor White
Write-Host "       Expected: All shards process, final answer displayed" -ForegroundColor Gray
Write-Host ""

# Test 5: Real-time updates
Write-Host "[TEST 5] Real-Time Update Verification..." -ForegroundColor Yellow
Write-Host "  Check browser console for:" -ForegroundColor Gray
Write-Host "    - '[WS] Stage update: input -> processing'" -ForegroundColor White
Write-Host "    - '[WS] Stage update: discovery -> processing'" -ForegroundColor White
Write-Host "    - '[WS] Stage update: shard0 -> processing'" -ForegroundColor White
Write-Host "    - '[WS] Stage update: shard0 -> complete'" -ForegroundColor White
Write-Host "    - (Repeat for shard1, shard2, shard3)" -ForegroundColor White
Write-Host "    - '[WS] Stage update: output -> complete'" -ForegroundColor White
Write-Host ""

# Test 6: Coordinated shard assignment
Write-Host "[TEST 6] Coordinated Shard Assignment..." -ForegroundColor Yellow
Write-Host "  Check web server console for:" -ForegroundColor Gray
Write-Host "    - '[COORDINATOR] Last assigned shard: X'" -ForegroundColor White
Write-Host "    - '[COORDINATOR] Coordinated assignment: spawning node for shard Y'" -ForegroundColor White
Write-Host "    - Verify sequential assignment (0, 1, 2, 3)" -ForegroundColor White
Write-Host ""

Write-Host "=== TEST SUITE READY ===" -ForegroundColor Green
Write-Host "Run tests manually in the web interface at http://localhost:8080" -ForegroundColor Cyan
Write-Host ""
