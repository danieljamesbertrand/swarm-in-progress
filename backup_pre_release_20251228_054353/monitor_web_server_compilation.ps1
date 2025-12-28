# Monitor Web Server Compilation
# Tracks cargo build process and shows compilation progress

Write-Host ""
Write-Host "========================================" -ForegroundColor Cyan
Write-Host "  MONITORING WEB SERVER COMPILATION" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

$ErrorActionPreference = "Continue"

# Function to check if cargo/rustc is running
function Test-CompilationRunning {
    $cargo = Get-Process | Where-Object {$_.ProcessName -eq "cargo"} -ErrorAction SilentlyContinue
    $rustc = Get-Process | Where-Object {$_.ProcessName -eq "rustc"} -ErrorAction SilentlyContinue
    return ($cargo -ne $null -or $rustc -ne $null)
}

# Function to get compilation process info
function Get-CompilationInfo {
    $cargo = Get-Process | Where-Object {$_.ProcessName -eq "cargo"} -ErrorAction SilentlyContinue
    $rustc = Get-Process | Where-Object {$_.ProcessName -eq "rustc"} -ErrorAction SilentlyContinue
    
    $info = @{
        CargoRunning = ($cargo -ne $null)
        RustcRunning = ($rustc -ne $null)
        CargoCount = if ($cargo) { $cargo.Count } else { 0 }
        RustcCount = if ($rustc) { $rustc.Count } else { 0 }
        CargoPIDs = if ($cargo) { $cargo.Id } else { @() }
        RustcPIDs = if ($rustc) { $rustc.Id } else { @() }
    }
    
    return $info
}

# Check if compilation is already running
Write-Host "[CHECK] Checking for active compilation..." -ForegroundColor Yellow
$initialCheck = Get-CompilationInfo

if ($initialCheck.CargoRunning -or $initialCheck.RustcRunning) {
    Write-Host "[INFO] Compilation processes detected:" -ForegroundColor Green
    if ($initialCheck.CargoRunning) {
        Write-Host "  Cargo processes: $($initialCheck.CargoCount)" -ForegroundColor Cyan
        Write-Host "  Cargo PIDs: $($initialCheck.CargoPIDs -join ', ')" -ForegroundColor Gray
    }
    if ($initialCheck.RustcRunning) {
        Write-Host "  Rustc processes: $($initialCheck.RustcCount)" -ForegroundColor Cyan
        Write-Host "  Rustc PIDs: $($initialCheck.RustcPIDs -join ', ')" -ForegroundColor Gray
    }
} else {
    Write-Host "[INFO] No active compilation detected" -ForegroundColor Yellow
    Write-Host "[INFO] Starting compilation monitoring..." -ForegroundColor Yellow
    Write-Host ""
    Write-Host "To start compilation, run in another terminal:" -ForegroundColor Cyan
    Write-Host "  cargo run --bin web_server" -ForegroundColor White
    Write-Host ""
    Write-Host "Or:" -ForegroundColor Cyan
    Write-Host "  cargo build --bin web_server" -ForegroundColor White
    Write-Host ""
}

# Monitoring loop
$startTime = Get-Date
$lastStatusTime = $startTime
$maxWaitTime = 600  # 10 minutes max
$checkInterval = 2  # Check every 2 seconds
$statusInterval = 10  # Show status every 10 seconds
$wasRunning = $false
$compilationStarted = $false
$compilationFinished = $false

Write-Host "[MONITOR] Starting monitoring loop..." -ForegroundColor Yellow
Write-Host "  Check interval: $checkInterval seconds" -ForegroundColor Gray
Write-Host "  Status update: every $statusInterval seconds" -ForegroundColor Gray
Write-Host "  Max wait time: $maxWaitTime seconds" -ForegroundColor Gray
Write-Host ""

