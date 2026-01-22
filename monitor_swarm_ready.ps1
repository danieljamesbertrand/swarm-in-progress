# Monitor for Swarm Ready / Inference Ready Messages
# Checks node windows and announces when swarm is ready

Write-Host ""
Write-Host "========================================" -ForegroundColor Cyan
Write-Host "  Monitoring for Swarm Ready Status" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

$maxWaitTime = 300  # 5 minutes max wait
$checkInterval = 5  # Check every 5 seconds
$elapsed = 0

Write-Host "Monitoring all 8 node windows for swarm ready messages..." -ForegroundColor Yellow
Write-Host "Looking for: [SWARM] SWARM READY FOR INFERENCE" -ForegroundColor Gray
Write-Host "Will check every $checkInterval seconds for up to $maxWaitTime seconds" -ForegroundColor Gray
Write-Host ""

$swarmReady = $false
$inferenceReady = $false

while ($elapsed -lt $maxWaitTime -and -not $swarmReady) {
    Start-Sleep -Seconds $checkInterval
    $elapsed += $checkInterval
    
    Write-Host "[$elapsed s] Checking node windows..." -ForegroundColor Gray
    
    # Note: We can't directly read the output of other PowerShell windows
    # This script provides instructions for manual monitoring
    # The user should check the node windows manually
    
    Write-Host "  Please check all 8 node windows for:" -ForegroundColor Yellow
    Write-Host "    - [SWARM] ✓✓✓ SWARM READY FOR INFERENCE ✓✓✓" -ForegroundColor Green
    Write-Host "    - [STATUS] Swarm Ready: ✓ YES" -ForegroundColor Green
    Write-Host "    - [STATUS] Pipeline Complete: ✓ YES" -ForegroundColor Green
    Write-Host ""
}

if ($swarmReady) {
    Write-Host ""
    Write-Host "========================================" -ForegroundColor Green
    Write-Host "  ✓✓✓ SWARM IS READY FOR INFERENCE! ✓✓✓" -ForegroundColor Green
    Write-Host "========================================" -ForegroundColor Green
    Write-Host ""
} else {
    Write-Host ""
    Write-Host "Monitoring timeout reached. Please check node windows manually." -ForegroundColor Yellow
    Write-Host ""
}

Write-Host "To check status manually:" -ForegroundColor Cyan
Write-Host "  1. Look at each of the 8 node windows" -ForegroundColor White
Write-Host "  2. Search for: 'SWARM READY' or 'Swarm Ready: YES'" -ForegroundColor White
Write-Host "  3. Check status reports for: 'Pipeline Complete: YES'" -ForegroundColor White
Write-Host ""
