# Diagnose Shard Loading Block
# Checks what's preventing nodes from completing loading

param(
    [string]$ShardsDir = "models_cache\shards"
)

Write-Host ""
Write-Host "╔══════════════════════════════════════════════════════════════╗" -ForegroundColor Cyan
Write-Host "║  DIAGNOSING SHARD LOADING BLOCK                             ║" -ForegroundColor Cyan
Write-Host "╚══════════════════════════════════════════════════════════════╝" -ForegroundColor Cyan
Write-Host ""

# Step 1: Check if shard files exist locally
Write-Host "[1/4] Checking for local shard files..." -ForegroundColor Yellow
Write-Host ""

$missingShards = @()
$foundShards = @()

for ($i = 0; $i -lt 8; $i++) {
    $shardFile = Join-Path $ShardsDir "shard-$i.gguf"
    if (Test-Path $shardFile) {
        $fileSize = (Get-Item $shardFile).Length
        $sizeGB = [math]::Round($fileSize / 1GB, 2)
        $sizeStr = "$sizeGB GB"
        Write-Host "  Shard $i : [OK] EXISTS ($sizeStr)" -ForegroundColor Green
        $foundShards += $i
    } else {
        Write-Host "  Shard $i : [MISSING]" -ForegroundColor Red
        $missingShards += $i
    }
}

Write-Host ""
if ($missingShards.Count -eq 0) {
    Write-Host "  [OK] All 8 shard files exist locally" -ForegroundColor Green
} else {
    Write-Host "  [BLOCKER] Missing $($missingShards.Count) shard file(s): $($missingShards -join ', ')" -ForegroundColor Red
    Write-Host ""
    Write-Host "  This is likely blocking swarm ready!" -ForegroundColor Yellow
    Write-Host "  Nodes need these files in: $ShardsDir" -ForegroundColor Yellow
}

Write-Host ""

# Step 2: Check node processes
Write-Host "[2/4] Checking for running node processes..." -ForegroundColor Yellow
Write-Host ""

$nodeProcesses = Get-Process -Name "node" -ErrorAction SilentlyContinue | Where-Object { $_.Path -like "*punch*" -or $_.CommandLine -like "*shard*" }
$cargoProcesses = Get-Process -Name "cargo" -ErrorAction SilentlyContinue | Where-Object { $_.CommandLine -like "*shard*" -or $_.CommandLine -like "*node*" }

$totalNodes = $nodeProcesses.Count + $cargoProcesses.Count

if ($totalNodes -gt 0) {
    Write-Host "  [OK] Found $totalNodes node process(es) running" -ForegroundColor Green
    if ($nodeProcesses) {
        Write-Host "    - $($nodeProcesses.Count) node.exe process(es)" -ForegroundColor Gray
    }
    if ($cargoProcesses) {
        Write-Host "    - $($cargoProcesses.Count) cargo process(es)" -ForegroundColor Gray
    }
} else {
    Write-Host "  [WARNING] No node processes found" -ForegroundColor Yellow
    Write-Host "    Nodes may not be running" -ForegroundColor Gray
}

Write-Host ""

# Step 3: Check what nodes are reporting
Write-Host "[3/4] What to check in node windows..." -ForegroundColor Yellow
Write-Host ""

Write-Host "  For EACH node window, look for:" -ForegroundColor Cyan
Write-Host ""
Write-Host "  [GOOD] Shard loaded:" -ForegroundColor Green
Write-Host "    [SHARD] SHARD X LOADED BEFORE JOINING NETWORK" -ForegroundColor White
Write-Host ""
Write-Host "  [BAD] Shard not found:" -ForegroundColor Red
Write-Host "    [SHARD] ASSIGNED SHARD X NOT FOUND LOCALLY" -ForegroundColor White
Write-Host ""
Write-Host "  Also check status reports:" -ForegroundColor Cyan
Write-Host "    [STATUS] Discovered Shards: X / 8" -ForegroundColor White
Write-Host "    [STATUS] Shard Loaded: YES / NO" -ForegroundColor White
Write-Host ""

# Step 4: Provide solutions
Write-Host "[4/4] Solutions..." -ForegroundColor Yellow
Write-Host ""

if ($missingShards.Count -gt 0) {
    Write-Host "  SOLUTION 1: Copy missing shard files" -ForegroundColor Yellow
    Write-Host ""
    Write-Host "    Missing shards: $($missingShards -join ', ')" -ForegroundColor White
    Write-Host "    Copy these files to: $ShardsDir" -ForegroundColor White
    Write-Host ""
    Write-Host "    Example:" -ForegroundColor Gray
    foreach ($shardId in $missingShards) {
        Write-Host "      Copy shard-$shardId.gguf to $ShardsDir\shard-$shardId.gguf" -ForegroundColor Gray
    }
    Write-Host ""
    Write-Host "    After copying, restart the affected node(s)" -ForegroundColor White
    Write-Host ""
}

Write-Host "  SOLUTION 2: Wait for torrent downloads" -ForegroundColor Yellow
Write-Host ""
Write-Host "    If shard files are on the server (rsync.net or eagleoneonline.ca):" -ForegroundColor White
Write-Host "    - Nodes will download them via torrent" -ForegroundColor White
Write-Host "    - Look for: [LOAD_SHARD] 📥 Starting torrent download..." -ForegroundColor White
Write-Host "    - Downloads can take 15 minutes to 2+ hours depending on file size" -ForegroundColor White
Write-Host "    - Each shard is ~12-13 GB" -ForegroundColor White
Write-Host ""

Write-Host "  SOLUTION 3: Manual LOAD_SHARD command" -ForegroundColor Yellow
Write-Host ""
Write-Host "    If coordinator is running, it should send LOAD_SHARD automatically" -ForegroundColor White
Write-Host "    If not, you can manually trigger it (requires coordinator/web_server)" -ForegroundColor White
Write-Host ""

Write-Host "╔══════════════════════════════════════════════════════════════╗" -ForegroundColor Cyan
Write-Host "║  DIAGNOSIS COMPLETE                                          ║" -ForegroundColor Cyan
Write-Host "╚══════════════════════════════════════════════════════════════╝" -ForegroundColor Cyan
Write-Host ""

if ($missingShards.Count -gt 0) {
    Write-Host "BLOCKER IDENTIFIED:" -ForegroundColor Red
    Write-Host "  Missing shard files: $($missingShards -join ', ')" -ForegroundColor Red
    Write-Host ""
    Write-Host "  These nodes have shard_loaded = false" -ForegroundColor Yellow
    Write-Host "  This blocks are_all_shards_loaded function from returning true" -ForegroundColor Yellow
    Write-Host "  Swarm ready cannot be achieved until these files exist" -ForegroundColor Yellow
    Write-Host ""
    Write-Host "  ACTION: Copy missing shard files to $ShardsDir" -ForegroundColor Cyan
} else {
    Write-Host "LOCAL FILES: All shard files exist" -ForegroundColor Green
    Write-Host ""
    Write-Host '  If swarm is still not ready, check:' -ForegroundColor Yellow
    Write-Host '    1. Are all 8 nodes running?' -ForegroundColor White
    Write-Host '    2. Are all nodes showing SHARD X LOADED in their windows?' -ForegroundColor White
    Write-Host '    3. Are all nodes discovered? Check status: Discovered Shards: 8 / 8' -ForegroundColor White
    Write-Host '    4. Check for any error messages in node windows' -ForegroundColor White
}

Write-Host ""
