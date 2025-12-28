# Check Tensor Loading Status
Write-Host ""
Write-Host "========================================" -ForegroundColor Cyan
Write-Host "  TENSOR LOADING STATUS CHECK" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

# Check shard files
Write-Host "[1/3] Checking shard files..." -ForegroundColor Yellow
$shardFiles = @()
for ($i = 0; $i -lt 4; $i++) {
    $shardFile = "models_cache/shards/shard-$i.gguf"
    if (Test-Path $shardFile) {
        $size = (Get-Item $shardFile).Length / 1MB
        $sizeGB = [math]::Round($size / 1024, 2)
        $sizeRounded = [math]::Round($size, 2)
        Write-Host "  [OK] Shard ${i}: ${sizeRounded} MB ($sizeGB GB)" -ForegroundColor Green
        $shardFiles += $i
    } else {
        Write-Host "  [MISSING] Shard ${i}: NOT FOUND" -ForegroundColor Red
    }
}

if ($shardFiles.Count -eq 0) {
    Write-Host ""
    Write-Host "  [WARN] No shard files found!" -ForegroundColor Yellow
    Write-Host "  Nodes will need to download shards via torrent protocol" -ForegroundColor Gray
    Write-Host "  This happens automatically when LOAD_SHARD command is received" -ForegroundColor Gray
} else {
    Write-Host ""
    Write-Host "  [OK] $($shardFiles.Count)/4 shard files found" -ForegroundColor Green
    Write-Host "  Nodes should load these automatically" -ForegroundColor Gray
}

# Check running nodes
Write-Host ""
Write-Host "[2/3] Checking shard nodes..." -ForegroundColor Yellow
$nodes = Get-Process | Where-Object {$_.ProcessName -eq "shard_listener"} -ErrorAction SilentlyContinue
$nodeCount = if ($nodes) { $nodes.Count } else { 0 }

Write-Host "  Running nodes: $nodeCount/4" -ForegroundColor $(if ($nodeCount -eq 4) { 'Green' } elseif ($nodeCount -gt 0) { 'Yellow' } else { 'Red' })

if ($nodeCount -eq 0) {
    Write-Host "  [ERROR] No shard nodes are running!" -ForegroundColor Red
    Write-Host "  Start nodes first: .\start_all_4_shards.ps1" -ForegroundColor Yellow
} else {
    Write-Host "  [OK] Nodes are running" -ForegroundColor Green
    Write-Host ""
    Write-Host "  Check each node terminal window for:" -ForegroundColor Yellow
    Write-Host "    - 'Loading model' or 'Loading shard' messages" -ForegroundColor White
    Write-Host "    - 'Model loaded successfully' messages" -ForegroundColor White
    Write-Host "    - '[INFERENCE]' messages when processing" -ForegroundColor White
    Write-Host "    - Any error messages" -ForegroundColor White
}

# Check web server for pipeline status
Write-Host ""
Write-Host "[3/3] Checking pipeline status..." -ForegroundColor Yellow
try {
    $response = Invoke-WebRequest -Uri "http://localhost:8080" -TimeoutSec 3 -UseBasicParsing -ErrorAction Stop
    Write-Host "  [OK] Web server is responding" -ForegroundColor Green
    Write-Host ""
    Write-Host "  Open http://localhost:8080 to see:" -ForegroundColor Yellow
    Write-Host "    - Pipeline status (X/4 nodes online)" -ForegroundColor White
    Write-Host "    - Node registration status" -ForegroundColor White
    Write-Host "    - Shard availability" -ForegroundColor White
} catch {
    Write-Host "  [ERROR] Web server not responding: $_" -ForegroundColor Red
    Write-Host "  Web server may still be starting up" -ForegroundColor Yellow
}

Write-Host ""
Write-Host "========================================" -ForegroundColor Cyan
Write-Host "  TENSOR LOADING INDICATORS" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""
Write-Host "In Shard Node Terminals, look for:" -ForegroundColor Yellow
Write-Host ""
Write-Host "✅ Tensor Loading Messages:" -ForegroundColor Green
Write-Host "  - 'Loading model shard X'" -ForegroundColor Gray
Write-Host "  - 'Model loaded successfully'" -ForegroundColor Gray
Write-Host "  - 'Shard X loaded'" -ForegroundColor Gray
Write-Host "  - 'Tensor shape: ...'" -ForegroundColor Gray
Write-Host ""
Write-Host "✅ Shard Announcement:" -ForegroundColor Green
Write-Host "  - '[DHT] ✓✓✓ ANNOUNCED SHARD X TO DHT ✓✓✓'" -ForegroundColor Gray
Write-Host ""
Write-Host "✅ Ready for Inference:" -ForegroundColor Green
Write-Host "  - '[COMMAND] ✓ Validation passed'" -ForegroundColor Gray
Write-Host "  - '[INFERENCE] Processing inference request...'" -ForegroundColor Gray
Write-Host ""
Write-Host "❌ Error Messages:" -ForegroundColor Red
Write-Host "  - 'Failed to load model'" -ForegroundColor Gray
Write-Host "  - 'File not found'" -ForegroundColor Gray
Write-Host "  - 'Out of memory'" -ForegroundColor Gray
Write-Host "  - panic messages" -ForegroundColor Gray
Write-Host ""
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

