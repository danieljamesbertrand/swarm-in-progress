# Sophisticated Backup System Documentation

## Overview

A comprehensive backup system for `eagleoneonline.ca` that:
- ✅ Creates timestamped backups after each deployment
- ✅ Tracks all changes with full metadata
- ✅ Enables recovery of specific rollouts
- ✅ Runs automatically on system boot and scheduled intervals
- ✅ Stores backups on rsync.net for long-term storage

---

## Components

### 1. Backup Script (`backup_to_rsync.ps1`)

**Purpose**: Creates comprehensive backups with full metadata tracking

**Features**:
- Timestamped backup IDs
- System information collection
- Git repository state tracking
- Source code backup
- Binary backup
- Configuration backup
- Shard metadata backup
- Upload to rsync.net
- Backup manifest generation

**Usage**:
```powershell
.\backup_to_rsync.ps1 -BackupLabel "deployment-v1.2.3" -GitCommit "abc123"
```

**Parameters**:
- `-BackupLabel`: Human-readable label for the backup
- `-GitCommit`: Git commit hash (optional, auto-detected)
- `-RemoteUser`: SSH user (default: dbertrand)
- `-RemoteHost`: Server hostname (default: eagleoneonline.ca)
- `-RemoteDir`: Project directory (default: /home/dbertrand/punch-simple)

---

### 2. Restore Script (`restore_from_backup.ps1`)

**Purpose**: Restore system from a specific backup

**Features**:
- Restore from local or remote (rsync.net) backups
- Selective restoration (source, binaries, config)
- Dry-run mode for testing
- Manifest verification

**Usage**:
```powershell
# List available backups first
.\list_backups.ps1

# Restore a specific backup
.\restore_from_backup.ps1 -BackupId "deployment-20240115-143022"

# Dry run (test without restoring)
.\restore_from_backup.ps1 -BackupId "deployment-20240115-143022" -DryRun

# Selective restore
.\restore_from_backup.ps1 -BackupId "deployment-20240115-143022" -RestoreSource -RestoreConfig
```

**Parameters**:
- `-BackupId`: Backup ID to restore (required)
- `-DryRun`: Test restore without making changes
- `-RestoreSource`: Restore source code (default: true)
- `-RestoreBinaries`: Restore compiled binaries (default: true)
- `-RestoreConfig`: Restore configuration files (default: true)

---

### 3. List Backups Script (`list_backups.ps1`)

**Purpose**: List all available backups

**Features**:
- Lists local backups on server
- Lists remote backups on rsync.net
- Shows backup metadata (date, label, commit)

**Usage**:
```powershell
.\list_backups.ps1
```

**Output**:
```
Local Backups on eagleoneonline.ca:
  Backup ID                          | Date                | Label      | Commit
  deployment-20240115-143022         | 2024-01-15 14:30:22 | deployment | abc12345
  scheduled-20240116-020000          | 2024-01-16 02:00:00 | scheduled  | def67890

Remote Backups on rsync.net:
  - deployment-20240115-143022
  - scheduled-20240116-020000
```

---

### 4. Backup Service (`backup_service.sh`)

**Purpose**: Automated backup service running on the server

**Features**:
- Runs on system boot
- Scheduled daily backups
- Automatic cleanup of old backups
- Logging to file

**Configuration**:
- `BACKUP_INTERVAL_HOURS=24`: Create backup every 24 hours
- `MAX_BACKUPS_LOCAL=10`: Keep last 10 backups locally
- `MAX_BACKUPS_REMOTE=30`: Keep last 30 backups on rsync.net

**Manual Usage**:
```bash
# Create manual backup
./backup_service.sh manual "deployment-v1.2.3"

# Create scheduled backup (checks interval)
./backup_service.sh auto

# Cleanup old backups
./backup_service.sh cleanup
```

---

### 5. Systemd Service Installation (`install_backup_service.ps1`)

**Purpose**: Install backup service as systemd service and timer

