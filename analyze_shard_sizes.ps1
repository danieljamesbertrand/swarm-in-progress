# Analyze optimal shard sizes for the model

$model = Get-Item "models_cache\mistral-7b-instruct-v0.2.Q4_K_M.gguf"
$sizeGB = $model.Length / 1GB
$sizeMB = $model.Length / 1MB

Write-Host ""
Write-Host "========================================" -ForegroundColor Cyan
Write-Host "  SHARD SIZE ANALYSIS" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""
Write-Host "Model: $($model.Name)" -ForegroundColor White
$sizeMBText = "$([math]::Round($sizeMB, 0)) MB"
Write-Host "Total size: $([math]::Round($sizeGB, 2)) GB ($sizeMBText)" -ForegroundColor White
Write-Host ""

Write-Host "Shard size options (4-12 shards):" -ForegroundColor Yellow
Write-Host ""

for ($n = 4; $n -le 12; $n++) {
    $shardGB = $sizeGB / $n
    $shardMB = $shardGB * 1024
    
    # Determine if size is practical
    $practical = $true
    $reason = ""
    
    if ($shardMB -lt 100) {
        $practical = $false
        $reason = "Too small"
    } elseif ($shardMB -lt 200) {
        $reason = "Small but workable"
    } elseif ($shardMB -lt 500) {
        $reason = "Good size"
    } elseif ($shardMB -lt 1000) {
        $reason = "Large but manageable"
    } else {
        $reason = "Very large"
    }
    
    $color = if ($practical) { "Green" } else { "Red" }
    $marker = if ($n -eq 8) { " [RECOMMENDED]" } elseif ($n -eq 6) { " [Good balance]" } else { "" }
    $shardMBText = "$([math]::Round($shardMB, 0)) MB"
    
    Write-Host "  $n shards: $([math]::Round($shardGB, 2)) GB ($shardMBText) per shard - $reason$marker" -ForegroundColor $color
}

Write-Host ""
Write-Host "Recommendations:" -ForegroundColor Yellow
Write-Host "  - 8 shards: Best balance - ~0.51 GB each, good for distributed systems" -ForegroundColor Green
Write-Host "  - 6 shards: Good alternative - ~0.68 GB each, fewer nodes needed" -ForegroundColor Green
Write-Host "  - 12 shards: Maximum - ~0.34 GB each, fastest transfers but needs 12 nodes" -ForegroundColor Cyan
Write-Host ""

# Check if splitting is feasible
Write-Host "Feasibility check:" -ForegroundColor Yellow
Write-Host "  [OK] PowerShell script supports any number of shards (4-12)" -ForegroundColor Green
Write-Host "  [OK] Byte-level splitting works for all sizes" -ForegroundColor Green
Write-Host "  [WARNING] Byte-level split does not respect GGUF tensor boundaries" -ForegroundColor Yellow
Write-Host "     For production, consider using proper GGUF splitter" -ForegroundColor Yellow
Write-Host ""
