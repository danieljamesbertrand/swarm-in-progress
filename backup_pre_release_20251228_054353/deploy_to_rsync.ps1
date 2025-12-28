# Deploy punch-simple executables (listener.exe and dialer.exe) to rsync server using SCP
# Run from punch-simple directory: .\deploy_to_rsync.ps1

$ErrorActionPreference = "Continue"

Write-Host ""
Write-Host "========================================" -ForegroundColor Cyan
Write-Host "Deploy punch-simple to Rsync Server" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

# Configuration
$RSYNC_HOST = if ($env:RSYNC_HOST) { $env:RSYNC_HOST } else { "zh5605.rsync.net" }
$RSYNC_USER = if ($env:RSYNC_USER) { $env:RSYNC_USER } else { "zh5605" }
$RSYNC_PASSWORD = if ($env:RSYNC_PASSWORD) { $env:RSYNC_PASSWORD } else { "3da393f1" }
$RSYNC_REMOTE_PATH = if ($env:RSYNC_EXECUTABLES_PATH) { $env:RSYNC_EXECUTABLES_PATH } else { "~/executables" }

$listenerPath = "target\release\listener.exe"
$dialerPath = "target\release\dialer.exe"

# Check if executables exist
$missing = @()
if (-not (Test-Path $listenerPath)) {
    $missing += "listener.exe"
}
if (-not (Test-Path $dialerPath)) {
    $missing += "dialer.exe"
}

if ($missing.Count -gt 0) {
    Write-Host "[INFO] Missing executables: $($missing -join ', ')" -ForegroundColor Yellow
    Write-Host "[INFO] Building release binaries..." -ForegroundColor Yellow
    cargo build --release
    if ($LASTEXITCODE -ne 0) {
        Write-Host "[ERROR] Build failed!" -ForegroundColor Red
        exit 1
    }
}

# Verify executables exist
if (-not (Test-Path $listenerPath)) {
    Write-Host "[ERROR] listener.exe not found after build!" -ForegroundColor Red
    exit 1
}
if (-not (Test-Path $dialerPath)) {
    Write-Host "[ERROR] dialer.exe not found after build!" -ForegroundColor Red
    exit 1
}

Write-Host "[INFO] Found executables:" -ForegroundColor Green
$listenerSize = (Get-Item $listenerPath).Length / 1MB
$dialerSize = (Get-Item $dialerPath).Length / 1MB
Write-Host "  listener.exe: $([math]::Round($listenerSize, 2)) MB" -ForegroundColor Gray
Write-Host "  dialer.exe: $([math]::Round($dialerSize, 2)) MB" -ForegroundColor Gray
Write-Host ""

# Check for WSL with installed distribution (needed for sshpass on Windows)
$wslAvailable = $false
try {
    $wslList = wsl --list --quiet 2>&1
    if ($LASTEXITCODE -eq 0 -and $wslList -notmatch "no installed distributions") {
        # Test if we can actually run a command
        $null = wsl echo "test" 2>&1
        if ($LASTEXITCODE -eq 0) {
            $wslAvailable = $true
            Write-Host "[INFO] WSL detected - will use WSL for SCP with sshpass" -ForegroundColor Green
        }
    }
} catch {
    # WSL not available
}

if (-not $wslAvailable) {
    Write-Host "[INFO] Using native SCP (will prompt for password)" -ForegroundColor Yellow
    Write-Host "[INFO] Password: $RSYNC_PASSWORD" -ForegroundColor Gray
}

Write-Host ""
Write-Host "[DEPLOY] Uploading to rsync server..." -ForegroundColor Yellow
Write-Host "  Host: $RSYNC_HOST" -ForegroundColor Gray
Write-Host "  User: $RSYNC_USER" -ForegroundColor Gray
Write-Host "  Path: $RSYNC_REMOTE_PATH" -ForegroundColor Gray
Write-Host ""

$remotePath = "$RSYNC_USER@${RSYNC_HOST}:$RSYNC_REMOTE_PATH"

# First, ensure remote directory exists
Write-Host "[DEPLOY] Ensuring remote directory exists..." -ForegroundColor Gray
if ($wslAvailable) {
    $mkdirCmd = "sshpass -p '$RSYNC_PASSWORD' ssh -o StrictHostKeyChecking=no $RSYNC_USER@${RSYNC_HOST} 'mkdir -p $RSYNC_REMOTE_PATH'"
    wsl bash -c $mkdirCmd
} else {
    Write-Host "[INFO] Note: Directory creation may require password authentication" -ForegroundColor Gray
    Write-Host "[INFO] If upload fails, create directory manually:" -ForegroundColor Gray
    Write-Host "      ssh $RSYNC_USER@${RSYNC_HOST} 'mkdir -p $RSYNC_REMOTE_PATH'" -ForegroundColor Gray
}

