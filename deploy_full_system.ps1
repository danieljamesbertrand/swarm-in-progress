# Deploy Full System: Rendezvous Server + 4 Shard Nodes + Web Server
# This script deploys the complete distributed inference system

param(
    [string]$BootstrapHost = "eagleoneonline.ca",
    [string]$BootstrapPort = "51820",
    [string]$Cluster = "llama-cluster",
    [int]$TotalShards = 4,
    [int]$TotalLayers = 32,
    [string]$ModelName = "llama-8b",
    [string]$ShardsDir = "models_cache/shards",
    [switch]$SkipRendezvous = $false,
    [switch]$SkipNodes = $false,
    [switch]$SkipWebServer = $false
)

Write-Host ""
Write-Host "================================================================================" -ForegroundColor Cyan
Write-Host "  DEPLOY FULL DISTRIBUTED INFERENCE SYSTEM" -ForegroundColor Cyan
Write-Host "================================================================================" -ForegroundColor Cyan
Write-Host ""

# Determine bootstrap address (QUIC preferred)
$bootstrapAddr = "/ip4/${BootstrapHost}/udp/${BootstrapPort}/quic-v1"
$bootstrapAddrTcp = "/ip4/${BootstrapHost}/tcp/${BootstrapPort}"

# Resolve hostname to IP address (required for Multiaddr parsing)
Write-Host "[0/4] Resolving bootstrap hostname..." -ForegroundColor Yellow
try {
    $bootstrapIP = [System.Net.Dns]::GetHostAddresses($BootstrapHost) | Where-Object { $_.AddressFamily -eq 'InterNetwork' } | Select-Object -First 1
    if (-not $bootstrapIP) {
        Write-Host "  [ERROR] Failed to resolve ${BootstrapHost} to IPv4 address" -ForegroundColor Red
        exit 1
    }
    $bootstrapIPString = $bootstrapIP.IPAddressToString
    Write-Host "  [OK] Resolved ${BootstrapHost} to ${bootstrapIPString}" -ForegroundColor Green
} catch {
    Write-Host "  [ERROR] DNS resolution failed: $_" -ForegroundColor Red
    exit 1
}

# Update bootstrap addresses to use IP instead of hostname
$bootstrapAddr = "/ip4/${bootstrapIPString}/udp/${BootstrapPort}/quic-v1"
$bootstrapAddrTcp = "/ip4/${bootstrapIPString}/tcp/${BootstrapPort}"

Write-Host ""
Write-Host "Configuration:" -ForegroundColor Yellow
Write-Host "  Bootstrap Server: ${BootstrapHost} (${bootstrapIPString}:${BootstrapPort})" -ForegroundColor Gray
Write-Host "  Bootstrap Address (QUIC): ${bootstrapAddr}" -ForegroundColor Gray
Write-Host "  Bootstrap Address (TCP): ${bootstrapAddrTcp}" -ForegroundColor Gray
Write-Host "  Cluster: ${Cluster}" -ForegroundColor Gray
Write-Host "  Total Shards: ${TotalShards}" -ForegroundColor Gray
Write-Host "  Total Layers: ${TotalLayers}" -ForegroundColor Gray
Write-Host "  Model: ${ModelName}" -ForegroundColor Gray
Write-Host ""

# Step 1: Verify/Create Rendezvous Server
if (-not $SkipRendezvous) {
    Write-Host "[1/4] Checking rendezvous server on ${BootstrapHost}..." -ForegroundColor Yellow
    
    # Check if server process is running (via SSH)
    $serverCheck = ssh dbertrand@${BootstrapHost} "ps aux | grep -E 'server --listen|node.*bootstrap' | grep -v grep" 2>$null
    if ($serverCheck) {
        Write-Host "  [OK] Rendezvous server process is running" -ForegroundColor Green
        
        # Check if server has torrent seeding
        $checkSeeding = ssh dbertrand@${BootstrapHost} "cd ~/punch-simple && if [ -f target/release/server ]; then ./target/release/server --help 2>&1 | grep -c 'seed-dir' || echo '0'; else echo '0'; fi" 2>$null
        if ($checkSeeding -and [int]$checkSeeding -gt 0) {
            Write-Host "  [OK] Rendezvous server has torrent seeding support" -ForegroundColor Green
        } else {
            Write-Host "  [WARNING] Rendezvous server may not have torrent seeding" -ForegroundColor Yellow
            Write-Host "    Run: .\deploy_server_to_eagleoneonline.ps1" -ForegroundColor Gray
        }
    } else {
        Write-Host "  [WARNING] Rendezvous server process not found" -ForegroundColor Yellow
        Write-Host "    The server may still be starting, or you may need to start it:" -ForegroundColor Gray
        Write-Host "    ssh dbertrand@${BootstrapHost} 'cd ~/punch-simple && nohup ./target/release/server --listen-addr 0.0.0.0 --port ${BootstrapPort} --transport quic --seed-dir ~/punch-simple/shards > server.log 2>&1 &'" -ForegroundColor Gray
        Write-Host ""
        Write-Host "    Continuing anyway - nodes will try to connect..." -ForegroundColor Yellow
    }
} else {
    Write-Host "[1/4] Skipping rendezvous server check (--SkipRendezvous)" -ForegroundColor Yellow
}

