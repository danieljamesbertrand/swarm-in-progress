#!/bin/bash
# WSL script to list safetensors files on rsync.net
# Usage: wsl bash list_rsync_wsl.sh

HOST="zh5605.rsync.net"
USER="zh5605"
PASSWORD="3da393f1"

SSHPASS_CMD=""
if [ -f "/usr/bin/sshpass" ]; then
    SSHPASS_CMD="/usr/bin/sshpass -p $PASSWORD"
elif command -v sshpass &> /dev/null; then
    SSHPASS_CMD="sshpass -p $PASSWORD"
fi

echo ""
echo "🔍 Listing safetensors files on $HOST..."
echo ""

# Try multiple commands to list files
COMMANDS=(
    "find . -name '*.safetensors' -type f"
    "gfind . -name '*.safetensors' -type f -ls"
    "ls -lh *.safetensors"
    "ls -lh | grep safetensors"
    "ls -la | grep safetensors"
)

for cmd in "${COMMANDS[@]}"; do
    echo "Trying: $cmd"
    if [ -n "$SSHPASS_CMD" ]; then
        RESULT=$($SSHPASS_CMD ssh -o StrictHostKeyChecking=no -o ConnectTimeout=10 "${USER}@${HOST}" "$cmd" 2>/dev/null)
    else
        RESULT=$(ssh -o StrictHostKeyChecking=no -o ConnectTimeout=10 "${USER}@${HOST}" "$cmd" 2>/dev/null)
    fi
    
    if [ -n "$RESULT" ]; then
        echo "✅ Found files:"
        echo "$RESULT"
        echo ""
        break
    fi
done

# Also try listing all files to see what's there
echo "Listing all files in current directory:"
if [ -n "$SSHPASS_CMD" ]; then
    $SSHPASS_CMD ssh -o StrictHostKeyChecking=no "${USER}@${HOST}" "ls -lah" 2>/dev/null | head -20
else
    ssh -o StrictHostKeyChecking=no "${USER}@${HOST}" "ls -lah" 2>/dev/null | head -20
fi

echo ""






