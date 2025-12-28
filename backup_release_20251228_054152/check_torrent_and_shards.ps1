# Check torrent files and shard loading status
Write-Host "`n========================================" -ForegroundColor Cyan
Write-Host "  TORRENT FILES & SHARD LOADING CHECK" -ForegroundColor Cyan
Write-Host "========================================`n" -ForegroundColor Cyan

# Check shard files exist
Write-Host "[1/4] Checking shard files on disk..." -ForegroundColor Yellow
$shardFiles = @("shard-0.gguf", "shard-1.gguf", "shard-2.gguf", "shard-3.gguf")
$found = 0
foreach ($file in $shardFiles) {
    $path = "models_cache/shards/$file"
    if (Test-Path $path) {
        $sizeMB = [math]::Round((Get-Item $path).Length / 1MB, 1)
        Write-Host "  ✓ $file ($sizeMB MB)" -ForegroundColor Green
        $found++
    } else {
        Write-Host "  ✗ $file (NOT FOUND)" -ForegroundColor Red
    }
}
Write-Host "  Shard Files: $found/4 found`n" -ForegroundColor $(if ($found -eq 4) { 'Green' } else { 'Yellow' })

# Check running nodes
Write-Host "[2/4] Checking running shard nodes..." -ForegroundColor Yellow
$nodes = Get-Process | Where-Object {$_.ProcessName -eq "shard_listener"}
if ($nodes.Count -eq 0) {
    Write-Host "  ✗ No shard nodes running" -ForegroundColor Red
    Write-Host "`n  Please start the system first:" -ForegroundColor Yellow
    Write-Host "    1. Start bootstrap server" -ForegroundColor Gray
    Write-Host "    2. Start web server (will spawn 4 nodes)" -ForegroundColor Gray
    exit 1
}
Write-Host "  ✓ Found $($nodes.Count) shard node(s)" -ForegroundColor Green
Write-Host ""

# Note: To query nodes for files, we would need to:
# 1. Get peer IDs from node logs or DHT
# 2. Send LIST_FILES command via P2P
# 3. Check loaded shards via GET_CAPABILITIES

Write-Host "[3/4] Expected torrent server behavior:" -ForegroundColor Yellow
Write-Host "  Each node should:" -ForegroundColor White
Write-Host "    - Scan models_cache/shards/ for .gguf files" -ForegroundColor Gray
Write-Host "    - Seed all 4 shard files (shard-0 through shard-3)" -ForegroundColor Gray
Write-Host "    - Register files in DHT for auto-propagation" -ForegroundColor Gray
Write-Host "    - Load assigned shard (shard-X.gguf where X = shard_id)" -ForegroundColor Gray
Write-Host ""

Write-Host "[4/4] Check node console windows for:" -ForegroundColor Yellow
Write-Host "  Torrent Seeding:" -ForegroundColor White
Write-Host "    [TORRENT] ✓ Seeding primary shard: shard-0.gguf" -ForegroundColor Gray
Write-Host "    [TORRENT] ✓ Seeding primary shard: shard-1.gguf" -ForegroundColor Gray
Write-Host "    [TORRENT] ✓ Seeding primary shard: shard-2.gguf" -ForegroundColor Gray
Write-Host "    [TORRENT] ✓ Seeding primary shard: shard-3.gguf" -ForegroundColor Gray
Write-Host "    [TORRENT] Primary shards (0-3): 4/4 seeded" -ForegroundColor Gray
Write-Host ""
Write-Host "  DHT Registration:" -ForegroundColor White
Write-Host "    [TORRENT] Registering 4 torrent file(s) in DHT" -ForegroundColor Gray
Write-Host "    [TORRENT] ✓ Registered torrent file in DHT: shard-X.gguf" -ForegroundColor Gray
Write-Host ""
Write-Host "  Shard Loading:" -ForegroundColor White
Write-Host "    [SHARD] ✓✓✓ SHARD X LOADED BEFORE JOINING NETWORK ✓✓✓" -ForegroundColor Gray
Write-Host "    OR" -ForegroundColor Gray
Write-Host "    [LOAD_SHARD] ✓ Loaded shard X from local directory" -ForegroundColor Gray
Write-Host ""

Write-Host "Summary:" -ForegroundColor Yellow
Write-Host "  Files on disk: $found/4" -ForegroundColor $(if ($found -eq 4) { 'Green' } else { 'Yellow' })
Write-Host "  Nodes running: $($nodes.Count)/4" -ForegroundColor $(if ($nodes.Count -eq 4) { 'Green' } else { 'Yellow' })
Write-Host "  Check console windows for detailed status" -ForegroundColor Gray
Write-Host ""