# Step 2: Clean up existing local nodes
Write-Host ""
Write-Host "[2/4] Cleaning up existing local processes..." -ForegroundColor Yellow

# Stop existing shard nodes
$existingShards = Get-Process | Where-Object {$_.ProcessName -eq "shard_listener" -or $_.ProcessName -eq "node"} -ErrorAction SilentlyContinue
if ($existingShards) {
    Write-Host "  Stopping $($existingShards.Count) existing shard node process(es)..." -ForegroundColor Gray
    $existingShards | Stop-Process -Force -ErrorAction SilentlyContinue
    Start-Sleep -Seconds 2
}

# Stop existing web server
$existingWeb = Get-Process | Where-Object {$_.ProcessName -eq "web_server"} -ErrorAction SilentlyContinue
if ($existingWeb) {
    Write-Host "  Stopping existing web server..." -ForegroundColor Gray
    $existingWeb | Stop-Process -Force -ErrorAction SilentlyContinue
    Start-Sleep -Seconds 2
}

Write-Host "  [OK] Cleanup complete" -ForegroundColor Green

# Step 3: Start 4 Shard Nodes
if (-not $SkipNodes) {
    Write-Host ""
    Write-Host "[3/4] Starting 4 shard nodes..." -ForegroundColor Yellow
    Write-Host "  Connecting to bootstrap: ${BootstrapHost}" -ForegroundColor Gray
    Write-Host "  Using QUIC transport (dual-stack: QUIC + TCP fallback)" -ForegroundColor Gray
    Write-Host ""
    
    for ($i = 0; $i -lt $TotalShards; $i++) {
        $port = 51821 + $i
        Write-Host "  [$($i+1)/${TotalShards}] Starting shard node $i on port ${port}..." -ForegroundColor Cyan
        
        $command = @"
cd '$PWD'; 
`$env:LLAMA_SHARD_ID='$i'; 
`$env:LLAMA_TOTAL_SHARDS='${TotalShards}'; 
`$env:LLAMA_TOTAL_LAYERS='${TotalLayers}'; 
`$env:LLAMA_MODEL_NAME='${ModelName}'; 
Write-Host '=== SHARD NODE $i ===' -ForegroundColor Cyan; 
Write-Host 'Bootstrap: ${BootstrapHost}' -ForegroundColor Gray; 
Write-Host 'Cluster: ${Cluster}' -ForegroundColor Gray; 
cargo run --bin node -- shard-listener --bootstrap ${bootstrapAddr} --cluster ${Cluster} --shard-id $i --total-shards ${TotalShards} --total-layers ${TotalLayers} --model-name ${ModelName} --port ${port} --shards-dir ${ShardsDir} --transport dual
"@
        
        Start-Process powershell -ArgumentList "-NoExit", "-Command", $command -WindowStyle Normal
        Start-Sleep -Seconds 3
    }
    
    Write-Host ""
    Write-Host "  [OK] All 4 shard nodes starting" -ForegroundColor Green
    Write-Host "  Waiting 15 seconds for nodes to connect and register..." -ForegroundColor Gray
    Start-Sleep -Seconds 15
    
    # Verify nodes are running
    $runningNodes = Get-Process | Where-Object {$_.ProcessName -eq "node" -or $_.ProcessName -eq "shard_listener"} -ErrorAction SilentlyContinue
    $nodeCount = if ($runningNodes) { $runningNodes.Count } else { 0 }
    Write-Host "  Running nodes: ${nodeCount}/${TotalShards}" -ForegroundColor $(if ($nodeCount -eq $TotalShards) { 'Green' } elseif ($nodeCount -gt 0) { 'Yellow' } else { 'Red' })
} else {
    Write-Host "[3/4] Skipping shard nodes (--SkipNodes)" -ForegroundColor Yellow
}

