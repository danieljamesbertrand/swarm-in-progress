# Start 4 Shard Listener Nodes
# Each node handles a different shard (0, 1, 2, 3) of the model

Write-Host "=== Starting 4 Shard Listener Nodes ===" -ForegroundColor Cyan
Write-Host ""

# Bootstrap server address (must be running first)
$bootstrap = "/ip4/127.0.0.1/tcp/51820"
$cluster = "llama-cluster"
$totalShards = 4

# Start each node in a separate window
for ($i = 0; $i -lt 4; $i++) {
    Write-Host "[$($i+1)/4] Starting shard node $i..." -ForegroundColor Yellow
    
    $command = "cd '$PWD'; Write-Host '=== SHARD NODE $i ===' -ForegroundColor Cyan; cargo run --bin node -- shard-listener --bootstrap $bootstrap --cluster $cluster --shard-id $i --total-shards $totalShards"
    
    Start-Process powershell -ArgumentList "-NoExit", "-Command", $command -WindowStyle Normal
    
    Start-Sleep -Seconds 2
}

Write-Host ""
Write-Host "=== All 4 nodes starting ===" -ForegroundColor Green
Write-Host "Each node is running in a separate window." -ForegroundColor Cyan
Write-Host ""
Write-Host "To verify nodes are running:" -ForegroundColor Yellow
Write-Host '  Get-Process | Where-Object {$_.ProcessName -eq "shard_listener"}' -ForegroundColor Gray
Write-Host ""

