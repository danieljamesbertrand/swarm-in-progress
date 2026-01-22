# Restore System from Backup
# Allows recovery of specific rollouts from rsync.net backups

param(
    [Parameter(Mandatory=$true)]
    [string]$BackupId,
    [string]$RemoteUser = "dbertrand",
    [string]$RemoteHost = "eagleoneonline.ca",
    [string]$RemoteDir = "/home/dbertrand/punch-simple",
    [string]$RsyncHost = "zh5605.rsync.net",
    [string]$RsyncUser = "zh5605",
    [string]$RsyncBaseDir = "/home/zh5605/backups/eagleoneonline",
    [switch]$DryRun = $false,
    [switch]$RestoreBinaries = $true,
    [switch]$RestoreSource = $true,
    [switch]$RestoreConfig = $true
)

Write-Host ""
Write-Host "╔══════════════════════════════════════════════════════════════╗" -ForegroundColor Cyan
Write-Host "║  RESTORE FROM BACKUP                                         ║" -ForegroundColor Cyan
Write-Host "╚══════════════════════════════════════════════════════════════╝" -ForegroundColor Cyan
Write-Host ""

Write-Host "Backup ID: $BackupId" -ForegroundColor Yellow
Write-Host "Server: $RemoteHost" -ForegroundColor Yellow
Write-Host "Dry Run: $DryRun" -ForegroundColor Yellow
Write-Host ""

# Step 1: Check if backup exists locally
Write-Host "[1/6] Checking for local backup..." -ForegroundColor Yellow
$localBackupPath = "$RemoteDir/.backups/$BackupId"
$checkLocalCmd = "ssh ${RemoteUser}@${RemoteHost} `"if [ -d $localBackupPath ]; then echo 'exists'; else echo 'missing'; fi`""
$localExists = Invoke-Expression $checkLocalCmd | Select-Object -Last 1

if ($localExists -eq "exists") {
    Write-Host "  [OK] Local backup found: $localBackupPath" -ForegroundColor Green
    $backupSource = "local"
} else {
    Write-Host "  [INFO] Local backup not found, will restore from rsync.net" -ForegroundColor Yellow
    $backupSource = "remote"
}

# Step 2: If remote, download from rsync.net
if ($backupSource -eq "remote") {
    Write-Host "[2/6] Downloading backup from rsync.net..." -ForegroundColor Yellow
    $rsyncSource = "${RsyncUser}@${RsyncHost}:${RsyncBaseDir}/$BackupId"
    $downloadCmd = "ssh ${RemoteUser}@${RemoteHost} `"mkdir -p $localBackupPath && rsync -avz $rsyncSource/ $localBackupPath/`""
    
    if ($DryRun) {
        Write-Host "  [DRY RUN] Would execute: $downloadCmd" -ForegroundColor Gray
    } else {
        try {
            Invoke-Expression $downloadCmd
            if ($LASTEXITCODE -eq 0) {
                Write-Host "  [OK] Backup downloaded from rsync.net" -ForegroundColor Green
            } else {
                Write-Host "  [ERROR] Failed to download backup" -ForegroundColor Red
                exit 1
            }
        } catch {
            Write-Host "  [ERROR] Download failed: $_" -ForegroundColor Red
            exit 1
        }
    }
} else {
    Write-Host "[2/6] Using local backup (skipping download)" -ForegroundColor Yellow
}

# Step 3: Read manifest
Write-Host "[3/6] Reading backup manifest..." -ForegroundColor Yellow
$manifestCmd = "ssh ${RemoteUser}@${RemoteHost} `"cat $localBackupPath/MANIFEST.txt 2>/dev/null || echo 'MANIFEST_NOT_FOUND'`""
$manifest = Invoke-Expression $manifestCmd

if ($manifest -match "MANIFEST_NOT_FOUND") {
    Write-Host "  [WARNING] Manifest not found, proceeding with restore anyway" -ForegroundColor Yellow
} else {
    Write-Host "  [OK] Manifest found" -ForegroundColor Green
    Write-Host ""
    Write-Host "Backup Information:" -ForegroundColor Cyan
    $manifestLines = $manifest -split "`n"
    foreach ($line in $manifestLines) {
        if ($line -match "^Backup ID:|^Backup Date:|^Git Commit:|^Backup Label:") {
            Write-Host "  $line" -ForegroundColor White
        }
    }
    Write-Host ""
}

