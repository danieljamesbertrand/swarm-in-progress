#!/bin/bash
# Find safetensors files on rsync.net server

HOST="zh5605.rsync.net"
USER="zh5605"
PASSWORD="3da393f1"

SSHPASS_CMD="/usr/bin/sshpass -p $PASSWORD"

echo "🔍 Searching for safetensors files on $HOST..."
echo ""

# Check the suspicious directory
echo "1. Checking 'homezh5605model_shardsshards' directory:"
$SSHPASS_CMD ssh -o StrictHostKeyChecking=no "${USER}@${HOST}" "ls -lah homezh5605model_shardsshards/"
echo ""

# Search recursively for safetensors
echo "2. Searching recursively for *.safetensors files:"
$SSHPASS_CMD ssh -o StrictHostKeyChecking=no "${USER}@${HOST}" "find . -name '*.safetensors' -type f"
echo ""

# List all directories
echo "3. Listing all directories:"
$SSHPASS_CMD ssh -o StrictHostKeyChecking=no "${USER}@${HOST}" "ls -d */"
echo ""

# Check dan-pc-1 directory
echo "4. Checking 'dan-pc-1' directory:"
$SSHPASS_CMD ssh -o StrictHostKeyChecking=no "${USER}@${HOST}" "ls -lah dan-pc-1/"
echo ""
