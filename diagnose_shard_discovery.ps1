# Diagnose why shards aren't coming online
Write-Host ""
Write-Host "========================================" -ForegroundColor Cyan
Write-Host "  SHARD DISCOVERY DIAGNOSTICS" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

# Check if processes are running
Write-Host "[1/5] Checking running processes..." -ForegroundColor Yellow
$bootstrap = Get-Process | Where-Object {$_.ProcessName -eq "server"} -ErrorAction SilentlyContinue
$webServer = Get-Process | Where-Object {$_.ProcessName -eq "web_server"} -ErrorAction SilentlyContinue
$shardNodes = Get-Process | Where-Object {$_.ProcessName -eq "shard_listener"} -ErrorAction SilentlyContinue

Write-Host "  Bootstrap Server: $(if ($bootstrap) { "[OK] PID: $($bootstrap.Id)" } else { "[MISSING]" })" -ForegroundColor $(if ($bootstrap) { 'Green' } else { 'Red' })
Write-Host "  Web Server:       $(if ($webServer) { "[OK] PID: $($webServer.Id)" } else { "[MISSING]" })" -ForegroundColor $(if ($webServer) { 'Green' } else { 'Red' })
Write-Host "  Shard Nodes:      $(if ($shardNodes) { "[OK] Count: $($shardNodes.Count)" } else { "[MISSING]" })" -ForegroundColor $(if ($shardNodes) { 'Green' } else { 'Red' })

if (-not $bootstrap -or -not $webServer -or -not $shardNodes) {
    Write-Host ""
    Write-Host "  [ERROR] Missing required processes!" -ForegroundColor Red
    Write-Host "  Start them with: powershell -ExecutionPolicy Bypass -File start_and_show_results.ps1" -ForegroundColor Yellow
    exit 1
}

# Check ports
Write-Host ""
Write-Host "[2/5] Checking ports..." -ForegroundColor Yellow
$ports = @{
    "51820" = "Bootstrap"
    "8080" = "Web Server HTTP"
    "8081" = "Web Server WebSocket"
}

foreach ($port in $ports.Keys) {
    $listening = netstat -ano | findstr ":$port" | findstr "LISTENING"
    $status = if ($listening) { "[LISTENING]" } else { "[NOT LISTENING]" }
    Write-Host "  Port $port ($($ports[$port])): $status" -ForegroundColor $(if ($listening) { 'Green' } else { 'Yellow' })
}

# Check DHT bootstrap status
Write-Host ""
Write-Host "[3/5] DHT Bootstrap Status..." -ForegroundColor Yellow
Write-Host "  Check web server terminal for:" -ForegroundColor Gray
Write-Host "    [DHT] Routing updated" -ForegroundColor White
Write-Host "    [DHT] Querying for 4 shards..." -ForegroundColor White
Write-Host ""

# Check shard announcement
Write-Host "[4/5] Shard Node Announcements..." -ForegroundColor Yellow
Write-Host "  Check shard node terminals for:" -ForegroundColor Gray
Write-Host "    [DHT] ✓✓✓ ANNOUNCED SHARD X TO DHT ✓✓✓" -ForegroundColor White
Write-Host ""

# Check discovery
Write-Host "[5/5] Shard Discovery..." -ForegroundColor Yellow
Write-Host "  Check web server terminal for:" -ForegroundColor Gray
Write-Host "    [DHT] Discovered shard X from peer_id" -ForegroundColor White
Write-Host "    [DHT] Pipeline status: X/4 nodes online" -ForegroundColor White
Write-Host ""

Write-Host "========================================" -ForegroundColor Cyan
Write-Host "  TROUBLESHOOTING STEPS" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""
Write-Host "If shards aren't coming online:" -ForegroundColor Yellow
Write-Host ""
Write-Host "1. Wait 10-20 seconds after starting nodes" -ForegroundColor White
Write-Host "   DHT discovery takes time to bootstrap and query" -ForegroundColor Gray
Write-Host ""
Write-Host "2. Check shard nodes announced to DHT:" -ForegroundColor White
Write-Host "   Look for: [DHT] ✓✓✓ ANNOUNCED SHARD X TO DHT ✓✓✓" -ForegroundColor Gray
Write-Host ""
Write-Host "3. Check web server is querying:" -ForegroundColor White
Write-Host "   Look for: [DHT] Querying for 4 shards..." -ForegroundColor Gray
Write-Host ""
Write-Host "4. Check web server discovered shards:" -ForegroundColor White
Write-Host "   Look for: [DHT] Discovered shard X from peer_id" -ForegroundColor Gray
Write-Host ""
Write-Host "5. Verify bootstrap connection:" -ForegroundColor White
Write-Host "   All nodes must connect to bootstrap on port 51820" -ForegroundColor Gray
Write-Host ""
Write-Host "6. Check cluster name matches:" -ForegroundColor White
Write-Host "   Shard nodes: --cluster llama-cluster" -ForegroundColor Gray
Write-Host "   Web server queries: llama-cluster" -ForegroundColor Gray
Write-Host ""
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

# Try to query pipeline status via WebSocket (if possible)
Write-Host "To check pipeline status, open:" -ForegroundColor Yellow
Write-Host "  http://localhost:8080" -ForegroundColor White
Write-Host ""
Write-Host "Or run the Rust client to see current status:" -ForegroundColor Yellow
Write-Host "  cargo run --example ai_query_client" -ForegroundColor White
Write-Host ""
