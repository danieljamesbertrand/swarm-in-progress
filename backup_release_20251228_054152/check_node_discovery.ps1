# Check Node Discovery Status
Write-Host ""
Write-Host "=== NODE DISCOVERY DIAGNOSTIC ===" -ForegroundColor Cyan
Write-Host ""

# Check processes
Write-Host "[1/4] Checking processes..." -ForegroundColor Yellow
$nodes = Get-Process | Where-Object {$_.ProcessName -eq "node"} -ErrorAction SilentlyContinue
$nodeCount = if ($nodes) { $nodes.Count } else { 0 }
Write-Host "  Node processes: $nodeCount" -ForegroundColor $(if($nodeCount -ge 6){'Green'}else{'Yellow'})

# Check if web server is responding
Write-Host ""
Write-Host "[2/4] Checking web server..." -ForegroundColor Yellow
try {
    $response = Invoke-WebRequest -Uri "http://localhost:8080" -TimeoutSec 3 -UseBasicParsing -ErrorAction Stop
    Write-Host "  [OK] Web server responding (Status: $($response.StatusCode))" -ForegroundColor Green
} catch {
    Write-Host "  [ERROR] Web server not responding: $($_.Exception.Message)" -ForegroundColor Red
}

# Check WebSocket
Write-Host ""
Write-Host "[3/4] Checking WebSocket..." -ForegroundColor Yellow
Write-Host "  WebSocket should be on: ws://localhost:8081" -ForegroundColor Gray
Write-Host "  (Cannot test WebSocket from PowerShell without additional tools)" -ForegroundColor Gray

# Expected configuration
Write-Host ""
Write-Host "[4/4] Expected Configuration:" -ForegroundColor Yellow
Write-Host "  Cluster name: llama-cluster" -ForegroundColor White
Write-Host "  Total shards: 4" -ForegroundColor White
Write-Host "  Bootstrap: /ip4/127.0.0.1/tcp/51820" -ForegroundColor White
Write-Host ""

Write-Host "=== DIAGNOSIS ===" -ForegroundColor Cyan
Write-Host ""
Write-Host "If nodes are not being discovered:" -ForegroundColor Yellow
Write-Host "  1. Check bootstrap server is running" -ForegroundColor White
Write-Host "  2. Check shard node terminal windows for:" -ForegroundColor White
Write-Host "     - '[DHT] ✓ Started Kademlia bootstrap'" -ForegroundColor Gray
Write-Host "     - '[DHT] ✓✓✓ ANNOUNCED SHARD X TO DHT ✓✓✓'" -ForegroundColor Gray
Write-Host "  3. Check web server terminal window for:" -ForegroundColor White
Write-Host "     - '[DHT] ✓ Started Kademlia bootstrap'" -ForegroundColor Gray
Write-Host "     - '[DHT] Querying for 4 shards...'" -ForegroundColor Gray
Write-Host "     - '[DHT] ✓ Discovered shard X...'" -ForegroundColor Gray
Write-Host ""
Write-Host "Timing: Nodes need 10-30 seconds to:" -ForegroundColor Yellow
Write-Host "  - Bootstrap to DHT" -ForegroundColor White
Write-Host "  - Announce their shards" -ForegroundColor White
Write-Host "  - Be discovered by web server" -ForegroundColor White
Write-Host ""
Write-Host "If nodes still aren't discovered after 60 seconds," -ForegroundColor Yellow
Write-Host "there may be a DHT connectivity issue." -ForegroundColor Yellow
Write-Host ""