**Features**:
- Creates systemd service file
- Creates systemd timer for scheduled backups
- Enables service on boot
- Starts timer automatically

**Usage**:
```powershell
.\install_backup_service.ps1
```

**What it does**:
1. Uploads `backup_service.sh` to server
2. Makes script executable
3. Creates `/etc/systemd/system/punch-backup.service`
4. Creates `/etc/systemd/system/punch-backup.timer`
5. Enables and starts the timer

**Service Schedule**:
- First backup: 1 hour after system boot
- Subsequent backups: Every 24 hours

---

## Integration with Deployment

### Automatic Backup After Deployment

The `deploy_server_to_eagleoneonline.ps1` script now automatically creates a backup after each deployment:

```powershell
# Deployment automatically triggers backup
.\deploy_server_to_eagleoneonline.ps1
```

**What happens**:
1. Server code is deployed
2. Server is rebuilt
3. Server is restarted
4. **Backup is automatically created** with:
   - Label: `deployment-YYYYMMDD-HHMMSS`
   - Git commit hash
   - Full system state

---

## Backup Structure

### Local Backup Directory

```
/home/dbertrand/punch-simple/.backups/
├── backup-index.txt                    # Index of all backups
├── backup-service.log                  # Service logs
├── deployment-20240115-143022/         # Individual backup
│   ├── MANIFEST.txt                   # Backup manifest
│   ├── system-info.txt                # System information
│   ├── git-info.txt                   # Git repository state
│   ├── source/                         # Source code backup
│   ├── binaries/                       # Compiled binaries
│   ├── config/                         # Configuration files
│   └── shard-metadata/                 # Shard file metadata
└── scheduled-20240116-020000/         # Scheduled backup
    └── ...
```

### Remote Backup (rsync.net)

```
/home/zh5605/backups/eagleoneonline/
├── deployment-20240115-143022/         # Same structure as local
├── scheduled-20240116-020000/
└── ...
```

---

## Backup Manifest

Each backup includes a `MANIFEST.txt` file with:

```text
# Backup Manifest
# Generated: 2024-01-15 14:30:22 UTC

## Backup Information
Backup ID: deployment-20240115-143022
Backup Label: deployment
Backup Date: 2024-01-15 14:30:22 UTC
Server: eagleoneonline.ca
Remote Directory: /home/dbertrand/punch-simple

## Deployment Information
Git Commit: abc1234567890def
Deployment Label: deployment

## Backup Contents
- Source Code: /home/dbertrand/punch-simple/.backups/deployment-20240115-143022/source
- Binaries: /home/dbertrand/punch-simple/.backups/deployment-20240115-143022/binaries
- Configuration: /home/dbertrand/punch-simple/.backups/deployment-20240115-143022/config
- Shard Metadata: /home/dbertrand/punch-simple/.backups/deployment-20240115-143022/shard-metadata

## System Information
[System details, uptime, disk usage, etc.]

## Git Information
[Commit hash, branch, last commit details, etc.]

## Recovery Instructions
[How to restore this backup]
```

---

## Recovery Workflow

### Step 1: List Available Backups

```powershell
.\list_backups.ps1
```

### Step 2: Choose Backup to Restore

Identify the backup ID from the list, e.g., `deployment-20240115-143022`

### Step 3: Review Backup Details

```bash
ssh dbertrand@eagleoneonline.ca 'cat /home/dbertrand/punch-simple/.backups/deployment-20240115-143022/MANIFEST.txt'
```

### Step 4: Restore Backup

```powershell
# Dry run first (recommended)
.\restore_from_backup.ps1 -BackupId "deployment-20240115-143022" -DryRun

# Actual restore
.\restore_from_backup.ps1 -BackupId "deployment-20240115-143022"
```

### Step 5: Rebuild and Restart (if needed)

```bash
ssh dbertrand@eagleoneonline.ca
cd ~/punch-simple
cargo build --release --bin server
sudo systemctl restart punch-rendezvous
```

