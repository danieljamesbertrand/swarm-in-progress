#!/bin/bash
# Find .gguf files on rsync.net server

HOST="zh5605.rsync.net"
USER="zh5605"
PASSWORD="3da393f1"
SSHPASS_CMD="/usr/bin/sshpass -p $PASSWORD"

echo "🔍 Searching for .gguf files on rsync.net server..."
echo ""

# Search for .gguf files
echo "1. Finding all .gguf files recursively:"
$SSHPASS_CMD ssh -o StrictHostKeyChecking=no "${USER}@${HOST}" "find . -name '*.gguf' -type f"
echo ""

echo "2. Finding files with 'gguf' in name:"
$SSHPASS_CMD ssh -o StrictHostKeyChecking=no "${USER}@${HOST}" "find . -type f -name '*gguf*'"
echo ""

echo "3. Finding large files that might be model files (>100MB):"
$SSHPASS_CMD ssh -o StrictHostKeyChecking=no "${USER}@${HOST}" "find . -type f -size +100M -name '*.gguf'"
echo ""

echo "4. Listing files in common model directories:"
for dir in "homezh5605model_shardsshards" "dan-pc-1" "Users" "home" "tmp"; do
    echo "   Checking $dir/ for .gguf files..."
    $SSHPASS_CMD ssh -o StrictHostKeyChecking=no "${USER}@${HOST}" "find $dir -maxdepth 3 -name '*.gguf' -type f 2>/dev/null | head -10"
done
echo ""

echo "5. Checking for any files with 'llama' or 'mistral' in name that might be models:"
$SSHPASS_CMD ssh -o StrictHostKeyChecking=no "${USER}@${HOST}" "find . -type f \( -name '*llama*' -o -name '*mistral*' \) -size +50M 2>/dev/null | head -20"
echo ""






