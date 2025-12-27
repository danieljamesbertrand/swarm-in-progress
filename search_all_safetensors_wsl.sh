#!/bin/bash
# Comprehensive search for safetensors files on rsync.net

HOST="zh5605.rsync.net"
USER="zh5605"
PASSWORD="3da393f1"
SSHPASS_CMD="/usr/bin/sshpass -p $PASSWORD"

echo "🔍 Comprehensive search for safetensors files..."
echo ""

# Search in multiple ways
echo "1. Recursive find (all directories):"
$SSHPASS_CMD ssh -o StrictHostKeyChecking=no "${USER}@${HOST}" "find . -name '*.safetensors' -type f 2>/dev/null"
echo ""

echo "2. Search in common directories:"
for dir in "dan-pc-1" "homezh5605model_shardsshards" "Users" "home" "tmp" "var"; do
    echo "   Checking $dir/..."
    $SSHPASS_CMD ssh -o StrictHostKeyChecking=no "${USER}@${HOST}" "find $dir -name '*.safetensors' -type f 2>/dev/null" | head -5
done
echo ""

echo "3. Search for any files with 'model' or 'shard' in name:"
$SSHPASS_CMD ssh -o StrictHostKeyChecking=no "${USER}@${HOST}" "find . -type f \( -name '*model*' -o -name '*shard*' \) 2>/dev/null | head -20"
echo ""

echo "4. Check for large files (>100MB) that might be model files:"
$SSHPASS_CMD ssh -o StrictHostKeyChecking=no "${USER}@${HOST}" "find . -type f -size +100M 2>/dev/null | head -20"
echo ""





