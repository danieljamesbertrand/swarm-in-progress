# Sophisticated Backup System for eagleoneonline.ca
# Creates timestamped backups with full metadata tracking
# Backs up to rsync.net for long-term storage and recovery

param(
    [string]$RemoteUser = "dbertrand",
    [string]$RemoteHost = "eagleoneonline.ca",
    [string]$RemoteDir = "/home/dbertrand/punch-simple",
    [string]$BackupLabel = "",
    [string]$GitCommit = "",
    [switch]$SkipDeployment = $false
)

# Rsync.net credentials
$RsyncHost = "zh5605.rsync.net"
$RsyncUser = "zh5605"
$RsyncPassword = "3da393f1"
$RsyncBaseDir = "/home/zh5605/backups/eagleoneonline"

Write-Host ""
Write-Host "╔══════════════════════════════════════════════════════════════╗" -ForegroundColor Cyan
Write-Host "║  SOPHISTICATED BACKUP SYSTEM - eagleoneonline.ca            ║" -ForegroundColor Cyan
Write-Host "╚══════════════════════════════════════════════════════════════╝" -ForegroundColor Cyan
Write-Host ""

# Generate backup timestamp and label
$timestamp = Get-Date -Format "yyyyMMdd-HHmmss"
$backupId = if ($BackupLabel) { "$BackupLabel-$timestamp" } else { "backup-$timestamp" }
$backupDate = Get-Date -Format "yyyy-MM-dd HH:mm:ss UTC"

Write-Host "[BACKUP] Creating backup: $backupId" -ForegroundColor Yellow
Write-Host "[BACKUP] Timestamp: $backupDate" -ForegroundColor Gray
Write-Host ""

# Step 1: Create backup directory on remote server
Write-Host "[1/8] Creating backup staging directory on server..." -ForegroundColor Yellow
$stagingDir = "$RemoteDir/.backups/$backupId"
$createDirCmd = "ssh ${RemoteUser}@${RemoteHost} `"mkdir -p $stagingDir`""
try {
    Invoke-Expression $createDirCmd | Out-Null
    Write-Host "  [OK] Staging directory created: $stagingDir" -ForegroundColor Green
} catch {
    Write-Host "  [ERROR] Failed to create staging directory: $_" -ForegroundColor Red
    exit 1
}

# Step 2: Collect system information
Write-Host "[2/8] Collecting system information..." -ForegroundColor Yellow
$systemInfo = @"
# System Information
Backup ID: $backupId
Backup Date: $backupDate
Server: $RemoteHost
Backup Label: $BackupLabel
Git Commit: $GitCommit

# Server Information
"@

$serverInfoCmd = "ssh ${RemoteUser}@${RemoteHost} `"uname -a; hostname; uptime; df -h $RemoteDir`""
try {
    $serverInfo = Invoke-Expression $serverInfoCmd
    $systemInfo += "`n$serverInfo`n"
    Write-Host "  [OK] System information collected" -ForegroundColor Green
} catch {
    Write-Host "  [WARNING] Could not collect all system info: $_" -ForegroundColor Yellow
}

# Step 3: Get Git information (if available)
Write-Host "[3/8] Collecting Git repository information..." -ForegroundColor Yellow
$gitInfo = @"
# Git Repository Information
"@

$gitCommands = @(
    "cd $RemoteDir && git rev-parse HEAD 2>/dev/null || echo 'N/A'",
    "cd $RemoteDir && git rev-parse --short HEAD 2>/dev/null || echo 'N/A'",
    "cd $RemoteDir && git log -1 --pretty=format:'%H|%an|%ae|%ad|%s' --date=iso 2>/dev/null || echo 'N/A'",
    "cd $RemoteDir && git branch --show-current 2>/dev/null || echo 'N/A'",
    "cd $RemoteDir && git remote -v 2>/dev/null || echo 'N/A'",
    "cd $RemoteDir && git status --short 2>/dev/null || echo 'N/A'"
)

$gitInfo += "`n## Commit Information`n"
foreach ($cmd in $gitCommands) {
    try {
        $result = Invoke-Expression "ssh ${RemoteUser}@${RemoteHost} `"$cmd`""
        $gitInfo += "$result`n"
    } catch {
        $gitInfo += "Error: $_`n"
    }
}

Write-Host "  [OK] Git information collected" -ForegroundColor Green

