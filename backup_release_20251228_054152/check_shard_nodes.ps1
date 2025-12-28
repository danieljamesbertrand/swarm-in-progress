# Check Shard Nodes Status
Write-Host ""
Write-Host "========================================" -ForegroundColor Cyan
Write-Host "  SHARD NODES STATUS" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

# Check running shard nodes
Write-Host "[1/3] Checking running shard nodes..." -ForegroundColor Yellow
$shardNodes = Get-Process | Where-Object {$_.ProcessName -eq "shard_listener"} -ErrorAction SilentlyContinue
$nodeCount = if ($shardNodes) { $shardNodes.Count } else { 0 }

Write-Host "  Running shard nodes: $nodeCount/4" -ForegroundColor $(if ($nodeCount -eq 4) { 'Green' } elseif ($nodeCount -gt 0) { 'Yellow' } else { 'Red' })

if ($shardNodes) {
    Write-Host "  Node PIDs:" -ForegroundColor Gray
    $shardNodes | ForEach-Object {
        Write-Host "    - PID: $($_.Id)" -ForegroundColor Gray
    }
} else {
    Write-Host "  [ERROR] No shard nodes are running!" -ForegroundColor Red
}

# Check expected shards
Write-Host ""
Write-Host "[2/3] Expected shard configuration..." -ForegroundColor Yellow
Write-Host "  Total shards: 4 (shard 0, 1, 2, 3)" -ForegroundColor White
Write-Host "  Each shard handles 8 layers (32 total layers / 4 shards)" -ForegroundColor White
Write-Host "  Shard 0: layers 0-7 (embeddings)" -ForegroundColor Gray
Write-Host "  Shard 1: layers 8-15" -ForegroundColor Gray
Write-Host "  Shard 2: layers 16-23" -ForegroundColor Gray
Write-Host "  Shard 3: layers 24-31 (output)" -ForegroundColor Gray

# Why only 1 is online
Write-Host ""
Write-Host "[3/3] Why only 1/4 shards are online:" -ForegroundColor Yellow
if ($nodeCount -eq 1) {
    Write-Host "  [INFO] Only 1 shard node was started (shard 0)" -ForegroundColor Yellow
    Write-Host "  This is expected for single-node testing" -ForegroundColor Gray
    Write-Host ""
    Write-Host "  For full pipeline (4/4 shards), you need to start:" -ForegroundColor White
    Write-Host "    - Shard 0 (already running)" -ForegroundColor Green
    Write-Host "    - Shard 1 (not running)" -ForegroundColor Red
    Write-Host "    - Shard 2 (not running)" -ForegroundColor Red
    Write-Host "    - Shard 3 (not running)" -ForegroundColor Red
} elseif ($nodeCount -eq 0) {
    Write-Host "  [ERROR] No shard nodes are running!" -ForegroundColor Red
    Write-Host "  You need to start at least shard 0 for inference to work" -ForegroundColor Yellow
} else {
    Write-Host "  [INFO] $nodeCount shard nodes are running" -ForegroundColor Yellow
    Write-Host "  Missing: $((4 - $nodeCount)) shard node(s)" -ForegroundColor Yellow
}

Write-Host ""
Write-Host "========================================" -ForegroundColor Cyan
Write-Host "  RECOMMENDATIONS" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

if ($nodeCount -eq 1) {
    Write-Host "For single-node testing (current setup):" -ForegroundColor Yellow
    Write-Host "  [OK] 1/4 shards is sufficient for basic inference" -ForegroundColor Green
    Write-Host "  The pipeline coordinator will use the available shard" -ForegroundColor Gray
    Write-Host "  Inference will work, but may be limited" -ForegroundColor Gray
    Write-Host ""
    Write-Host "For full pipeline (4/4 shards):" -ForegroundColor Yellow
    Write-Host "  Run: .\start_4_nodes.ps1" -ForegroundColor White
    Write-Host "  Or start each shard manually:" -ForegroundColor White
    Write-Host "    Shard 0: cargo run --bin shard_listener -- --shard-id 0 ..." -ForegroundColor Gray
    Write-Host "    Shard 1: cargo run --bin shard_listener -- --shard-id 1 ..." -ForegroundColor Gray
    Write-Host "    Shard 2: cargo run --bin shard_listener -- --shard-id 2 ..." -ForegroundColor Gray
    Write-Host "    Shard 3: cargo run --bin shard_listener -- --shard-id 3 ..." -ForegroundColor Gray
} elseif ($nodeCount -eq 0) {
    Write-Host "[ACTION] Start at least shard 0:" -ForegroundColor Yellow
    Write-Host "  `$env:LLAMA_SHARD_ID='0'" -ForegroundColor White
    Write-Host "  cargo run --bin shard_listener -- --bootstrap /ip4/127.0.0.1/tcp/51820 --cluster llama-cluster --shard-id 0 --total-shards 4 --total-layers 32 --model-name llama-8b --port 51821 --shards-dir models_cache/shards" -ForegroundColor White
} else {
    Write-Host "To start remaining shards:" -ForegroundColor Yellow
    $missing = 4 - $nodeCount
    Write-Host "  Start $missing more shard node(s)" -ForegroundColor White
}

Write-Host ""
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

