#!/bin/bash
# Deep search for model files on rsync.net

HOST="zh5605.rsync.net"
USER="zh5605"
PASSWORD="3da393f1"
SSHPASS_CMD="/usr/bin/sshpass -p $PASSWORD"

echo "🔍 Deep search for model files on rsync.net..."
echo ""

# Try to find files in various ways without redirection issues
echo "1. Finding all .safetensors files:"
$SSHPASS_CMD ssh -o StrictHostKeyChecking=no "${USER}@${HOST}" "find . -name '*.safetensors' -type f"
echo ""

echo "2. Finding files with 'model' in name:"
$SSHPASS_CMD ssh -o StrictHostKeyChecking=no "${USER}@${HOST}" "find . -type f -name '*model*'"
echo ""

echo "3. Finding files with 'shard' in name:"
$SSHPASS_CMD ssh -o StrictHostKeyChecking=no "${USER}@${HOST}" "find . -type f -name '*shard*'"
echo ""

echo "4. Finding large files (>50MB):"
$SSHPASS_CMD ssh -o StrictHostKeyChecking=no "${USER}@${HOST}" "find . -type f -size +50M"
echo ""

echo "5. Checking if files might be in subdirectories we haven't checked:"
$SSHPASS_CMD ssh -o StrictHostKeyChecking=no "${USER}@${HOST}" "ls -d */ | while read dir; do echo \"Checking \$dir:\"; find \"\$dir\" -maxdepth 2 -name '*.safetensors' -o -name '*model*' -o -name '*shard*' 2>/dev/null | head -3; done"
echo ""






