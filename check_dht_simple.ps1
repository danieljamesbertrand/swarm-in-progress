# Simple DHT Status Check - Uses HTTP if available, otherwise provides manual instructions

Write-Host ""
Write-Host "================================================================" -ForegroundColor Cyan
Write-Host "  DHT DISCOVERY STATUS CHECK" -ForegroundColor Cyan
Write-Host "================================================================" -ForegroundColor Cyan
Write-Host ""

# Check processes
$bootstrap = Get-Process -Name "server" -ErrorAction SilentlyContinue
$web = Get-Process -Name "web_server" -ErrorAction SilentlyContinue
$nodes = Get-Process -Name "shard_listener" -ErrorAction SilentlyContinue
$nodeCount = if ($nodes) { $nodes.Count } else { 0 }

Write-Host "[STATUS] System Components:" -ForegroundColor Yellow
Write-Host "  Bootstrap: $(if ($bootstrap) { 'RUNNING' } else { 'NOT RUNNING' })" -ForegroundColor $(if ($bootstrap) { 'Green' } else { 'Red' })
Write-Host "  Web Server: $(if ($web) { 'RUNNING' } else { 'NOT RUNNING' })" -ForegroundColor $(if ($web) { 'Green' } else { 'Red' })
Write-Host "  Nodes: $nodeCount/4" -ForegroundColor $(if ($nodeCount -eq 4) { 'Green' } elseif ($nodeCount -gt 0) { 'Yellow' } else { 'Red' })
Write-Host ""

# Check web server
if ($web) {
    Write-Host "[CHECK] Testing web server connection..." -ForegroundColor Yellow
    try {
        $response = Invoke-WebRequest -Uri "http://localhost:8080" -TimeoutSec 3 -UseBasicParsing -ErrorAction Stop
        Write-Host "  [SUCCESS] Web server responding" -ForegroundColor Green
        Write-Host ""
        Write-Host "[INFO] To check DHT status:" -ForegroundColor Yellow
        Write-Host "  1. Open browser: http://localhost:8080" -ForegroundColor White
        Write-Host "  2. Check the Pipeline Status section" -ForegroundColor White
        Write-Host "  3. Look for 'Online Nodes: X/4'" -ForegroundColor White
        Write-Host ""
    } catch {
        Write-Host "  [FAILED] Web server not responding" -ForegroundColor Red
    }
} else {
    Write-Host "[WARNING] Web server not running - cannot check status" -ForegroundColor Yellow
}

Write-Host ""
Write-Host "================================================================" -ForegroundColor Cyan
Write-Host "  MANUAL CONSOLE CHECK REQUIRED" -ForegroundColor Cyan
Write-Host "================================================================" -ForegroundColor Cyan
Write-Host ""
Write-Host "Since DHT discovery is not working (0 nodes found), check:" -ForegroundColor Yellow
Write-Host ""
Write-Host "1. NODE CONSOLE WINDOWS (4 windows for shard_listener):" -ForegroundColor Cyan
Write-Host "   Look for this message:" -ForegroundColor White
Write-Host "     [DHT] ANNOUNCED SHARD X TO DHT" -ForegroundColor Green
Write-Host ""
Write-Host "   If you DON'T see it:" -ForegroundColor Yellow
Write-Host "     - Node may not have bootstrapped to DHT" -ForegroundColor Gray
Write-Host "     - Check for: [DHT] Started Kademlia bootstrap" -ForegroundColor Gray
Write-Host "     - Check for: [DHT] Routing updated" -ForegroundColor Gray
Write-Host ""
Write-Host "2. WEB SERVER CONSOLE WINDOW:" -ForegroundColor Cyan
Write-Host "   Look for these messages:" -ForegroundColor White
Write-Host "     [DHT] Querying for 4 shards..." -ForegroundColor Green
Write-Host "     [DHT] Discovered shard X from {peer_id}" -ForegroundColor Green
Write-Host ""
Write-Host "   If you see 'Querying' but no 'Discovered':" -ForegroundColor Yellow
Write-Host "     - Coordinator is querying but not finding records" -ForegroundColor Gray
Write-Host "     - DHT routing may be broken" -ForegroundColor Gray
Write-Host ""
Write-Host "3. BOOTSTRAP SERVER CONSOLE WINDOW:" -ForegroundColor Cyan
Write-Host "   Look for:" -ForegroundColor White
Write-Host "     ConnectionEstablished events (good)" -ForegroundColor Green
Write-Host "     RoutingUpdated events (good)" -ForegroundColor Green
Write-Host "     UnroutablePeer errors (bad - routing issue)" -ForegroundColor Red
Write-Host ""
Write-Host "================================================================" -ForegroundColor Cyan
Write-Host ""
Write-Host "QUICK FIX: If nodes are running but not discovered:" -ForegroundColor Yellow
Write-Host "  1. Restart all processes (bootstrap, web server, nodes)" -ForegroundColor White
Write-Host "  2. Wait 30-60 seconds for DHT to populate" -ForegroundColor White
Write-Host "  3. Check console windows for announcement messages" -ForegroundColor White
Write-Host ""