# Step 4: Start Web Server
if (-not $SkipWebServer) {
    Write-Host ""
    Write-Host "[4/4] Starting web server..." -ForegroundColor Yellow
    Write-Host "  Bootstrap: ${BootstrapHost}" -ForegroundColor Gray
    Write-Host "  Web UI: http://localhost:8080" -ForegroundColor Gray
    Write-Host "  Transport: QUIC + TCP (dual-stack)" -ForegroundColor Gray
    Write-Host "  JSON Pipeline: Supported via /json-message/1.0" -ForegroundColor Gray
    Write-Host ""
    
    $webCommand = @"
cd '$PWD'; 
`$env:BOOTSTRAP='${bootstrapAddr}'; 
Write-Host '=== WEB SERVER ===' -ForegroundColor Cyan; 
Write-Host 'Bootstrap: ${BootstrapHost}' -ForegroundColor Gray; 
Write-Host 'Web UI: http://localhost:8080' -ForegroundColor Gray; 
cargo run --bin web_server
"@
    
    Start-Process powershell -ArgumentList "-NoExit", "-Command", $webCommand -WindowStyle Normal
    
    Write-Host "  [OK] Web server starting" -ForegroundColor Green
    Write-Host "  Waiting 20 seconds for web server to start..." -ForegroundColor Gray
    Start-Sleep -Seconds 20
    
    # Test web server
    $maxAttempts = 5
    $attempt = 0
    $webServerReady = $false
    
    while ($attempt -lt $maxAttempts -and -not $webServerReady) {
        $attempt++
        try {
            $response = Invoke-WebRequest -Uri "http://localhost:8080" -TimeoutSec 5 -UseBasicParsing -ErrorAction Stop
            Write-Host "  [OK] Web server is responding! (Status: $($response.StatusCode))" -ForegroundColor Green
            $webServerReady = $true
        } catch {
            if ($attempt -lt $maxAttempts) {
                Write-Host "  [ATTEMPT $attempt/${maxAttempts}] Waiting for web server..." -ForegroundColor Gray
                Start-Sleep -Seconds 3
            }
        }
    }
    
    if (-not $webServerReady) {
        Write-Host "  [WARNING] Web server may not be ready yet" -ForegroundColor Yellow
        Write-Host "    Check the web server terminal window" -ForegroundColor Gray
    }
} else {
    Write-Host "[4/4] Skipping web server (--SkipWebServer)" -ForegroundColor Yellow
}

# Final Summary
Write-Host ""
Write-Host "================================================================================" -ForegroundColor Green
Write-Host "  DEPLOYMENT COMPLETE" -ForegroundColor Green
Write-Host "================================================================================" -ForegroundColor Green
Write-Host ""

Write-Host "System Status:" -ForegroundColor Yellow
Write-Host "  Rendezvous Server: ${BootstrapHost}:${BootstrapPort}" -ForegroundColor Cyan
Write-Host "  Shard Nodes: 4 nodes (shards 0-3)" -ForegroundColor Cyan
Write-Host "  Web Server: http://localhost:8080" -ForegroundColor Cyan
Write-Host ""

Write-Host "Web Server Features:" -ForegroundColor Yellow
Write-Host "  [OK] QUIC Transport: Supported (dual-stack)" -ForegroundColor Green
Write-Host "  [OK] JSON Pipeline Messaging: Supported (/json-message/1.0)" -ForegroundColor Green
Write-Host "  [OK] Pipeline Coordinator: Integrated" -ForegroundColor Green
Write-Host "  [OK] DHT Discovery: Enabled" -ForegroundColor Green
Write-Host ""

Write-Host "Next Steps:" -ForegroundColor Yellow
Write-Host "  1. Open web UI: http://localhost:8080" -ForegroundColor Cyan
Write-Host "  2. Wait for nodes to discover each other (30-60 seconds)" -ForegroundColor Gray
Write-Host "  3. Check node status in web UI" -ForegroundColor Gray
Write-Host "  4. Submit an inference query" -ForegroundColor Gray
Write-Host ""

Write-Host "To verify nodes are running:" -ForegroundColor Yellow
Write-Host '  Get-Process | Where-Object {$_.ProcessName -eq "node" -or $_.ProcessName -eq "web_server"}' -ForegroundColor Gray
Write-Host ""

Write-Host "To view logs:" -ForegroundColor Yellow
Write-Host "  Check each PowerShell window for node output" -ForegroundColor Gray
Write-Host ""
