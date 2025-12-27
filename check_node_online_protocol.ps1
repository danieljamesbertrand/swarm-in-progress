# Check if nodes are following the online protocol
Write-Host "`n========================================" -ForegroundColor Cyan
Write-Host "  NODE ONLINE PROTOCOL DIAGNOSTIC" -ForegroundColor Cyan
Write-Host "========================================`n" -ForegroundColor Cyan

Write-Host "Checking for shard nodes..." -ForegroundColor Yellow
$nodes = Get-Process | Where-Object {$_.ProcessName -eq "shard_listener"}
if ($nodes.Count -eq 0) {
    Write-Host "  ❌ No shard nodes running" -ForegroundColor Red
    exit 1
}
Write-Host "  ✓ Found $($nodes.Count) shard node(s)" -ForegroundColor Green

Write-Host "`nProtocol Requirements:" -ForegroundColor Yellow
Write-Host "  1. Node must bootstrap to DHT" -ForegroundColor White
Write-Host "  2. Node must add addresses to Kademlia" -ForegroundColor White
Write-Host "  3. Node must receive RoutingUpdated event" -ForegroundColor White
Write-Host "  4. Node must call put_record to DHT" -ForegroundColor White
Write-Host "  5. Coordinator must query DHT with get_record" -ForegroundColor White
Write-Host "  6. Coordinator must receive FoundRecord event" -ForegroundColor White
Write-Host "  7. Record must pass freshness check" -ForegroundColor White
Write-Host "  8. Node appears in get_pipeline" -ForegroundColor White

Write-Host "`nTo verify:" -ForegroundColor Yellow
Write-Host "  - Check node console windows for: [DHT] ✓✓✓ ANNOUNCED SHARD X TO DHT ✓✓✓" -ForegroundColor Gray
Write-Host "  - Check web server console for: [DHT] ✓ Discovered shard X from <peer_id>" -ForegroundColor Gray
Write-Host "  - Check bootstrap console for: RoutingUpdated (not UnroutablePeer)" -ForegroundColor Gray

Write-Host "`nCurrent Status:" -ForegroundColor Yellow
$web = Get-Process | Where-Object {$_.ProcessName -eq "web_server"} | Select-Object -First 1
$server = Get-Process | Where-Object {$_.ProcessName -eq "server"} | Select-Object -First 1
Write-Host "  Bootstrap Server: $(if ($server) { "RUNNING" } else { "NOT RUNNING" })" -ForegroundColor $(if ($server) { 'Green' } else { 'Red' })
Write-Host "  Web Server:       $(if ($web) { "RUNNING" } else { "NOT RUNNING" })" -ForegroundColor $(if ($web) { 'Green' } else { 'Red' })
Write-Host "  Shard Nodes:      $($nodes.Count)/4" -ForegroundColor $(if ($nodes.Count -ge 4) { 'Green' } else { 'Yellow' })

Write-Host "`nSee SHARD_NODE_ONLINE_PROTOCOL.md for complete protocol details" -ForegroundColor Gray
Write-Host ""

