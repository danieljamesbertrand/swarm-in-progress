# Deploy Updated Server with Torrent Seeding to eagleoneonline.ca
# This script updates the server binary on the remote server with torrent seeding support

param(
    [string]$RemoteUser = "dbertrand",
    [string]$RemoteHost = "eagleoneonline.ca",
    [string]$RemoteDir = "/home/dbertrand/punch-simple",
    [string]$SeedDir = "/home/dbertrand/punch-simple/shards",
    [switch]$SkipBuild = $false,
    [switch]$SkipRestart = $false
)

Write-Host ""
Write-Host "================================================================================" -ForegroundColor Cyan
Write-Host "  DEPLOY SERVER WITH TORRENT SEEDING TO EAGLEONEONLINE.CA" -ForegroundColor Cyan
Write-Host "================================================================================" -ForegroundColor Cyan
Write-Host ""

# Step 1: Verify local server.rs has torrent seeding
Write-Host "[1/6] Verifying local server.rs has torrent seeding..." -ForegroundColor Yellow
$serverRsPath = "src\server.rs"
if (-not (Test-Path $serverRsPath)) {
    Write-Host "  [ERROR] $serverRsPath not found!" -ForegroundColor Red
    exit 1
}

$hasSeedDir = Select-String -Path $serverRsPath -Pattern "seed_dir" -Quiet
$hasTorrentServer = Select-String -Path $serverRsPath -Pattern "TorrentServer" -Quiet

if (-not $hasSeedDir -or -not $hasTorrentServer) {
    Write-Host "  [ERROR] Local server.rs does not have torrent seeding support!" -ForegroundColor Red
    Write-Host "    Make sure the file includes seed_dir parameter and TorrentServer implementation" -ForegroundColor Yellow
    exit 1
}

Write-Host "  [OK] Local server.rs has torrent seeding support" -ForegroundColor Green

# Step 2: Copy server.rs to remote server
Write-Host "[2/6] Copying server.rs to remote server..." -ForegroundColor Yellow
$scpCommand = "scp -F NUL `"$serverRsPath`" ${RemoteUser}@${RemoteHost}:${RemoteDir}/src/server.rs"
Write-Host "  Running: scp server.rs to ${RemoteUser}@${RemoteHost}" -ForegroundColor Gray

try {
    $result = Invoke-Expression $scpCommand 2>&1
    if ($LASTEXITCODE -ne 0) {
        Write-Host "  [ERROR] Failed to copy server.rs: $result" -ForegroundColor Red
        exit 1
    }
    Write-Host "  [OK] server.rs copied successfully" -ForegroundColor Green
} catch {
    Write-Host "  [ERROR] Error copying file: $_" -ForegroundColor Red
    exit 1
}

# Step 3: Build on remote server
if (-not $SkipBuild) {
    Write-Host "[3/6] Building server binary on remote server..." -ForegroundColor Yellow
    Write-Host "  This may take several minutes..." -ForegroundColor Gray
    
    $buildCommand = "ssh ${RemoteUser}@${RemoteHost} `"cd ${RemoteDir}; cargo build --release --bin server`""
    
    try {
        Invoke-Expression $buildCommand
        if ($LASTEXITCODE -ne 0) {
            Write-Host "  [ERROR] Build failed on remote server" -ForegroundColor Red
            exit 1
        }
        Write-Host "  [OK] Server binary built successfully" -ForegroundColor Green
    } catch {
        Write-Host "  [ERROR] Error building: $_" -ForegroundColor Red
        exit 1
    }
} else {
    Write-Host "[3/6] Skipping build (SkipBuild flag specified)" -ForegroundColor Yellow
}

# Step 4: Verify remote binary has seed_dir option
Write-Host "[4/6] Verifying remote binary has torrent seeding..." -ForegroundColor Yellow
$verifyCommand = "ssh ${RemoteUser}@${RemoteHost} `"cd ${RemoteDir}; ./target/release/server --help`""
try {
    $helpOutput = Invoke-Expression $verifyCommand 2>&1 | Out-String
    if ($helpOutput -match "seed-dir") {
        Write-Host "  [OK] Remote binary has seed-dir option" -ForegroundColor Green
    } else {
        Write-Host "  [WARNING] Remote binary may not have seed-dir option" -ForegroundColor Yellow
        Write-Host "    Help output:" -ForegroundColor Gray
        Write-Host $helpOutput -ForegroundColor Gray
    }
    } catch {
        Write-Host "  [WARNING] Could not verify binary (this is okay if server is running)" -ForegroundColor Yellow
    }

# Step 5: Stop existing server (if running)
if (-not $SkipRestart) {
    Write-Host "[5/6] Stopping existing server process..." -ForegroundColor Yellow
    $stopCommand = "ssh ${RemoteUser}@${RemoteHost} `"pkill -f 'server --listen' 2>/dev/null; pkill -f 'node.*bootstrap' 2>/dev/null; true`""
    try {
        Invoke-Expression $stopCommand | Out-Null
        Start-Sleep -Seconds 2
        Write-Host "  [OK] Stopped existing server processes" -ForegroundColor Green
    } catch {
        Write-Host "  [WARNING] No existing server process found (this is okay)" -ForegroundColor Yellow
    }
} else {
    Write-Host "[5/6] Skipping server restart (SkipRestart flag specified)" -ForegroundColor Yellow
}

