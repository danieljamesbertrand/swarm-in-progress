#!/bin/bash
# Automated Backup Service for eagleoneonline.ca
# Runs on system boot and creates scheduled backups
# This script is installed as a systemd service

set -e

# Configuration
REMOTE_USER="dbertrand"
REMOTE_HOST="eagleoneonline.ca"
REMOTE_DIR="/home/dbertrand/punch-simple"
RSYNC_HOST="zh5605.rsync.net"
RSYNC_USER="zh5605"
RSYNC_BASE_DIR="/home/zh5605/backups/eagleoneonline"

# Backup configuration
BACKUP_INTERVAL_HOURS=24  # Create backup every 24 hours
MAX_BACKUPS_LOCAL=10      # Keep last 10 backups locally
MAX_BACKUPS_REMOTE=30     # Keep last 30 backups on rsync.net

# Logging
LOG_FILE="$REMOTE_DIR/.backups/backup-service.log"
mkdir -p "$(dirname "$LOG_FILE")"

log() {
    echo "[$(date '+%Y-%m-%d %H:%M:%S')] $1" | tee -a "$LOG_FILE"
}

# Create backup
create_backup() {
    local backup_label="$1"
    local git_commit="$2"
    local timestamp=$(date +"%Y%m%d-%H%M%S")
    local backup_id="${backup_label}-${timestamp}"
    local staging_dir="$REMOTE_DIR/.backups/$backup_id"
    
    log "Creating backup: $backup_id"
    
    # Create staging directory
    mkdir -p "$staging_dir"
    
    # Collect system information
    {
        echo "# System Information"
        echo "Backup ID: $backup_id"
        echo "Backup Date: $(date -u '+%Y-%m-%d %H:%M:%S UTC')"
        echo "Server: $REMOTE_HOST"
        echo "Backup Label: $backup_label"
        echo "Git Commit: $git_commit"
        echo ""
        echo "# Server Information"
        uname -a
        hostname
        uptime
        df -h "$REMOTE_DIR"
    } > "$staging_dir/system-info.txt"
    
    # Collect Git information
    if [ -d "$REMOTE_DIR/.git" ]; then
        {
            echo "# Git Repository Information"
            echo "Commit: $(cd "$REMOTE_DIR" && git rev-parse HEAD 2>/dev/null || echo 'N/A')"
            echo "Short Commit: $(cd "$REMOTE_DIR" && git rev-parse --short HEAD 2>/dev/null || echo 'N/A')"
            echo "Branch: $(cd "$REMOTE_DIR" && git branch --show-current 2>/dev/null || echo 'N/A')"
            echo "Last Commit:"
            cd "$REMOTE_DIR" && git log -1 --pretty=format:"%H|%an|%ae|%ad|%s" --date=iso 2>/dev/null || echo 'N/A'
        } > "$staging_dir/git-info.txt"
    fi
    
    # Backup source code
    log "Backing up source code..."
    mkdir -p "$staging_dir/source"
    rsync -av --exclude='target/' --exclude='.git/' --exclude='.backups/' \
        "$REMOTE_DIR/" "$staging_dir/source/" > /dev/null 2>&1
    
    # Backup binaries
    if [ -d "$REMOTE_DIR/target/release" ]; then
        log "Backing up binaries..."
        mkdir -p "$staging_dir/binaries"
        cp -r "$REMOTE_DIR/target/release"/* "$staging_dir/binaries/" 2>/dev/null || true
    fi
    
    # Backup configuration
    log "Backing up configuration..."
    mkdir -p "$staging_dir/config"
    [ -f "/etc/systemd/system/punch-rendezvous.service" ] && \
        cp "/etc/systemd/system/punch-rendezvous.service" "$staging_dir/config/" || true
    [ -f "$REMOTE_DIR/Cargo.toml" ] && \
        cp "$REMOTE_DIR/Cargo.toml" "$staging_dir/config/" || true
    
    # Create manifest
    {
        echo "# Backup Manifest"
        echo "# Generated: $(date -u '+%Y-%m-%d %H:%M:%S UTC')"
        echo ""
        echo "Backup ID: $backup_id"
        echo "Backup Label: $backup_label"
        echo "Backup Date: $(date -u '+%Y-%m-%d %H:%M:%S UTC')"
        echo "Server: $REMOTE_HOST"
        echo "Git Commit: $git_commit"
        echo ""
        echo "Backup Contents:"
        echo "- Source Code: $staging_dir/source"
        echo "- Binaries: $staging_dir/binaries"
        echo "- Configuration: $staging_dir/config"
        echo ""
        echo "Remote Storage: rsync://${RSYNC_USER}@${RSYNC_HOST}${RSYNC_BASE_DIR}/$backup_id"
    } > "$staging_dir/MANIFEST.txt"
    
    # Upload to rsync.net
    log "Uploading to rsync.net..."
    rsync -avz --progress "$staging_dir/" \
        "${RSYNC_USER}@${RSYNC_HOST}:${RSYNC_BASE_DIR}/$backup_id/" 2>&1 | tee -a "$LOG_FILE" || {
        log "WARNING: Upload to rsync.net failed, backup available locally at $staging_dir"
    }
    
    # Update backup index
    echo "$backup_id|$(date -u '+%Y-%m-%d %H:%M:%S UTC')|$backup_label|$git_commit" >> \
        "$REMOTE_DIR/.backups/backup-index.txt"
    
    log "Backup complete: $backup_id"
    
    # Cleanup old local backups
    cleanup_old_backups "$MAX_BACKUPS_LOCAL" "local"
    
    return 0
}

# Cleanup old backups
cleanup_old_backups() {
    local max_backups=$1
    local location=$2
    
    if [ "$location" = "local" ]; then
        log "Cleaning up old local backups (keeping last $max_backups)..."
        cd "$REMOTE_DIR/.backups"
        ls -td backup-* deployment-* 2>/dev/null | tail -n +$((max_backups + 1)) | \
            xargs -r rm -rf
    fi
}

# Main function
main() {
    local action="${1:-auto}"
    
    case "$action" in
        "auto")
            # Check if backup is needed (last backup older than interval)
            local last_backup=$(tail -n 1 "$REMOTE_DIR/.backups/backup-index.txt" 2>/dev/null | cut -d'|' -f2 || echo "")
            if [ -n "$last_backup" ]; then
                local last_backup_epoch=$(date -d "$last_backup" +%s 2>/dev/null || echo 0)
                local now_epoch=$(date +%s)
                local hours_since=$(( (now_epoch - last_backup_epoch) / 3600 ))
                
                if [ $hours_since -lt $BACKUP_INTERVAL_HOURS ]; then
                    log "Last backup was $hours_since hours ago, skipping (interval: $BACKUP_INTERVAL_HOURS hours)"
                    exit 0
                fi
            fi
            
            # Create scheduled backup
            local git_commit=""
            [ -d "$REMOTE_DIR/.git" ] && \
                git_commit=$(cd "$REMOTE_DIR" && git rev-parse HEAD 2>/dev/null || echo "")
            create_backup "scheduled" "$git_commit"
            ;;
        "manual")
            # Create manual backup
            local label="${2:-manual}"
            local git_commit=""
            [ -d "$REMOTE_DIR/.git" ] && \
                git_commit=$(cd "$REMOTE_DIR" && git rev-parse HEAD 2>/dev/null || echo "")
            create_backup "$label" "$git_commit"
            ;;
        "cleanup")
            cleanup_old_backups "$MAX_BACKUPS_LOCAL" "local"
            ;;
        *)
            echo "Usage: $0 {auto|manual [label]|cleanup}"
            exit 1
            ;;
    esac
}

main "$@"