while ($true) {
    $currentTime = Get-Date
    $elapsed = ($currentTime - $startTime).TotalSeconds
    
    # Check if we've exceeded max wait time
    if ($elapsed -gt $maxWaitTime) {
        Write-Host ""
        Write-Host "[WARN] Maximum wait time ($maxWaitTime seconds) exceeded" -ForegroundColor Yellow
        break
    }
    
    # Get current compilation status
    $info = Get-CompilationInfo
    $isRunning = $info.CargoRunning -or $info.RustcRunning
    
    # Detect compilation start
    if ($isRunning -and -not $wasRunning) {
        $compilationStarted = $true
        Write-Host ""
        Write-Host "[COMPILATION STARTED]" -ForegroundColor Green
        Write-Host "  Time: $(Get-Date -Format 'HH:mm:ss')" -ForegroundColor Gray
        if ($info.CargoRunning) {
            Write-Host "  Cargo processes: $($info.CargoCount)" -ForegroundColor Cyan
        }
        if ($info.RustcRunning) {
            Write-Host "  Rustc processes: $($info.RustcCount)" -ForegroundColor Cyan
        }
        Write-Host ""
    }
    
    # Show periodic status updates
    $timeSinceLastStatus = ($currentTime - $lastStatusTime).TotalSeconds
    if ($isRunning -and $timeSinceLastStatus -ge $statusInterval) {
        $minutes = [math]::Floor($elapsed / 60)
        $seconds = [math]::Floor($elapsed % 60)
        Write-Host "[STATUS] Compilation in progress..." -ForegroundColor Yellow
        Write-Host "  Elapsed time: ${minutes}m ${seconds}s" -ForegroundColor Gray
        if ($info.CargoRunning) {
            Write-Host "  Cargo processes: $($info.CargoCount)" -ForegroundColor Cyan
        }
        if ($info.RustcRunning) {
            Write-Host "  Rustc processes: $($info.RustcCount)" -ForegroundColor Cyan
        }
        Write-Host ""
        $lastStatusTime = $currentTime
    }
    
    # Detect compilation completion
    if ($wasRunning -and -not $isRunning -and $compilationStarted) {
        $compilationFinished = $true
        $totalTime = ($currentTime - $startTime).TotalSeconds
        $minutes = [math]::Floor($totalTime / 60)
        $seconds = [math]::Floor($totalTime % 60)
        
        Write-Host ""
        Write-Host "[COMPILATION COMPLETED]" -ForegroundColor Green
        Write-Host "  Time: $(Get-Date -Format 'HH:mm:ss')" -ForegroundColor Gray
        Write-Host "  Total duration: ${minutes}m ${seconds}s" -ForegroundColor Cyan
        Write-Host ""
        
        # Check if web_server binary exists
        $releaseBinary = "target\release\web_server.exe"
        $debugBinary = "target\debug\web_server.exe"
        
        if (Test-Path $releaseBinary) {
            $binaryInfo = Get-Item $releaseBinary
            Write-Host "[BINARY] Release binary found:" -ForegroundColor Green
            Write-Host "  Path: $releaseBinary" -ForegroundColor Gray
            Write-Host "  Size: $([math]::Round($binaryInfo.Length / 1MB, 2)) MB" -ForegroundColor Gray
            Write-Host "  Modified: $($binaryInfo.LastWriteTime)" -ForegroundColor Gray
        } elseif (Test-Path $debugBinary) {
            $binaryInfo = Get-Item $debugBinary
            Write-Host "[BINARY] Debug binary found:" -ForegroundColor Green
            Write-Host "  Path: $debugBinary" -ForegroundColor Gray
            Write-Host "  Size: $([math]::Round($binaryInfo.Length / 1MB, 2)) MB" -ForegroundColor Gray
            Write-Host "  Modified: $($binaryInfo.LastWriteTime)" -ForegroundColor Gray
        } else {
            Write-Host "[WARN] Web server binary not found" -ForegroundColor Yellow
            Write-Host "  Checked: $releaseBinary" -ForegroundColor Gray
            Write-Host "  Checked: $debugBinary" -ForegroundColor Gray
        }
        
        Write-Host ""
        Write-Host "========================================" -ForegroundColor Cyan
        Write-Host ""
        break
    }
    
    $wasRunning = $isRunning
    
    # Sleep before next check
    Start-Sleep -Seconds $checkInterval
}

# Final status
if (-not $compilationFinished) {
    Write-Host ""
    Write-Host "[MONITOR] Monitoring stopped" -ForegroundColor Yellow
    $finalInfo = Get-CompilationInfo
    if ($finalInfo.CargoRunning -or $finalInfo.RustcRunning) {
        Write-Host "[INFO] Compilation may still be running" -ForegroundColor Yellow
        Write-Host "  Check manually or restart monitoring" -ForegroundColor Gray
    }
}

Write-Host ""
Write-Host "To check web server status:" -ForegroundColor Cyan
Write-Host "  .\check_web_server_status.ps1" -ForegroundColor White
Write-Host ""
Write-Host "To start the web server:" -ForegroundColor Cyan
Write-Host "  .\start_web_server.ps1" -ForegroundColor White
Write-Host ""

