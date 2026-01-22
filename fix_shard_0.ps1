# Fix shard-0.gguf by replacing the oversized file with the correct one

$oldFile = "E:\rust\punch-orig\models_cache\shards\shard-0.gguf"
$newFile = "E:\rust\punch-orig\models_cache\shards\shard-new-0.gguf"
$tempFile = "E:\rust\punch-orig\models_cache\shards\shard-0-fixed.gguf"

Write-Host ""
Write-Host "Fixing shard-0.gguf..." -ForegroundColor Cyan
Write-Host ""

if (-not (Test-Path $newFile)) {
    Write-Host "ERROR: shard-new-0.gguf not found!" -ForegroundColor Red
    exit 1
}

# Check current shard-0 size
if (Test-Path $oldFile) {
    $oldSize = (Get-Item $oldFile).Length / 1GB
    Write-Host "Current shard-0.gguf size: $([math]::Round($oldSize, 2)) GB" -ForegroundColor Yellow
    
    if ($oldSize -lt 1) {
        Write-Host "File is already correct size, no fix needed." -ForegroundColor Green
        exit 0
    }
}

# Try multiple approaches
$success = $false

# Approach 1: Copy to temp, then replace
Write-Host "Attempting fix (multiple retries)..." -ForegroundColor Yellow
for ($i = 1; $i -le 5; $i++) {
    Write-Host "  Attempt $i/5..." -ForegroundColor Gray
    
    try {
        # Copy new file to temp location
        Copy-Item $newFile $tempFile -Force -ErrorAction Stop
        
        # Wait a bit
        Start-Sleep -Seconds 2
        
        # Try to remove old file
        if (Test-Path $oldFile) {
            Remove-Item $oldFile -Force -ErrorAction Stop
        }
        
        # Wait a bit more
        Start-Sleep -Seconds 1
        
        # Move temp to final location
        Move-Item $tempFile $oldFile -Force -ErrorAction Stop
        
        $success = $true
        Write-Host "  Success!" -ForegroundColor Green
        break
    } catch {
        Write-Host "  Failed: $($_.Exception.Message)" -ForegroundColor Red
        Start-Sleep -Seconds (2 * $i)  # Exponential backoff
    }
}

if (-not $success) {
    Write-Host ""
    Write-Host "Could not replace file - it appears to be locked by another process." -ForegroundColor Red
    Write-Host ""
    Write-Host "Please:" -ForegroundColor Yellow
    Write-Host "  1. Close any programs that might be using shard-0.gguf" -ForegroundColor White
    Write-Host "  2. Restart your computer if needed" -ForegroundColor White
    Write-Host "  3. Then run this script again" -ForegroundColor White
    Write-Host ""
    Write-Host "Alternatively, manually:" -ForegroundColor Yellow
    Write-Host "  - Delete: models_cache\shards\shard-0.gguf" -ForegroundColor White
    Write-Host "  - Rename: models_cache\shards\shard-new-0.gguf -> shard-0.gguf" -ForegroundColor White
    Write-Host ""
    exit 1
}

# Verify the fix
if (Test-Path $oldFile) {
    $newSize = (Get-Item $oldFile).Length / 1MB
    Write-Host ""
    Write-Host "Verification:" -ForegroundColor Green
    Write-Host "  shard-0.gguf is now $([math]::Round($newSize, 0)) MB" -ForegroundColor White
    
    if ($newSize -lt 600) {
        Write-Host "  [OK] File is correct size!" -ForegroundColor Green
    } else {
        Write-Host "  [WARNING] File size still seems wrong" -ForegroundColor Yellow
    }
}

# Clean up temp files
Remove-Item $tempFile -Force -ErrorAction SilentlyContinue
Remove-Item $newFile -Force -ErrorAction SilentlyContinue

Write-Host ""
Write-Host "Done!" -ForegroundColor Green
Write-Host ""
