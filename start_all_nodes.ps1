# Start All 8 Shard Nodes
# Starts nodes for shards 0-7 in separate windows

param(
    [switch]$Wait = $false
)

Write-Host ""
Write-Host "========================================" -ForegroundColor Cyan
Write-Host "  STARTING ALL 8 SHARD NODES" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

# Check if start_node_to_rendezvous.ps1 exists
if (-not (Test-Path "start_node_to_rendezvous.ps1")) {
    Write-Host "[ERROR] start_node_to_rendezvous.ps1 not found!" -ForegroundColor Red
    Write-Host "  Make sure you're running this from the project root directory" -ForegroundColor Yellow
    exit 1
}

Write-Host "Starting nodes for shards 0-7..." -ForegroundColor Yellow
Write-Host ""

# Start each node in a separate PowerShell window
for ($shardId = 0; $shardId -lt 8; $shardId++) {
    Write-Host "[$($shardId + 1)/8] Starting node for shard $shardId..." -ForegroundColor Yellow
    
    # Start in new window
    Start-Process powershell -ArgumentList @(
        "-NoExit",
        "-Command",
        "cd '$PWD'; .\start_node_to_rendezvous.ps1 -ShardId $shardId"
    )
    
    # Small delay between starts to avoid overwhelming the system
    Start-Sleep -Milliseconds 500
}

Write-Host ""
Write-Host "========================================" -ForegroundColor Green
Write-Host "  ALL 8 NODES STARTED" -ForegroundColor Green
Write-Host "========================================" -ForegroundColor Green
Write-Host ""
Write-Host "8 PowerShell windows should have opened, one for each shard node." -ForegroundColor White
Write-Host ""
Write-Host "What to expect:" -ForegroundColor Yellow
Write-Host "  - Each node will connect to the rendezvous server" -ForegroundColor White
Write-Host "  - Each node will scan for its shard file" -ForegroundColor White
Write-Host "  - If file exists: [SHARD] SHARD X LOADED BEFORE JOINING NETWORK" -ForegroundColor Green
Write-Host "  - If file missing: [SHARD] ASSIGNED SHARD X NOT FOUND LOCALLY" -ForegroundColor Red
Write-Host "  - Nodes will announce to DHT" -ForegroundColor White
Write-Host "  - Nodes will discover each other (2-5 seconds with optimization)" -ForegroundColor White
Write-Host "  - When all shards are loaded and discovered: SWARM READY!" -ForegroundColor Green
Write-Host ""
Write-Host "Timeline:" -ForegroundColor Yellow
Write-Host "  T+1s:  Nodes find shard files (if they exist)" -ForegroundColor White
Write-Host "  T+5s:  All nodes announced to DHT" -ForegroundColor White
Write-Host "  T+5s:  All nodes discovered each other" -ForegroundColor White
Write-Host "  T+5s:  Swarm ready (if all shards loaded)!" -ForegroundColor Green
Write-Host ""
Write-Host "Monitor the node windows for:" -ForegroundColor Cyan
Write-Host "  - [SHARD] messages (file loading status)" -ForegroundColor White
Write-Host "  - [STATUS] messages (discovery progress)" -ForegroundColor White
Write-Host "  - [SWARM] SWARM IS READY FOR INFERENCE" -ForegroundColor Green
Write-Host ""
