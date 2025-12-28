# Single Node Inference Test
# Tests inference with one shard node asking "what do a cat and a snake have in common"

Write-Host ""
Write-Host "═══════════════════════════════════════════════════════════════" -ForegroundColor Cyan
Write-Host "  SINGLE NODE INFERENCE TEST" -ForegroundColor Cyan
Write-Host "  Question: 'what do a cat and a snake have in common'" -ForegroundColor Cyan
Write-Host "═══════════════════════════════════════════════════════════════" -ForegroundColor Cyan
Write-Host ""

# Cleanup existing processes
Write-Host "[1/6] Cleaning up existing processes..." -ForegroundColor Yellow
$processes = Get-Process | Where-Object { 
    $_.ProcessName -match "bootstrap|web_server|shard_listener|server" -or
    $_.ProcessName -eq "cargo"
} -ErrorAction SilentlyContinue

if ($processes) {
    $processes | ForEach-Object {
        try {
            Stop-Process -Id $_.Id -Force -ErrorAction SilentlyContinue
        } catch {}
    }
    Start-Sleep -Seconds 3
}
Write-Host "  [OK] Cleanup complete" -ForegroundColor Green

# Check shard files
Write-Host ""
Write-Host "[2/6] Checking shard files..." -ForegroundColor Yellow
$shard0 = "models_cache/shards/shard-0.gguf"
if (Test-Path $shard0) {
    Write-Host "  [OK] Found shard-0.gguf" -ForegroundColor Green
} else {
    Write-Host "  [WARN] Warning: shard-0.gguf not found" -ForegroundColor Yellow
    Write-Host "     Node will still start but may need to download shard" -ForegroundColor Gray
}

# Start bootstrap server
Write-Host ""
Write-Host "[3/6] Starting bootstrap server..." -ForegroundColor Yellow
Start-Process powershell -ArgumentList "-NoExit", "-Command", "cd '$PWD'; cargo run --bin node -- bootstrap --listen-addr 0.0.0.0 --port 51820" -WindowStyle Minimized
Start-Sleep -Seconds 5

$bootstrap = Get-Process | Where-Object {$_.ProcessName -eq "node"} -ErrorAction SilentlyContinue
if ($bootstrap) {
    Write-Host "  [OK] Bootstrap running (PID: $($bootstrap.Id))" -ForegroundColor Green
} else {
    Write-Host "  [ERROR] Bootstrap failed to start" -ForegroundColor Red
    exit 1
}

# Start single shard node (shard 0)
Write-Host ""
Write-Host "[4/6] Starting single shard node (shard 0)..." -ForegroundColor Yellow
$env:LLAMA_SHARD_ID = "0"
$env:LLAMA_TOTAL_SHARDS = "4"
$env:LLAMA_TOTAL_LAYERS = "32"
$env:LLAMA_MODEL_NAME = "llama-8b"
Start-Process powershell -ArgumentList "-NoExit", "-Command", "cd '$PWD'; `$env:LLAMA_SHARD_ID='0'; `$env:LLAMA_TOTAL_SHARDS='4'; `$env:LLAMA_TOTAL_LAYERS='32'; `$env:LLAMA_MODEL_NAME='llama-8b'; cargo run --bin shard_listener -- --bootstrap /ip4/127.0.0.1/tcp/51820 --cluster llama-cluster --shard-id 0 --total-shards 4 --total-layers 32 --model-name llama-8b --port 51821 --shards-dir models_cache/shards" -WindowStyle Normal
Start-Sleep -Seconds 8

$node = Get-Process | Where-Object {$_.ProcessName -eq "shard_listener"} -ErrorAction SilentlyContinue
if ($node) {
    Write-Host "  [OK] Shard node running (PID: $($node.Id))" -ForegroundColor Green
} else {
    Write-Host "  [WARN] Shard node may still be starting..." -ForegroundColor Yellow
}

