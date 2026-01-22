# Install Backup Service on eagleoneonline.ca
# Creates systemd service for automated backups on boot and scheduled intervals

param(
    [string]$RemoteUser = "dbertrand",
    [string]$RemoteHost = "eagleoneonline.ca",
    [string]$RemoteDir = "/home/dbertrand/punch-simple"
)

Write-Host ""
Write-Host "╔══════════════════════════════════════════════════════════════╗" -ForegroundColor Cyan
Write-Host "║  INSTALL BACKUP SERVICE                                      ║" -ForegroundColor Cyan
Write-Host "╚══════════════════════════════════════════════════════════════╝" -ForegroundColor Cyan
Write-Host ""

# Step 1: Upload backup service script
Write-Host "[1/5] Uploading backup service script..." -ForegroundColor Yellow
$backupScriptLocal = "backup_service.sh"
$backupScriptRemote = "$RemoteDir/backup_service.sh"

if (-not (Test-Path $backupScriptLocal)) {
    Write-Host "  [ERROR] Backup script not found: $backupScriptLocal" -ForegroundColor Red
    exit 1
}

$uploadCmd = "scp -F NUL `"$backupScriptLocal`" ${RemoteUser}@${RemoteHost}:${backupScriptRemote}"
try {
    Invoke-Expression $uploadCmd | Out-Null
    Write-Host "  [OK] Backup script uploaded" -ForegroundColor Green
} catch {
    Write-Host "  [ERROR] Upload failed: $_" -ForegroundColor Red
    exit 1
}

# Step 2: Make script executable
Write-Host "[2/5] Making script executable..." -ForegroundColor Yellow
$chmodCmd = "ssh ${RemoteUser}@${RemoteHost} `"chmod +x $backupScriptRemote`""
try {
    Invoke-Expression $chmodCmd | Out-Null
    Write-Host "  [OK] Script is now executable" -ForegroundColor Green
} catch {
    Write-Host "  [ERROR] Failed to make executable: $_" -ForegroundColor Red
    exit 1
}

# Step 3: Create systemd service file
Write-Host "[3/5] Creating systemd service..." -ForegroundColor Yellow
$serviceContent = @"
[Unit]
Description=Punch Simple Backup Service
After=network-online.target
Wants=network-online.target

[Service]
Type=oneshot
User=$RemoteUser
WorkingDirectory=$RemoteDir
ExecStart=$backupScriptRemote auto
StandardOutput=append:$RemoteDir/.backups/backup-service.log
StandardError=append:$RemoteDir/.backups/backup-service.log

[Install]
WantedBy=multi-user.target
"@

$tempDir = $env:TEMP
$serviceFileName = 'punch-backup.service'
$serviceFile = Join-Path -Path $tempDir -ChildPath $serviceFileName
$serviceFileRemote = '/etc/systemd/system/punch-backup.service'

# Write service file locally first
$serviceContent | Out-File -FilePath $serviceFile -Encoding UTF8

# Upload service file
$uploadServiceCmd = "scp -F NUL `"$serviceFile`" ${RemoteUser}@${RemoteHost}:/tmp/punch-backup.service"
try {
    Invoke-Expression $uploadServiceCmd | Out-Null
    
    # Move to systemd directory (requires sudo)
    $moveServiceCmd = "ssh ${RemoteUser}@${RemoteHost} `"sudo mv /tmp/punch-backup.service $serviceFileRemote; sudo chmod 644 $serviceFileRemote`""
    Invoke-Expression $moveServiceCmd | Out-Null
    
    Write-Host "  [OK] Systemd service file created" -ForegroundColor Green
} catch {
    Write-Host "  [ERROR] Failed to create service file: $_" -ForegroundColor Red
    exit 1
} finally {
    Remove-Item $serviceFile -ErrorAction SilentlyContinue
}

# Step 4: Create timer for scheduled backups
Write-Host "[4/5] Creating systemd timer for scheduled backups..." -ForegroundColor Yellow
$timerContent = @"
[Unit]
Description=Run Punch Simple Backup Service daily
Requires=punch-backup.service

[Timer]
OnBootSec=1h
OnUnitActiveSec=24h
Persistent=true

[Install]
WantedBy=timers.target
"@

$timerFileName = 'punch-backup.timer'
$timerFile = Join-Path -Path $tempDir -ChildPath $timerFileName
$timerFileRemote = '/etc/systemd/system/punch-backup.timer'

$timerContent | Out-File -FilePath $timerFile -Encoding UTF8

$uploadTimerCmd = "scp -F NUL `"$timerFile`" ${RemoteUser}@${RemoteHost}:/tmp/punch-backup.timer"
try {
    Invoke-Expression $uploadTimerCmd | Out-Null
    
    $moveTimerCmd = "ssh ${RemoteUser}@${RemoteHost} `"sudo mv /tmp/punch-backup.timer $timerFileRemote; sudo chmod 644 $timerFileRemote`""
    Invoke-Expression $moveTimerCmd | Out-Null
    
    Write-Host "  [OK] Systemd timer created" -ForegroundColor Green
} catch {
    Write-Host "  [ERROR] Failed to create timer: $_" -ForegroundColor Red
    exit 1
} finally {
    Remove-Item $timerFile -ErrorAction SilentlyContinue
}

# Step 5: Enable and start service
Write-Host "[5/5] Enabling and starting backup service..." -ForegroundColor Yellow
$enableCmd = "ssh ${RemoteUser}@${RemoteHost} `"sudo systemctl daemon-reload; sudo systemctl enable punch-backup.timer; sudo systemctl start punch-backup.timer`""
try {
    Invoke-Expression $enableCmd | Out-Null
    
    # Check status
    $statusCmd = "ssh ${RemoteUser}@${RemoteHost} `"sudo systemctl status punch-backup.timer --no-pager`""
    $status = Invoke-Expression $statusCmd
    
    Write-Host "  [OK] Backup service enabled and started" -ForegroundColor Green
    Write-Host ""
    Write-Host "Service Status:" -ForegroundColor Cyan
    Write-Host $status -ForegroundColor White
} catch {
    Write-Host "  [ERROR] Failed to enable service: $_" -ForegroundColor Red
    exit 1
}

Write-Host ""
Write-Host "╔══════════════════════════════════════════════════════════════╗" -ForegroundColor Green
Write-Host "║  BACKUP SERVICE INSTALLED                                    ║" -ForegroundColor Green
Write-Host "╚══════════════════════════════════════════════════════════════╝" -ForegroundColor Green
Write-Host ""
Write-Host "Service Information:" -ForegroundColor Yellow
Write-Host "  Service: punch-backup.service" -ForegroundColor White
Write-Host "  Timer: punch-backup.timer" -ForegroundColor White
Write-Host "  Schedule: Daily (24 hours), starts 1 hour after boot" -ForegroundColor White
Write-Host "  Log: $RemoteDir/.backups/backup-service.log" -ForegroundColor White
Write-Host ""
Write-Host "Useful Commands:" -ForegroundColor Yellow
Write-Host "  Check timer status: ssh $RemoteUser@$RemoteHost 'sudo systemctl status punch-backup.timer'" -ForegroundColor Gray
Write-Host "  Check service logs: ssh $RemoteUser@$RemoteHost 'tail -f $RemoteDir/.backups/backup-service.log'" -ForegroundColor Gray
Write-Host "  Manual backup: ssh $RemoteUser@$RemoteHost '$RemoteDir/backup_service.sh manual deployment-now'" -ForegroundColor Gray
Write-Host "  List backups: .\list_backups.ps1" -ForegroundColor Gray
Write-Host ""