# Step 4: Confirm restore
if (-not $DryRun) {
    Write-Host "[4/6] Confirming restore operation..." -ForegroundColor Yellow
    Write-Host "  This will restore:" -ForegroundColor White
    if ($RestoreSource) { Write-Host "    - Source code" -ForegroundColor White }
    if ($RestoreBinaries) { Write-Host "    - Compiled binaries" -ForegroundColor White }
    if ($RestoreConfig) { Write-Host "    - Configuration files" -ForegroundColor White }
    Write-Host ""
    Write-Host "  WARNING: This will overwrite current files!" -ForegroundColor Red
    $confirm = Read-Host "  Continue? (yes/no)"
    
    if ($confirm -ne "yes") {
        Write-Host "  [CANCELLED] Restore aborted by user" -ForegroundColor Yellow
        exit 0
    }
}

# Step 5: Restore files
Write-Host "[5/6] Restoring files..." -ForegroundColor Yellow

if ($RestoreSource) {
    Write-Host "  Restoring source code..." -ForegroundColor Gray
    $restoreSourceCmd = "ssh ${RemoteUser}@${RemoteHost} `"rsync -av --exclude='target/' --exclude='.git/' --exclude='.backups/' $localBackupPath/source/ $RemoteDir/`""
    if ($DryRun) {
        Write-Host "    [DRY RUN] Would execute: $restoreSourceCmd" -ForegroundColor Gray
    } else {
        Invoke-Expression $restoreSourceCmd | Out-Null
        Write-Host "    [OK] Source code restored" -ForegroundColor Green
    }
}

if ($RestoreBinaries) {
    Write-Host "  Restoring binaries..." -ForegroundColor Gray
    $restoreBinCmd = "ssh ${RemoteUser}@${RemoteHost} `"if [ -d $localBackupPath/binaries ]; then mkdir -p $RemoteDir/target/release && cp -r $localBackupPath/binaries/* $RemoteDir/target/release/ 2>/dev/null; echo 'Binaries restored'; else echo 'No binaries in backup'; fi`""
    if ($DryRun) {
        Write-Host "    [DRY RUN] Would execute: $restoreBinCmd" -ForegroundColor Gray
    } else {
        Invoke-Expression $restoreBinCmd | Out-Null
        Write-Host "    [OK] Binaries restored" -ForegroundColor Green
    }
}

if ($RestoreConfig) {
    Write-Host "  Restoring configuration files..." -ForegroundColor Gray
    $restoreConfigCmd = "ssh ${RemoteUser}@${RemoteHost} `"if [ -d $localBackupPath/config ]; then for file in $localBackupPath/config/*; do if [ -f `$file ]; then filename=`$(basename `$file); if [ `$filename = 'punch-rendezvous.service' ]; then sudo cp `$file /etc/systemd/system/; else cp `$file $RemoteDir/ 2>/dev/null || cp `$file ~/.cargo/ 2>/dev/null; fi; fi; done; echo 'Config restored'; else echo 'No config in backup'; fi`""
    if ($DryRun) {
        Write-Host "    [DRY RUN] Would execute: $restoreConfigCmd" -ForegroundColor Gray
    } else {
        Invoke-Expression $restoreConfigCmd | Out-Null
        Write-Host "    [OK] Configuration restored" -ForegroundColor Green
    }
}

# Step 6: Post-restore actions
Write-Host "[6/6] Post-restore actions..." -ForegroundColor Yellow

if (-not $DryRun) {
    Write-Host "  Reloading systemd..." -ForegroundColor Gray
    $reloadSystemd = "ssh ${RemoteUser}@${RemoteHost} `"sudo systemctl daemon-reload`""
    Invoke-Expression $reloadSystemd | Out-Null
    
    Write-Host "  [OK] Systemd reloaded" -ForegroundColor Green
    Write-Host ""
    Write-Host "  [INFO] You may need to:" -ForegroundColor Yellow
    Write-Host "    - Rebuild binaries: cargo build --release" -ForegroundColor White
    Write-Host "    - Restart service: sudo systemctl restart punch-rendezvous" -ForegroundColor White
    Write-Host "    - Verify deployment: Check server logs" -ForegroundColor White
}

Write-Host ""
Write-Host "╔══════════════════════════════════════════════════════════════╗" -ForegroundColor Green
Write-Host "║  RESTORE COMPLETE                                            ║" -ForegroundColor Green
Write-Host "╚══════════════════════════════════════════════════════════════╝" -ForegroundColor Green
Write-Host ""
Write-Host "Backup ID: $BackupId" -ForegroundColor Cyan
Write-Host "Restored from: $backupSource" -ForegroundColor White
Write-Host ""