# Start web server
Write-Host ""
Write-Host "[5/6] Starting web server..." -ForegroundColor Yellow
$env:BOOTSTRAP = "/ip4/127.0.0.1/tcp/51820"
Start-Process powershell -ArgumentList "-NoExit", "-Command", "cd '$PWD'; `$env:BOOTSTRAP='/ip4/127.0.0.1/tcp/51820'; cargo run --bin web_server" -WindowStyle Normal
Start-Sleep -Seconds 10

$web = Get-Process | Where-Object {$_.ProcessName -eq "web_server"} -ErrorAction SilentlyContinue
if ($web) {
    Write-Host "  [OK] Web server running (PID: $($web.Id))" -ForegroundColor Green
} else {
    Write-Host "  [WARN] Web server may still be starting..." -ForegroundColor Yellow
}

# Wait for node to be ready
Write-Host ""
Write-Host "[6/6] Waiting for node to be ready..." -ForegroundColor Yellow
$maxWait = 20
$elapsed = 0
while ($elapsed -lt $maxWait) {
    Start-Sleep -Seconds 2
    $elapsed += 2
    $node = Get-Process | Where-Object {$_.ProcessName -eq "shard_listener"} -ErrorAction SilentlyContinue
    if ($node) {
        Write-Host "  Node running... ($elapsed seconds elapsed)" -ForegroundColor Gray
        if ($elapsed -ge 10) {
            Write-Host "  [OK] Node should be ready" -ForegroundColor Green
            break
        }
    }
}

Write-Host ""
Write-Host "═══════════════════════════════════════════════════════════════" -ForegroundColor Cyan
Write-Host "  READY FOR INFERENCE TEST" -ForegroundColor Green
Write-Host "═══════════════════════════════════════════════════════════════" -ForegroundColor Cyan
Write-Host ""
Write-Host "Processes Running:" -ForegroundColor Yellow
if ($bootstrap) {
    Write-Host "  Bootstrap: [OK] (PID: $($bootstrap.Id))" -ForegroundColor Green
} else {
    Write-Host "  Bootstrap: [ERROR]" -ForegroundColor Red
}
if ($node) {
    Write-Host "  Shard Node: [OK] (PID: $($node.Id))" -ForegroundColor Green
} else {
    Write-Host "  Shard Node: [ERROR]" -ForegroundColor Red
}
if ($web) {
    Write-Host "  Web Server: [OK] (PID: $($web.Id))" -ForegroundColor Green
} else {
    Write-Host "  Web Server: [ERROR]" -ForegroundColor Red
}
Write-Host ""
Write-Host "To Test Inference:" -ForegroundColor Yellow
Write-Host "  1. Open browser: http://localhost:8080" -ForegroundColor White
Write-Host "  2. Wait 5-10 seconds for node to register" -ForegroundColor White
Write-Host "  3. Enter question: 'what do a cat and a snake have in common'" -ForegroundColor Cyan
Write-Host "  4. Click Send or press Enter" -ForegroundColor White
Write-Host "  5. Watch the response appear below" -ForegroundColor White
Write-Host ""
Write-Host "Expected Behavior:" -ForegroundColor Yellow
Write-Host "  - Node should process the inference request" -ForegroundColor White
Write-Host "  - Response should appear in the web console" -ForegroundColor White
Write-Host "  - Check node terminal for inference logs" -ForegroundColor White
Write-Host "  - Check web server terminal for coordinator logs" -ForegroundColor White
Write-Host ""
Write-Host "Monitoring Logs:" -ForegroundColor Yellow
Write-Host "  - Shard node terminal: Look for '[INFERENCE]' messages" -ForegroundColor Gray
Write-Host "  - Web server terminal: Look for '[INFERENCE]' and '[P2P]' messages" -ForegroundColor Gray
Write-Host "  - Browser console (F12): Look for WebSocket messages" -ForegroundColor Gray
Write-Host ""
Write-Host "═══════════════════════════════════════════════════════════════" -ForegroundColor Cyan
Write-Host ""

