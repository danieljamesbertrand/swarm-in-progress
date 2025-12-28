# Start All 4 Shard Nodes for Full Pipeline
Write-Host ""
Write-Host "========================================" -ForegroundColor Cyan
Write-Host "  STARTING ALL 4 SHARD NODES" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

# Check bootstrap
Write-Host "[1/5] Checking bootstrap server..." -ForegroundColor Yellow
$bootstrap = Get-Process | Where-Object {$_.ProcessName -eq "server"} -ErrorAction SilentlyContinue
if (-not $bootstrap) {
    Write-Host "  [WARN] Bootstrap server not running" -ForegroundColor Yellow
    Write-Host "  Starting bootstrap server..." -ForegroundColor Gray
    Start-Process powershell -ArgumentList "-NoExit", "-Command", "cd '$PWD'; cargo run --bin server -- --listen-addr 0.0.0.0 --port 51820" -WindowStyle Minimized
    Start-Sleep -Seconds 5
    Write-Host "  [OK] Bootstrap started" -ForegroundColor Green
} else {
    Write-Host "  [OK] Bootstrap running (PID: $($bootstrap.Id))" -ForegroundColor Green
}

# Kill existing shard nodes
Write-Host ""
Write-Host "[2/5] Cleaning up existing shard nodes..." -ForegroundColor Yellow
$existing = Get-Process | Where-Object {$_.ProcessName -eq "shard_listener"} -ErrorAction SilentlyContinue
if ($existing) {
    Write-Host "  Stopping $($existing.Count) existing shard node(s)..." -ForegroundColor Gray
    $existing | Stop-Process -Force -ErrorAction SilentlyContinue
    Start-Sleep -Seconds 2
}
Write-Host "  [OK] Cleanup complete" -ForegroundColor Green

# Start all 4 shard nodes
Write-Host ""
Write-Host "[3/5] Starting 4 shard nodes..." -ForegroundColor Yellow
$bootstrapAddr = "/ip4/127.0.0.1/tcp/51820"
$cluster = "llama-cluster"
$totalShards = 4
$totalLayers = 32
$modelName = "llama-8b"
$shardsDir = "models_cache/shards"

for ($i = 0; $i -lt 4; $i++) {
    $port = 51821 + $i
    Write-Host "  Starting shard $i on port $port..." -ForegroundColor Gray
    
    $command = "cd '$PWD'; Write-Host '=== SHARD NODE $i ===' -ForegroundColor Cyan; `$env:LLAMA_SHARD_ID='$i'; `$env:LLAMA_TOTAL_SHARDS='$totalShards'; `$env:LLAMA_TOTAL_LAYERS='$totalLayers'; `$env:LLAMA_MODEL_NAME='$modelName'; cargo run --bin shard_listener -- --bootstrap $bootstrapAddr --cluster $cluster --shard-id $i --total-shards $totalShards --total-layers $totalLayers --model-name $modelName --port $port --shards-dir $shardsDir"
    
    Start-Process powershell -ArgumentList "-NoExit", "-Command", $command -WindowStyle Normal
    Start-Sleep -Seconds 3
}

Write-Host "  [OK] All 4 shard nodes starting" -ForegroundColor Green

# Wait for nodes to start
Write-Host ""
Write-Host "[4/5] Waiting for nodes to register (30 seconds)..." -ForegroundColor Yellow
$maxWait = 30
$elapsed = 0
$nodeCount = 0

while ($elapsed -lt $maxWait) {
    Start-Sleep -Seconds 2
    $elapsed += 2
    $nodes = Get-Process | Where-Object {$_.ProcessName -eq "shard_listener"} -ErrorAction SilentlyContinue
    $currentCount = if ($nodes) { $nodes.Count } else { 0 }
    
    if ($currentCount -gt $nodeCount) {
        Write-Host "  $currentCount/4 nodes running..." -ForegroundColor Cyan
        $nodeCount = $currentCount
    }
    
    if ($nodeCount -eq 4) {
        Write-Host "  [OK] All 4 nodes are running!" -ForegroundColor Green
        break
    }
    
    if ($elapsed % 10 -eq 0) {
        Write-Host "  Waiting... ($elapsed seconds, $currentCount/4 nodes)" -ForegroundColor Gray
    }
}

# Final status
Write-Host ""
Write-Host "[5/5] Final Status" -ForegroundColor Yellow
$finalNodes = Get-Process | Where-Object {$_.ProcessName -eq "shard_listener"} -ErrorAction SilentlyContinue
$finalCount = if ($finalNodes) { $finalNodes.Count } else { 0 }

Write-Host "  Shard nodes running: $finalCount/4" -ForegroundColor $(if ($finalCount -eq 4) { 'Green' } elseif ($finalCount -gt 0) { 'Yellow' } else { 'Red' })

if ($finalCount -eq 4) {
    Write-Host ""
    Write-Host "========================================" -ForegroundColor Cyan
    Write-Host "  ALL 4 SHARDS ARE RUNNING!" -ForegroundColor Green
    Write-Host "========================================" -ForegroundColor Cyan
    Write-Host ""
    Write-Host "Pipeline Status:" -ForegroundColor Yellow
    Write-Host "  Shard 0: layers 0-7 (embeddings)" -ForegroundColor Green
    Write-Host "  Shard 1: layers 8-15" -ForegroundColor Green
    Write-Host "  Shard 2: layers 16-23" -ForegroundColor Green
    Write-Host "  Shard 3: layers 24-31 (output)" -ForegroundColor Green
    Write-Host ""
    Write-Host "The web UI should now show 4/4 nodes online" -ForegroundColor Cyan
    Write-Host "Full pipeline inference is now available!" -ForegroundColor Green
} else {
    Write-Host ""
    Write-Host "  [WARN] Only $finalCount/4 nodes are running" -ForegroundColor Yellow
    Write-Host "  Nodes may still be compiling or starting up" -ForegroundColor Gray
    Write-Host "  Check the terminal windows for each shard node" -ForegroundColor Gray
    Write-Host "  Wait a bit longer and refresh the web UI" -ForegroundColor Gray
}

Write-Host ""
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