# Step 4: Backup source code
Write-Host "[4/8] Backing up source code..." -ForegroundColor Yellow
$sourceBackupDir = "$stagingDir/source"
$sourceBackupCmd = "ssh ${RemoteUser}@${RemoteHost} `"mkdir -p $sourceBackupDir && rsync -av --exclude='target/' --exclude='.git/' --exclude='.backups/' $RemoteDir/ $sourceBackupDir/`""
try {
    Invoke-Expression $sourceBackupCmd | Out-Null
    Write-Host "  [OK] Source code backed up" -ForegroundColor Green
} catch {
    Write-Host "  [ERROR] Source backup failed: $_" -ForegroundColor Red
    exit 1
}

# Step 5: Backup binaries
Write-Host "[5/8] Backing up compiled binaries..." -ForegroundColor Yellow
$binBackupDir = "$stagingDir/binaries"
$binBackupCmd = "ssh ${RemoteUser}@${RemoteHost} `"if [ -d $RemoteDir/target/release ]; then mkdir -p $binBackupDir && cp -r $RemoteDir/target/release/* $binBackupDir/ 2>/dev/null; echo 'Binaries backed up'; else echo 'No binaries found'; fi`""
try {
    $binResult = Invoke-Expression $binBackupCmd
    Write-Host "  [OK] Binaries backed up" -ForegroundColor Green
} catch {
    Write-Host "  [WARNING] Binary backup failed (may not exist): $_" -ForegroundColor Yellow
}

# Step 6: Backup configuration files
Write-Host "[6/8] Backing up configuration files..." -ForegroundColor Yellow
$configBackupDir = "$stagingDir/config"
$configFiles = @(
    "/etc/systemd/system/punch-rendezvous.service",
    "/home/dbertrand/.cargo/config.toml",
    "$RemoteDir/.env",
    "$RemoteDir/Cargo.toml"
)

$configBackupCmd = "ssh ${RemoteUser}@${RemoteHost} `"mkdir -p $configBackupDir && "
foreach ($configFile in $configFiles) {
    $configBackupCmd += "if [ -f $configFile ]; then cp $configFile $configBackupDir/ 2>/dev/null; fi; "
}
$configBackupCmd += "echo 'Config files backed up'`""

try {
    Invoke-Expression $configBackupCmd | Out-Null
    Write-Host "  [OK] Configuration files backed up" -ForegroundColor Green
} catch {
    Write-Host "  [WARNING] Some config files may not exist: $_" -ForegroundColor Yellow
}

# Step 7: Backup shard files metadata (not the files themselves - too large)
Write-Host "[7/8] Backing up shard files metadata..." -ForegroundColor Yellow
$shardBackupDir = "$stagingDir/shard-metadata"
$shardMetadataCmd = "ssh ${RemoteUser}@${RemoteHost} `"mkdir -p $shardBackupDir && if [ -d $RemoteDir/shards ]; then ls -lh $RemoteDir/shards/ > $shardBackupDir/file-list.txt 2>/dev/null; find $RemoteDir/shards -type f -exec sha256sum {} \; > $shardBackupDir/sha256sums.txt 2>/dev/null; echo 'Shard metadata backed up'; else echo 'No shards directory'; fi`""
try {
    Invoke-Expression $shardMetadataCmd | Out-Null
    Write-Host "  [OK] Shard metadata backed up" -ForegroundColor Green
} catch {
    Write-Host "  [WARNING] Shard metadata backup failed: $_" -ForegroundColor Yellow
}

# Step 8: Create backup manifest
Write-Host "[8/8] Creating backup manifest..." -ForegroundColor Yellow
$manifest = @"
# Backup Manifest
# Generated: $backupDate

## Backup Information
Backup ID: $backupId
Backup Label: $BackupLabel
Backup Date: $backupDate
Server: $RemoteHost
Remote Directory: $RemoteDir

## Deployment Information
Git Commit: $GitCommit
Deployment Label: $BackupLabel

## Backup Contents
- Source Code: $sourceBackupDir
- Binaries: $binBackupDir
- Configuration: $configBackupDir
- Shard Metadata: $shardBackupDir

## System Information
$systemInfo

## Git Information
$gitInfo

## Backup Location
Local Staging: $stagingDir
Remote Storage: rsync://${RsyncUser}@${RsyncHost}${RsyncBaseDir}/$backupId

## Recovery Instructions
To restore this backup:
1. SSH to server: ssh $RemoteUser@$RemoteHost
2. Navigate to backup: cd $stagingDir
3. Review contents: ls -la
4. Restore files as needed

Or use restore script:
  .\restore_from_backup.ps1 -BackupId $backupId
"@

$manifestPath = "$stagingDir/MANIFEST.txt"
$manifestCmd = "ssh ${RemoteUser}@${RemoteHost} `"cat > $manifestPath << 'EOF'
$manifest
EOF
`""

try {
    Invoke-Expression $manifestCmd | Out-Null
    Write-Host "  [OK] Manifest created" -ForegroundColor Green
} catch {
    Write-Host "  [ERROR] Manifest creation failed: $_" -ForegroundColor Red
    exit 1
}

# Step 9: Upload to rsync.net
Write-Host ""
Write-Host "[9/9] Uploading backup to rsync.net..." -ForegroundColor Yellow
Write-Host "  Destination: rsync://${RsyncUser}@${RsyncHost}${RsyncBaseDir}/" -ForegroundColor Gray

# Set up SSH key or use password authentication
# For rsync.net, we'll use SSH with password (via sshpass if available, or manual)
$rsyncDest = "${RsyncUser}@${RsyncHost}:${RsyncBaseDir}/$backupId"

# Use rsync with SSH
# Note: rsync.net supports SSH key authentication (recommended) or password
$rsyncCmd = "ssh ${RemoteUser}@${RemoteHost} `"rsync -avz --progress -e 'ssh -o StrictHostKeyChecking=no' $stagingDir/ $rsyncDest/`""

