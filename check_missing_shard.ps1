# Check which shard is missing by examining web server output
# This script helps identify which of the 4 shards (0, 1, 2, 3) is not running

Write-Host "=== CHECKING FOR MISSING SHARD ===" -ForegroundColor Cyan
Write-Host ""

# Check running nodes
$nodes = Get-Process | Where-Object {$_.ProcessName -eq "shard_listener"}
$nodeCount = ($nodes | Measure-Object).Count

Write-Host "Currently Running: $nodeCount/4 shard nodes" -ForegroundColor $(if($nodeCount -eq 4){'Green'}elseif($nodeCount -gt 0){'Yellow'}else{'Red'})
Write-Host ""

if ($nodeCount -lt 4) {
    Write-Host "Missing: $((4 - $nodeCount)) node(s)" -ForegroundColor Yellow
    Write-Host ""
    Write-Host "To identify which shard is missing:" -ForegroundColor Cyan
    Write-Host "  1. Check the web server console window" -ForegroundColor White
    Write-Host "  2. Look for messages like:" -ForegroundColor White
    Write-Host "     - '[COORDINATOR] ✗ Failed to spawn node for shard X'" -ForegroundColor Gray
    Write-Host "     - '[COORDINATOR] ⚠️  Shard X node did not come online in time'" -ForegroundColor Gray
    Write-Host "     - 'Missing shard IDs: [X]'" -ForegroundColor Gray
    Write-Host ""
    Write-Host "  3. Check the web interface at http://localhost:8080" -ForegroundColor White
    Write-Host "     - Look at the pipeline stages (Shard 0, 1, 2, 3)" -ForegroundColor Gray
    Write-Host "     - Red/error stages indicate missing shards" -ForegroundColor Gray
    Write-Host ""
    Write-Host "Common causes:" -ForegroundColor Cyan
    Write-Host "  • Node still compiling (first run: 30-60 seconds)" -ForegroundColor White
    Write-Host "  • Node crashed during startup" -ForegroundColor White
    Write-Host "  • Port conflict or resource limit" -ForegroundColor White
    Write-Host "  • Bootstrap connection issue" -ForegroundColor White
} else {
    Write-Host "All 4 nodes are running!" -ForegroundColor Green
}

Write-Host ""