# Step 6: Start server with torrent seeding
if (-not $SkipRestart) {
    Write-Host "[6/6] Starting server with torrent seeding..." -ForegroundColor Yellow
    
    # Check if seed directory exists on remote
    $checkSeedDir = "ssh ${RemoteUser}@${RemoteHost} `"if test -d ${SeedDir}; then echo 'exists'; else echo 'missing'; fi`""
    $seedDirExists = Invoke-Expression $checkSeedDir | Select-Object -Last 1
    
    if ($seedDirExists -eq "missing") {
        Write-Host "  [WARNING] Seed directory ${SeedDir} does not exist on remote server" -ForegroundColor Yellow
        Write-Host "     Server will start without seeding (you can add files later)" -ForegroundColor Yellow
    } else {
        Write-Host "  [OK] Seed directory exists: ${SeedDir}" -ForegroundColor Green
    }
    
    # Start server in background with nohup
    # Escape & properly for PowerShell
    $ampersand = '&'
    $remoteCmd = "cd ${RemoteDir}; nohup ./target/release/server --listen-addr 0.0.0.0 --port 51820 --transport quic --seed-dir ${SeedDir} > server.log 2>&1 $ampersand"
    $startCommand = "ssh ${RemoteUser}@${RemoteHost} '$remoteCmd'"
    
    Write-Host "  Starting server..." -ForegroundColor Gray
    Write-Host "    Command: server --listen-addr 0.0.0.0 --port 51820 --transport quic --seed-dir ${SeedDir}" -ForegroundColor Gray
    
    try {
        Invoke-Expression $startCommand | Out-Null
        Start-Sleep -Seconds 3
        
        # Verify server is running
        $checkRunning = "ssh ${RemoteUser}@${RemoteHost} `"ps aux | grep -E 'server --listen|node.*bootstrap' | grep -v grep`""
        $running = Invoke-Expression $checkRunning
        
        if ($running) {
            Write-Host "  [OK] Server started successfully" -ForegroundColor Green
            Write-Host ""
            Write-Host "  Server is running on: ${RemoteHost}:51820" -ForegroundColor Cyan
            Write-Host "  Torrent seeding: Enabled (from ${SeedDir})" -ForegroundColor Cyan
            Write-Host ""
            Write-Host "  To view logs:" -ForegroundColor Yellow
            Write-Host "    ssh ${RemoteUser}@${RemoteHost} 'tail -f ${RemoteDir}/server.log'" -ForegroundColor Gray
            Write-Host ""
            Write-Host "  To check status:" -ForegroundColor Yellow
            Write-Host "    ssh ${RemoteUser}@${RemoteHost} 'ps aux | grep server'" -ForegroundColor Gray
        } else {
            Write-Host "  [WARNING] Server may not have started. Check logs:" -ForegroundColor Yellow
            Write-Host "    ssh ${RemoteUser}@${RemoteHost} 'cat ${RemoteDir}/server.log'" -ForegroundColor Gray
        }
    } catch {
        Write-Host "  [ERROR] Error starting server: $_" -ForegroundColor Red
        Write-Host "    You may need to start it manually:" -ForegroundColor Yellow
        Write-Host "    ssh ${RemoteUser}@${RemoteHost}" -ForegroundColor Gray
        Write-Host "    cd ${RemoteDir}" -ForegroundColor Gray
        Write-Host "    ./target/release/server --listen-addr 0.0.0.0 --port 51820 --transport quic --seed-dir ${SeedDir}" -ForegroundColor Gray
    }
} else {
    Write-Host "[6/6] Skipping server start (SkipRestart flag specified)" -ForegroundColor Yellow
    Write-Host ""
    Write-Host "  To start the server manually, run:" -ForegroundColor Yellow
    Write-Host "    ssh ${RemoteUser}@${RemoteHost}" -ForegroundColor Gray
    Write-Host "    cd ${RemoteDir}" -ForegroundColor Gray
    Write-Host "    ./target/release/server --listen-addr 0.0.0.0 --port 51820 --transport quic --seed-dir ${SeedDir}" -ForegroundColor Gray
}

Write-Host ""
Write-Host "================================================================================" -ForegroundColor Green
Write-Host "  DEPLOYMENT COMPLETE" -ForegroundColor Green
Write-Host "================================================================================" -ForegroundColor Green
Write-Host ""