Write-Host "  [INFO] Uploading backup..." -ForegroundColor Gray
Write-Host "  [INFO] This may take several minutes depending on backup size..." -ForegroundColor Gray

try {
    # First, test SSH connection to rsync.net
    $testConnection = "ssh -o StrictHostKeyChecking=no -o ConnectTimeout=10 ${RsyncUser}@${RsyncHost} 'echo Connection successful' 2>&1"
    $testResult = Invoke-Expression $testConnection
    
    if ($LASTEXITCODE -eq 0 -or $testResult -match "successful") {
        # Create remote directory first
        $createRemoteDir = "ssh -o StrictHostKeyChecking=no ${RsyncUser}@${RsyncHost} `"mkdir -p ${RsyncBaseDir}/$backupId`""
        Invoke-Expression $createRemoteDir | Out-Null
        
        # Upload backup
        $uploadResult = Invoke-Expression $rsyncCmd 2>&1
        if ($LASTEXITCODE -eq 0) {
            Write-Host "  [OK] Backup uploaded to rsync.net successfully" -ForegroundColor Green
        } else {
            Write-Host "  [WARNING] Upload may have failed. Check manually:" -ForegroundColor Yellow
            Write-Host "    rsync -avz $stagingDir/ ${RsyncUser}@${RsyncHost}:${RsyncBaseDir}/$backupId/" -ForegroundColor Gray
        }
    } else {
        Write-Host "  [WARNING] Could not connect to rsync.net. Manual upload required:" -ForegroundColor Yellow
        Write-Host "    From server: rsync -avz $stagingDir/ ${RsyncUser}@${RsyncHost}:${RsyncBaseDir}/$backupId/" -ForegroundColor Gray
        Write-Host "    Password: $RsyncPassword" -ForegroundColor Gray
    }
} catch {
    Write-Host "  [WARNING] Upload failed: $_" -ForegroundColor Yellow
    Write-Host "  [INFO] Backup is available locally at: $stagingDir" -ForegroundColor Cyan
    Write-Host "  [INFO] Manual upload command:" -ForegroundColor Cyan
    Write-Host "    ssh $RemoteUser@$RemoteHost" -ForegroundColor Gray
    Write-Host "    rsync -avz $stagingDir/ ${RsyncUser}@${RsyncHost}:${RsyncBaseDir}/$backupId/" -ForegroundColor Gray
}

# Step 10: Create backup index
Write-Host ""
Write-Host "[10/10] Updating backup index..." -ForegroundColor Yellow
$indexEntry = "$backupId|$backupDate|$BackupLabel|$GitCommit"
$indexCmd = "ssh ${RemoteUser}@${RemoteHost} `"echo '$indexEntry' >> $RemoteDir/.backups/backup-index.txt`""
try {
    Invoke-Expression $indexCmd | Out-Null
    Write-Host "  [OK] Backup index updated" -ForegroundColor Green
} catch {
    Write-Host "  [WARNING] Index update failed: $_" -ForegroundColor Yellow
}

Write-Host ""
Write-Host "╔══════════════════════════════════════════════════════════════╗" -ForegroundColor Green
Write-Host "║  BACKUP COMPLETE                                              ║" -ForegroundColor Green
Write-Host "╚══════════════════════════════════════════════════════════════╝" -ForegroundColor Green
Write-Host ""
Write-Host "Backup ID: $backupId" -ForegroundColor Cyan
Write-Host "Local: $stagingDir" -ForegroundColor White
Write-Host "Remote: rsync://${RsyncUser}@${RsyncHost}${RsyncBaseDir}/$backupId" -ForegroundColor White
Write-Host ""
Write-Host "To list all backups:" -ForegroundColor Yellow
Write-Host "  ssh $RemoteUser@$RemoteHost 'cat $RemoteDir/.backups/backup-index.txt'" -ForegroundColor Gray
Write-Host ""
Write-Host "To restore this backup:" -ForegroundColor Yellow
Write-Host "  .\restore_from_backup.ps1 -BackupId $backupId" -ForegroundColor Gray
Write-Host ""
