# List All Available Backups
# Shows all backups from both local server and rsync.net

param(
    [string]$RemoteUser = "dbertrand",
    [string]$RemoteHost = "eagleoneonline.ca",
    [string]$RemoteDir = "/home/dbertrand/punch-simple",
    [string]$RsyncHost = "zh5605.rsync.net",
    [string]$RsyncUser = "zh5605",
    [string]$RsyncBaseDir = "/home/zh5605/backups/eagleoneonline"
)

Write-Host ""
Write-Host "╔══════════════════════════════════════════════════════════════╗" -ForegroundColor Cyan
Write-Host "║  AVAILABLE BACKUPS                                           ║" -ForegroundColor Cyan
Write-Host "╚══════════════════════════════════════════════════════════════╝" -ForegroundColor Cyan
Write-Host ""

# List local backups
Write-Host "Local Backups on $RemoteHost:" -ForegroundColor Yellow
$listLocalCmd = "ssh ${RemoteUser}@${RemoteHost} `"if [ -f $RemoteDir/.backups/backup-index.txt ]; then cat $RemoteDir/.backups/backup-index.txt; else echo 'No backup index found'; fi`""
$localBackups = Invoke-Expression $listLocalCmd

if ($localBackups -match "No backup index") {
    Write-Host "  No local backups found" -ForegroundColor Gray
} else {
    $backupLines = $localBackups -split "`n" | Where-Object { $_ -match "\|" }
    if ($backupLines.Count -eq 0) {
        Write-Host "  No backups in index" -ForegroundColor Gray
    } else {
        Write-Host ""
        Write-Host "  Backup ID                          | Date                | Label      | Commit" -ForegroundColor Cyan
        Write-Host "  " + ("-" * 100) -ForegroundColor Gray
        foreach ($line in $backupLines) {
            $parts = $line -split "\|"
            if ($parts.Count -ge 4) {
                $id = $parts[0].PadRight(35)
                $date = $parts[1].PadRight(20)
                $label = $parts[2].PadRight(12)
                $commit = if ($parts[3].Length -gt 8) { $parts[3].Substring(0, 8) } else { $parts[3] }
                Write-Host "  $id | $date | $label | $commit" -ForegroundColor White
            }
        }
    }
}

Write-Host ""
Write-Host "Remote Backups on rsync.net:" -ForegroundColor Yellow
$listRemoteCmd = "ssh -o StrictHostKeyChecking=no ${RsyncUser}@${RsyncHost} `"ls -1d ${RsyncBaseDir}/*/ 2>/dev/null | xargs -I {} basename {}`""
try {
    $remoteBackups = Invoke-Expression $listRemoteCmd 2>&1
    if ($remoteBackups -match "Permission denied" -or $remoteBackups -match "Connection refused") {
        Write-Host "  [WARNING] Could not connect to rsync.net" -ForegroundColor Yellow
        Write-Host "  [INFO] Manual check: ssh ${RsyncUser}@${RsyncHost} 'ls ${RsyncBaseDir}/'" -ForegroundColor Gray
    } elseif ($remoteBackups.Count -eq 0) {
        Write-Host "  No remote backups found" -ForegroundColor Gray
    } else {
        $remoteBackupList = $remoteBackups -split "`n" | Where-Object { $_ -and $_ -notmatch "Permission|Connection" }
        foreach ($backup in $remoteBackupList) {
            Write-Host "  - $backup" -ForegroundColor White
        }
    }
} catch {
    Write-Host "  [WARNING] Could not list remote backups: $_" -ForegroundColor Yellow
}

Write-Host ""
Write-Host "To restore a backup:" -ForegroundColor Yellow
Write-Host "  .\restore_from_backup.ps1 -BackupId <backup-id>" -ForegroundColor Gray
Write-Host ""
Write-Host "To view backup details:" -ForegroundColor Yellow
Write-Host "  ssh $RemoteUser@$RemoteHost 'cat $RemoteDir/.backups/<backup-id>/MANIFEST.txt'" -ForegroundColor Gray
Write-Host ""
