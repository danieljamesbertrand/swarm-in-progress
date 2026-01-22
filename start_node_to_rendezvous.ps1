# Start One Shard Node Connected to Rendezvous Server
# Connects to eagleoneonline.ca:51820

param(
    [int]$ShardId = 0,
    [int]$TotalShards = 8,
    [string]$BootstrapHost = "eagleoneonline.ca",
    [int]$BootstrapPort = 51820,
    [string]$Cluster = "llama-cluster",
    [int]$TotalLayers = 32,
    [string]$ModelName = "llama-8b",
    [string]$ShardsDir = "models_cache/shards",
    [string]$Transport = "dual",
    [int]$Port = 0
)

Write-Host ""
Write-Host "========================================" -ForegroundColor Cyan
Write-Host "  START SHARD NODE TO RENDEZVOUS SERVER" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

# Step 1: Resolve bootstrap hostname to IP
Write-Host "[1/3] Resolving rendezvous server..." -ForegroundColor Yellow
try {
    $bootstrapIP = [System.Net.Dns]::GetHostAddresses($BootstrapHost) | Where-Object { $_.AddressFamily -eq 'InterNetwork' } | Select-Object -First 1
    if (-not $bootstrapIP) {
        Write-Host "  [ERROR] Failed to resolve ${BootstrapHost} to IPv4 address" -ForegroundColor Red
        exit 1
    }
    $bootstrapIPString = $bootstrapIP.IPAddressToString
    Write-Host "  Resolved ${BootstrapHost} to ${bootstrapIPString}" -ForegroundColor Green
} catch {
    Write-Host "  [ERROR] DNS resolution failed: $_" -ForegroundColor Red
    exit 1
}

# Step 2: Build bootstrap address
# For dual transport, we need to provide both addresses or let libp2p handle it
# libp2p dual transport will try QUIC first, then TCP
if ($Transport -eq "quic") {
    $bootstrapAddr = "/ip4/$bootstrapIPString/udp/$BootstrapPort/quic-v1"
} elseif ($Transport -eq "dual") {
    # Dual transport: try QUIC first, fallback to TCP automatically
    # We can provide QUIC address and libp2p will handle TCP fallback
    $bootstrapAddr = "/ip4/$bootstrapIPString/udp/$BootstrapPort/quic-v1"
} else {
    $bootstrapAddr = "/ip4/$bootstrapIPString/tcp/$BootstrapPort"
}

Write-Host ""
Write-Host "[2/3] Node configuration..." -ForegroundColor Yellow
Write-Host "  Shard ID: $ShardId" -ForegroundColor White
Write-Host "  Total Shards: $TotalShards" -ForegroundColor White
Write-Host "  Bootstrap: ${BootstrapHost} (${bootstrapIPString}:${BootstrapPort})" -ForegroundColor White
Write-Host "  Bootstrap Address: $bootstrapAddr" -ForegroundColor White
Write-Host "  Transport: $Transport" -ForegroundColor White
Write-Host "  Cluster: $Cluster" -ForegroundColor White
Write-Host "  Shards Directory: $ShardsDir" -ForegroundColor White
Write-Host ""

# Step 3: Start the node
Write-Host "[3/3] Starting shard node..." -ForegroundColor Yellow
Write-Host "  This will start the node in a new PowerShell window" -ForegroundColor Gray
Write-Host ""

$command = @"
cd '$PWD'; 
`$env:LLAMA_SHARD_ID='$ShardId'; 
`$env:LLAMA_TOTAL_SHARDS='$TotalShards'; 
`$env:LLAMA_TOTAL_LAYERS='$TotalLayers'; 
`$env:LLAMA_MODEL_NAME='$ModelName'; 
Write-Host '========================================' -ForegroundColor Cyan;
Write-Host '  SHARD NODE $ShardId' -ForegroundColor Cyan;
Write-Host '========================================' -ForegroundColor Cyan;
Write-Host '';
Write-Host 'Bootstrap: ${BootstrapHost} (${bootstrapIPString}:${BootstrapPort})' -ForegroundColor Gray;
Write-Host 'Cluster: $Cluster' -ForegroundColor Gray;
Write-Host 'Transport: $Transport' -ForegroundColor Gray;
Write-Host '';
cargo run --bin node -- shard-listener --bootstrap $bootstrapAddr --cluster $Cluster --shard-id $ShardId --total-shards $TotalShards --total-layers $TotalLayers --model-name $ModelName --port $Port --shards-dir $ShardsDir --transport $Transport
"@

Start-Process powershell -ArgumentList "-NoExit", "-Command", $command -WindowStyle Normal

Write-Host "  [OK] Node starting in new window" -ForegroundColor Green
Write-Host ""
Write-Host "The node will:" -ForegroundColor Yellow
Write-Host "  1. Connect to rendezvous server at ${BootstrapHost}" -ForegroundColor White
Write-Host "  2. Join DHT and announce shard ID $ShardId" -ForegroundColor White
Write-Host "  3. Scan for local shard files in $ShardsDir" -ForegroundColor White
Write-Host "  4. Download missing shards via torrent if needed" -ForegroundColor White
Write-Host ""
Write-Host "Watch the node window for connection status and logs." -ForegroundColor Cyan
Write-Host ""