# Upload listener.exe
Write-Host "[DEPLOY] Uploading listener.exe via SCP..." -ForegroundColor Cyan
$listenerSuccess = $false

if ($wslAvailable) {
    # Convert Windows path to WSL path
    $wslListenerPath = $listenerPath -replace '\\', '/' -replace '^([A-Z]):', { param($m) "/mnt/$($m.Groups[1].Value.ToLower())" }
    $wslListenerPath = $wslListenerPath -replace '^E:', '/mnt/e'
    $wslListenerPath = $wslListenerPath -replace '^e:', '/mnt/e'
    
    # Use sshpass with scp via WSL
    $scpCmd = "sshpass -p '$RSYNC_PASSWORD' scp -o StrictHostKeyChecking=no '$wslListenerPath' '$remotePath/listener.exe'"
    wsl bash -c $scpCmd
    if ($LASTEXITCODE -eq 0) {
        $listenerSuccess = $true
    }
} else {
    # Try native SCP (may prompt for password)
    scp -o StrictHostKeyChecking=no "$listenerPath" "$remotePath/listener.exe"
    if ($LASTEXITCODE -eq 0) {
        $listenerSuccess = $true
    }
}

# Upload dialer.exe
Write-Host "[DEPLOY] Uploading dialer.exe via SCP..." -ForegroundColor Cyan
$dialerSuccess = $false

if ($wslAvailable) {
    # Convert Windows path to WSL path
    $wslDialerPath = $dialerPath -replace '\\', '/' -replace '^([A-Z]):', { param($m) "/mnt/$($m.Groups[1].Value.ToLower())" }
    $wslDialerPath = $wslDialerPath -replace '^E:', '/mnt/e'
    $wslDialerPath = $wslDialerPath -replace '^e:', '/mnt/e'
    
    # Use sshpass with scp via WSL
    $scpCmd = "sshpass -p '$RSYNC_PASSWORD' scp -o StrictHostKeyChecking=no '$wslDialerPath' '$remotePath/dialer.exe'"
    wsl bash -c $scpCmd
    if ($LASTEXITCODE -eq 0) {
        $dialerSuccess = $true
    }
} else {
    # Try native SCP (may prompt for password)
    scp -o StrictHostKeyChecking=no "$dialerPath" "$remotePath/dialer.exe"
    if ($LASTEXITCODE -eq 0) {
        $dialerSuccess = $true
    }
}

# Report results
Write-Host ""
if ($listenerSuccess -and $dialerSuccess) {
    Write-Host "========================================" -ForegroundColor Green
    Write-Host "Upload successful!" -ForegroundColor Green
    Write-Host "========================================" -ForegroundColor Green
    Write-Host ""
    Write-Host "Files uploaded to:" -ForegroundColor White
    Write-Host "  $remotePath/listener.exe" -ForegroundColor Cyan
    Write-Host "  $remotePath/dialer.exe" -ForegroundColor Cyan
    Write-Host ""
} else {
    Write-Host "========================================" -ForegroundColor Red
    Write-Host "Upload completed with errors!" -ForegroundColor Red
    Write-Host "========================================" -ForegroundColor Red
    Write-Host ""
    if (-not $listenerSuccess) {
        Write-Host "  [FAILED] listener.exe" -ForegroundColor Red
    } else {
        Write-Host "  [SUCCESS] listener.exe" -ForegroundColor Green
    }
    if (-not $dialerSuccess) {
        Write-Host "  [FAILED] dialer.exe" -ForegroundColor Red
    } else {
        Write-Host "  [SUCCESS] dialer.exe" -ForegroundColor Green
    }
    Write-Host ""
    Write-Host "Troubleshooting:" -ForegroundColor Yellow
    Write-Host "  1. Check rsync server credentials" -ForegroundColor White
    Write-Host "  2. Verify network connectivity" -ForegroundColor White
    if ($wslAvailable) {
        Write-Host "  3. Ensure WSL has sshpass installed" -ForegroundColor White
        Write-Host '     wsl sudo apt-get update; sudo apt-get install -y sshpass' -ForegroundColor Gray
    } else {
        Write-Host "  3. Install WSL for password automation" -ForegroundColor White
        Write-Host "     wsl --install" -ForegroundColor Gray
        Write-Host "  4. Or use SCP manually" -ForegroundColor White
    }
    Write-Host ""
    exit 1
}

