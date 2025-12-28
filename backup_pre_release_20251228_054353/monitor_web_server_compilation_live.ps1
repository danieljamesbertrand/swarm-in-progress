# Monitor Web Server Compilation with Live Output
# Runs cargo build and shows real-time compilation output

Write-Host ""
Write-Host "========================================" -ForegroundColor Cyan
Write-Host "  WEB SERVER COMPILATION MONITOR" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

$ErrorActionPreference = "Continue"

# Configuration
$buildMode = "run"  # "run" or "build"
$binName = "web_server"
$releaseBuild = $false

# Parse arguments
if ($args.Count -gt 0) {
    if ($args[0] -eq "release") {
        $releaseBuild = $true
        $buildMode = "build"
    } elseif ($args[0] -eq "build") {
        $buildMode = "build"
    } elseif ($args[0] -eq "run") {
        $buildMode = "run"
    }
}

# Display configuration
Write-Host "[CONFIG] Build configuration:" -ForegroundColor Yellow
Write-Host "  Mode: $buildMode" -ForegroundColor Cyan
Write-Host "  Binary: $binName" -ForegroundColor Cyan
if ($releaseBuild) {
    Write-Host "  Profile: release" -ForegroundColor Cyan
} else {
    Write-Host "  Profile: debug" -ForegroundColor Cyan
}
Write-Host ""

# Check if bootstrap is needed
$bootstrap = Get-Process | Where-Object {$_.ProcessName -eq "server"} -ErrorAction SilentlyContinue
if (-not $bootstrap -and $buildMode -eq "run") {
    Write-Host "[WARN] Bootstrap server not running" -ForegroundColor Yellow
    Write-Host "  The web server requires a bootstrap server" -ForegroundColor Gray
    Write-Host "  Start it with: cargo run --bin server -- --listen-addr 0.0.0.0 --port 51820" -ForegroundColor White
    Write-Host ""
    $response = Read-Host "Continue anyway? (y/n)"
    if ($response -ne "y" -and $response -ne "Y") {
        exit 0
    }
}

# Set environment variable for bootstrap
$env:BOOTSTRAP = "/ip4/127.0.0.1/tcp/51820"

# Build cargo command
$cargoArgs = @()
if ($buildMode -eq "build") {
    $cargoArgs += "build"
    if ($releaseBuild) {
        $cargoArgs += "--release"
    }
    $cargoArgs += "--bin"
    $cargoArgs += $binName
} else {
    $cargoArgs += "run"
    $cargoArgs += "--bin"
    $cargoArgs += $binName
}

$cargoCommand = "cargo $($cargoArgs -join ' ')"
Write-Host "[BUILD] Command: $cargoCommand" -ForegroundColor Cyan
Write-Host ""

# Start compilation with real-time output
$startTime = Get-Date
Write-Host "[COMPILATION STARTED] $(Get-Date -Format 'HH:mm:ss')" -ForegroundColor Green
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

try {
    # Run cargo directly to see real-time output
    Write-Host "Running cargo command..." -ForegroundColor Yellow
    Write-Host ""
    
    if ($buildMode -eq "run") {
        # For run mode, execute directly to see output
        & cargo run --bin $binName
        $exitCode = $LASTEXITCODE
    } else {
        # For build mode, execute directly
        if ($releaseBuild) {
            & cargo build --release --bin $binName
        } else {
            & cargo build --bin $binName
        }
        $exitCode = $LASTEXITCODE
    }
    
    $endTime = Get-Date
    $duration = ($endTime - $startTime).TotalSeconds
    $minutes = [math]::Floor($duration / 60)
    $seconds = [math]::Floor($duration % 60)
    
    Write-Host ""
    Write-Host "========================================" -ForegroundColor Cyan
    if ($exitCode -eq 0) {
        Write-Host "[COMPILATION SUCCESS]" -ForegroundColor Green
        Write-Host "  Duration: ${minutes}m ${seconds}s" -ForegroundColor Cyan
        Write-Host "  Completed: $(Get-Date -Format 'HH:mm:ss')" -ForegroundColor Gray
        Write-Host ""
        
        # Check binary
        if ($buildMode -eq "build") {
            if ($releaseBuild) {
                $binaryPath = "target\release\${binName}.exe"
            } else {
                $binaryPath = "target\debug\${binName}.exe"
            }
            
            if (Test-Path $binaryPath) {
                $binaryInfo = Get-Item $binaryPath
                Write-Host "[BINARY] Output:" -ForegroundColor Green
                Write-Host "  Path: $binaryPath" -ForegroundColor Gray
                Write-Host "  Size: $([math]::Round($binaryInfo.Length / 1MB, 2)) MB" -ForegroundColor Gray
                Write-Host "  Modified: $($binaryInfo.LastWriteTime)" -ForegroundColor Gray
            }
        } else {
            Write-Host "[INFO] Web server was started (run mode)" -ForegroundColor Green
            Write-Host "  Check if it's running on http://localhost:8080" -ForegroundColor Cyan
        }
    } else {
        Write-Host "[COMPILATION FAILED]" -ForegroundColor Red
        Write-Host "  Exit code: $exitCode" -ForegroundColor Red
        Write-Host "  Duration: ${minutes}m ${seconds}s" -ForegroundColor Gray
        Write-Host "  Check the output above for errors" -ForegroundColor Yellow
    }
    
} catch {
    Write-Host ""
    Write-Host "[ERROR] Exception during compilation:" -ForegroundColor Red
    Write-Host $_.Exception.Message -ForegroundColor Red
    Write-Host ""
}

Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