---

## rsync.net Configuration

### Credentials

- **Host**: `zh5605.rsync.net`
- **Username**: `zh5605`
- **Password**: `3da393f1`
- **Base Directory**: `/home/zh5605/backups/eagleoneonline`

### SSH Key Setup (Recommended)

For passwordless authentication:

```bash
# Generate SSH key (if needed)
ssh-keygen -t ed25519 -C "backup@eagleoneonline"

# Copy to rsync.net
ssh-copy-id zh5605@zh5605.rsync.net

# Test connection
ssh zh5605@zh5605.rsync.net 'echo "Connection successful"'
```

---

## Monitoring and Maintenance

### Check Backup Service Status

```bash
ssh dbertrand@eagleoneonline.ca 'sudo systemctl status punch-backup.timer'
```

### View Backup Logs

```bash
ssh dbertrand@eagleoneonline.ca 'tail -f /home/dbertrand/punch-simple/.backups/backup-service.log'
```

### Manual Backup Trigger

```bash
ssh dbertrand@eagleoneonline.ca '/home/dbertrand/punch-simple/backup_service.sh manual "emergency-backup"'
```

### Cleanup Old Backups

The service automatically cleans up old local backups (keeps last 10). For manual cleanup:

```bash
ssh dbertrand@eagleoneonline.ca '/home/dbertrand/punch-simple/backup_service.sh cleanup'
```

---

## Best Practices

### 1. Always Backup Before Major Changes

```powershell
# Create backup before deployment
.\backup_to_rsync.ps1 -BackupLabel "pre-deployment-v1.3.0"
```

### 2. Use Descriptive Labels

- ✅ Good: `deployment-v1.2.3`, `hotfix-auth-bug`, `feature-new-inference`
- ❌ Bad: `backup1`, `test`, `deployment`

### 3. Verify Backups Regularly

```powershell
# List backups monthly
.\list_backups.ps1

# Test restore in dry-run mode
.\restore_from_backup.ps1 -BackupId "deployment-20240115-143022" -DryRun
```

### 4. Keep Backup Index Updated

The backup index (`backup-index.txt`) is automatically maintained, but verify it:

```bash
ssh dbertrand@eagleoneonline.ca 'cat /home/dbertrand/punch-simple/.backups/backup-index.txt'
```

---

## Troubleshooting

### Backup Upload Fails

**Problem**: Backup created locally but upload to rsync.net fails

**Solution**:
1. Check SSH connection: `ssh zh5605@zh5605.rsync.net`
2. Verify credentials
3. Check disk space on rsync.net
4. Manual upload: `rsync -avz /path/to/backup/ zh5605@zh5605.rsync.net:/home/zh5605/backups/eagleoneonline/backup-id/`

### Service Not Running

**Problem**: Backup service not creating scheduled backups

**Solution**:
```bash
# Check service status
sudo systemctl status punch-backup.timer

# Restart service
sudo systemctl restart punch-backup.timer

# Check logs
tail -f /home/dbertrand/punch-simple/.backups/backup-service.log
```

### Restore Fails

**Problem**: Restore script fails to restore files

**Solution**:
1. Verify backup exists: `ls -la /home/dbertrand/punch-simple/.backups/backup-id/`
2. Check permissions: `ls -la /home/dbertrand/punch-simple/.backups/backup-id/`
3. Try manual restore: `rsync -av backup-id/source/ /home/dbertrand/punch-simple/`

---

## Summary

✅ **Automatic backups** after each deployment
✅ **Scheduled backups** daily via systemd timer
✅ **Full metadata tracking** for each backup
✅ **Easy recovery** of specific rollouts
✅ **Long-term storage** on rsync.net
✅ **Automatic cleanup** of old backups
✅ **Comprehensive logging** and monitoring

**The backup system is now fully integrated and ready to track all deployments!** 🎉
