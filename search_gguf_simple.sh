#!/bin/bash
# Simple search for .gguf files

HOST="zh5605.rsync.net"
USER="zh5605"
PASSWORD="3da393f1"
SSHPASS_CMD="/usr/bin/sshpass -p $PASSWORD"

echo "Searching for .gguf files..."
$SSHPASS_CMD ssh -o StrictHostKeyChecking=no "${USER}@${HOST}" 'find . -name "*.gguf" -type f'

echo ""
echo "Searching for files with model-related names..."
$SSHPASS_CMD ssh -o StrictHostKeyChecking=no "${USER}@${HOST}" 'find . -type f \( -name "*llama*.gguf" -o -name "*mistral*.gguf" -o -name "*model*.gguf" -o -name "*shard*.gguf" \)'

echo ""
echo "Done."





