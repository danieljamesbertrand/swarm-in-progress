# Deploy Shard Files to Rendezvous Server
# Uploads shard files to eagleoneonline.ca and configures the server to seed them

param(
    [string]$SourceDir = "models_cache\shards",
    [string]$RemoteUser = "dbertrand",
    [string]$RemoteHost = "eagleoneonline.ca",
    [string]$RemoteDir = "/home/dbertrand/punch-simple/shards"
)

Write-Host ""
Write-Host "╔══════════════════════════════════════════════════════════════╗" -ForegroundColor Cyan
Write-Host "║  DEPLOY SHARDS TO RENDEZVOUS SERVER                         ║" -ForegroundColor Cyan
Write-Host "╚══════════════════════════════════════════════════════════════╝" -ForegroundColor Cyan
Write-Host ""

# Step 1: Map safetensors to GGUF names locally (if needed)
Write-Host "[1/4] Checking for shard files..." -ForegroundColor Yellow
$shardFiles = Get-ChildItem $SourceDir -Filter "shard-*.gguf" -ErrorAction SilentlyContinue

if ($shardFiles.Count -eq 0) {
    Write-Host "  No shard files found. Attempting to map from safetensors..." -ForegroundColor Yellow
    $loaderExe = ".\target\release\shard_loader.exe"
    if (Test-Path $loaderExe) {
        & $loaderExe map --metadata-dir "E:\rust\llamaModels\shards" --safetensors-dir "E:\rust\llamaModels\shards_f16" --target-dir $SourceDir
        $shardFiles = Get-ChildItem $SourceDir -Filter "shard-*.gguf" -ErrorAction SilentlyContinue
    } else {
        Write-Host "  ⚠️  shard_loader.exe not found. Build it first: cargo build --release --bin shard_loader" -ForegroundColor Yellow
    }
}

if ($shardFiles.Count -eq 0) {
    Write-Host "  ✗ No shard files found in $SourceDir" -ForegroundColor Red
    exit 1
}

Write-Host "  ✓ Found $($shardFiles.Count) shard file(s)" -ForegroundColor Green
foreach ($file in $shardFiles) {
    $sizeMB = [math]::Round($file.Length / 1MB, 2)
    $sizeText = "$sizeMB MB"
    Write-Host "    - $($file.Name) ($sizeText)" -ForegroundColor Gray
}

# Step 2: Create remote directory
Write-Host ""
Write-Host "[2/4] Creating remote directory..." -ForegroundColor Yellow
$createCmd = "ssh -F NUL ${RemoteUser}@${RemoteHost} 'mkdir -p $RemoteDir'"
$result = Invoke-Expression $createCmd 2>&1
if ($LASTEXITCODE -eq 0) {
    Write-Host "  ✓ Remote directory created" -ForegroundColor Green
} else {
    Write-Host "  ⚠️  Directory may already exist (continuing...)" -ForegroundColor Yellow
}

# Step 3: Upload files
Write-Host ""
Write-Host "[3/4] Uploading shard files to server..." -ForegroundColor Yellow
$uploaded = 0
$failed = 0

foreach ($file in $shardFiles) {
    # Skip files that are too small (likely placeholders)
    if ($file.Length -lt 1000000) {
        $skipSizeMB = [math]::Round($file.Length / 1MB, 2)
        $skipSizeText = "$skipSizeMB MB"
        Write-Host "  Skipping $($file.Name) (too small: $skipSizeText)" -ForegroundColor Yellow
        continue
    }
    
    $sizeMB = [math]::Round($file.Length / 1MB, 2)
    $sizeText = "$sizeMB MB"
    Write-Host "  Uploading $($file.Name) ($sizeText)..." -ForegroundColor Gray
    
    $scpCmd = "scp -F NUL '$($file.FullName)' ${RemoteUser}@${RemoteHost}:$RemoteDir/"
    $result = Invoke-Expression $scpCmd 2>&1
    
    if ($LASTEXITCODE -eq 0) {
        Write-Host "    ✓ Uploaded successfully" -ForegroundColor Green
        $uploaded++
    } else {
        Write-Host "    ✗ Upload failed" -ForegroundColor Red
        $failed++
    }
}

# Step 4: Verify and configure server
Write-Host ""
Write-Host "[4/4] Verifying uploads and configuring server..." -ForegroundColor Yellow

$verifyCmd = "ssh -F NUL ${RemoteUser}@${RemoteHost} 'ls -lh $RemoteDir/*.gguf 2>/dev/null | wc -l'"
$remoteCount = (Invoke-Expression $verifyCmd 2>&1 | Out-String).Trim()

Write-Host ""
Write-Host "╔══════════════════════════════════════════════════════════════╗" -ForegroundColor Green
Write-Host "║  DEPLOYMENT SUMMARY                                           ║" -ForegroundColor Green
Write-Host "╚══════════════════════════════════════════════════════════════╝" -ForegroundColor Green
Write-Host ""
Write-Host "Uploaded: $uploaded/$($shardFiles.Count) files" -ForegroundColor $(if ($uploaded -eq $shardFiles.Count) { "Green" } else { "Yellow" })
if ($failed -gt 0) {
    Write-Host "Failed: $failed files" -ForegroundColor Red
}
Write-Host "Remote server has: $remoteCount .gguf file(s)" -ForegroundColor Cyan
Write-Host ""
Write-Host "Next steps:" -ForegroundColor Yellow
Write-Host "  1. Restart the rendezvous server with:" -ForegroundColor White
Write-Host "     ./target/release/server --listen-addr 0.0.0.0 --port 51820 --transport quic --seed-dir $RemoteDir" -ForegroundColor Gray
Write-Host ""
Write-Host "  2. The server will automatically seed all shard files" -ForegroundColor White
Write-Host ""
Write-Host "  3. Shard nodes will download missing shards via torrent when:" -ForegroundColor White
Write-Host "     - Coordinator sends LOAD_SHARD commands" -ForegroundColor Gray
Write-Host "     - Nodes discover files via LIST_FILES from server" -ForegroundColor Gray
Write-Host ""
Write-Host "  4. Once 4 nodes have all required shards, distributed inference begins" -ForegroundColor White
Write-Host ""
