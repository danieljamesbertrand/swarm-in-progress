# PowerShell wrapper for Python GGUF splitter
# Usage: .\split_gguf_proper.ps1 [gguf_file] [num_shards]

param(
    [string]$GgufFile = "",
    [int]$NumShards = 8
)

$LocalCache = "models_cache"
$ShardOutputDir = Join-Path $LocalCache "shards"

Write-Host ""
Write-Host "GGUF Proper Splitter (Respects Tensor Boundaries)" -ForegroundColor Cyan
Write-Host ""

# Check if Python is available
$pythonCmd = $null
if (Get-Command python -ErrorAction SilentlyContinue) {
    $pythonCmd = "python"
} elseif (Get-Command python3 -ErrorAction SilentlyContinue) {
    $pythonCmd = "python3"
} else {
    Write-Host "Error: Python not found. Please install Python 3." -ForegroundColor Red
    Write-Host "The proper GGUF splitter requires Python to understand the file format." -ForegroundColor Yellow
    exit 1
}

# Find .gguf file if not specified
if ([string]::IsNullOrEmpty($GgufFile)) {
    $ggufFiles = Get-ChildItem "$LocalCache\*.gguf" -ErrorAction SilentlyContinue
    if ($ggufFiles.Count -eq 0) {
        Write-Host "No .gguf files found in $LocalCache" -ForegroundColor Red
        Write-Host "Please download a .gguf file first" -ForegroundColor Yellow
        exit 1
    } elseif ($ggufFiles.Count -eq 1) {
        $GgufFile = $ggufFiles[0].FullName
        Write-Host "Using: $($ggufFiles[0].Name)" -ForegroundColor Green
    } else {
        Write-Host "Multiple .gguf files found. Please specify which one:" -ForegroundColor Yellow
        $index = 1
        foreach ($file in $ggufFiles) {
            $sizeGB = [math]::Round($file.Length / 1GB, 2)
            Write-Host "  $index. $($file.Name) ($sizeGB GB)" -ForegroundColor White
            $index++
        }
        $choice = Read-Host "Enter number"
        $GgufFile = $ggufFiles[$choice - 1].FullName
    }
}

if (-not (Test-Path $GgufFile)) {
    Write-Host "File not found: $GgufFile" -ForegroundColor Red
    exit 1
}

Write-Host "Running Python GGUF splitter..." -ForegroundColor Cyan
Write-Host ""

# Run the Python script
$scriptPath = Join-Path $PSScriptRoot "split_gguf_proper.py"
& $pythonCmd $scriptPath $GgufFile $NumShards $ShardOutputDir

if ($LASTEXITCODE -eq 0) {
    Write-Host ""
    Write-Host "Shards created successfully!" -ForegroundColor Green
    
    # Show created shards
    Write-Host ""
    Write-Host "Created shards:" -ForegroundColor Cyan
    Get-ChildItem "$ShardOutputDir\shard-*.gguf" -ErrorAction SilentlyContinue | Sort-Object Name | ForEach-Object {
        $sizeMB = [math]::Round($_.Length / 1MB, 2)
        Write-Host ("  {0,-30} {1,10} MB" -f $_.Name, $sizeMB) -ForegroundColor White
    }
} else {
    Write-Host ""
    Write-Host "Error: GGUF splitting failed" -ForegroundColor Red
    Write-Host "You may need to install Python dependencies or use an alternative method" -ForegroundColor Yellow
}

Write-Host ""






