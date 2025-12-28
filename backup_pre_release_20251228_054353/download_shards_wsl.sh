#!/bin/bash
# WSL script to download safetensors files from rsync.net
# Usage: wsl bash download_shards_wsl.sh

set -e

HOST="zh5605.rsync.net"
USER="zh5605"
PASSWORD="3da393f1"
LOCAL_CACHE="models_cache"

echo ""
echo "╔══════════════════════════════════════════════════════════════╗"
echo "║         📥 SHARD DOWNLOADER (WSL) 📥                        ║"
echo "╚══════════════════════════════════════════════════════════════╝"
echo ""

# Create cache directory
mkdir -p "$LOCAL_CACHE"

# Check if sshpass is available
SSHPASS_CMD=""
if [ -f "/usr/bin/sshpass" ]; then
    SSHPASS_CMD="/usr/bin/sshpass -p $PASSWORD"
    echo "✅ Using sshpass at /usr/bin/sshpass"
elif command -v sshpass &> /dev/null; then
    SSHPASS_CMD="sshpass -p $PASSWORD"
    echo "✅ Using sshpass from PATH"
else
    echo "⚠️  sshpass not found, will prompt for password"
    SSHPASS_CMD=""
fi
echo ""

echo ""
echo "🔍 Listing safetensors files on rsync.net server..."
echo ""

# Function to run SSH command with or without sshpass
run_ssh() {
    local cmd="$1"
    if [ -n "$SSHPASS_CMD" ]; then
        $SSHPASS_CMD ssh -o StrictHostKeyChecking=no -o ConnectTimeout=10 "${USER}@${HOST}" "$cmd" 2>/dev/null
    else
        ssh -o StrictHostKeyChecking=no -o ConnectTimeout=10 "${USER}@${HOST}" "$cmd" 2>/dev/null
    fi
}

# Function to run SCP command with or without sshpass
run_scp() {
    local src="$1"
    local dest="$2"
    if [ -n "$SSHPASS_CMD" ]; then
        $SSHPASS_CMD scp -o StrictHostKeyChecking=no -o ConnectTimeout=30 "$src" "$dest" 2>&1
    else
        scp -o StrictHostKeyChecking=no -o ConnectTimeout=30 "$src" "$dest" 2>&1
    fi
}

# Try to list files first (both safetensors and gguf)
LIST_CMD="find . -name '*.safetensors' -o -name '*.gguf' | head -50"

FILES=$(run_ssh "$LIST_CMD" || echo "")

if [ -z "$FILES" ]; then
    echo "⚠️  Could not list files, trying common patterns..."
    FILES="model-00001-of-00004.safetensors
model-00002-of-00004.safetensors
model-00003-of-00004.safetensors
model-00004-of-00004.safetensors
model-00001-of-00003.safetensors
model-00002-of-00003.safetensors
model-00003-of-00003.safetensors
shard-0.safetensors
shard-1.safetensors
shard-2.safetensors
shard-3.safetensors
model.gguf
llama-7b-q4_k_m.gguf
mistral-7b-instruct-v0.2.Q4_K_M.gguf"
fi

# Count files (filter out empty lines)
FILE_COUNT=$(echo "$FILES" | grep -v '^$' | wc -l)
echo "✅ Found $FILE_COUNT file(s) to download"
echo ""

if [ -z "$SSHPASS_CMD" ]; then
    echo "💡 You will be prompted for password. Enter: $PASSWORD"
    echo ""
fi

# Download each file
COUNTER=0
for filename in $FILES; do
    # Skip empty lines
    [ -z "$filename" ] && continue
    
    COUNTER=$((COUNTER + 1))
    DEST_PATH="$LOCAL_CACHE/$filename"
    
    # Skip if already exists
    if [ -f "$DEST_PATH" ]; then
        SIZE_MB=$(du -m "$DEST_PATH" 2>/dev/null | cut -f1 || echo "0")
        echo "⏭️  Skipping $filename (already exists, ${SIZE_MB} MB)"
        continue
    fi
    
    # Filter to only download .safetensors or .gguf files
    if [[ ! "$filename" =~ \.(safetensors|gguf)$ ]]; then
        echo "⏭️  Skipping $filename (not a model file)"
        continue
    fi
    
    echo "📥 Downloading $COUNTER/$FILE_COUNT..."
    echo "   File: $filename"
    
    # Use scp to download
    SCP_SRC="${USER}@${HOST}:${filename}"
    if run_scp "$SCP_SRC" "$DEST_PATH"; then
        if [ -f "$DEST_PATH" ]; then
            SIZE_MB=$(du -m "$DEST_PATH" 2>/dev/null | cut -f1 || echo "0")
            echo "   ✅ Complete! (${SIZE_MB} MB)"
        else
            echo "   ⚠️  File not found on server"
        fi
    else
        echo "   ⚠️  Failed to download $filename"
    fi
    echo ""
done

# Show what we downloaded
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "📁 Contents of $LOCAL_CACHE:"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

if [ -d "$LOCAL_CACHE" ]; then
for file in "$LOCAL_CACHE"/*.{safetensors,gguf} 2>/dev/null; do
    if [ -f "$file" ]; then
        SIZE_MB=$(du -m "$file" 2>/dev/null | cut -f1 || echo "0")
        FILENAME=$(basename "$file")
        printf "   %-40s %10s MB\n" "$FILENAME" "$SIZE_MB"
    fi
done
fi

echo ""
echo "✅ Download complete!"
echo ""

# Split .gguf files into 8 shards on proper tensor boundaries
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "🔪 Splitting .gguf files into 8 shards (proper tensor boundaries)"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""

SHARD_OUTPUT_DIR="$LOCAL_CACHE/shards"
mkdir -p "$SHARD_OUTPUT_DIR"

# Find all .gguf files in cache
GGUF_FILES=$(find "$LOCAL_CACHE" -maxdepth 1 -name "*.gguf" -type f 2>/dev/null)

if [ -z "$GGUF_FILES" ]; then
    echo "⏭️  No .gguf files found to split"
else
    # Check if Python 3 is available
    if ! command -v python3 &> /dev/null; then
        echo "⚠️  python3 not found. Cannot split .gguf files."
        echo "   Install Python 3 to enable proper tensor boundary splitting."
    else
        # Check for gguf-split (llama.cpp tool - preferred), then Rust binary, then Python script
        SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
        GGUF_SPLIT_CMD=""
        
        # Check for llama-gguf-split in PATH or llama.cpp directory
        if command -v llama-gguf-split &> /dev/null; then
            GGUF_SPLIT_CMD="llama-gguf-split"
        elif command -v gguf-split &> /dev/null; then
            GGUF_SPLIT_CMD="gguf-split"
        elif [ -f "$SCRIPT_DIR/llama.cpp/build/bin/llama-gguf-split" ]; then
            GGUF_SPLIT_CMD="$SCRIPT_DIR/llama.cpp/build/bin/llama-gguf-split"
        elif [ -f "$SCRIPT_DIR/llama.cpp/build/bin/gguf-split" ]; then
            GGUF_SPLIT_CMD="$SCRIPT_DIR/llama.cpp/build/bin/gguf-split"
        elif [ -f "$SCRIPT_DIR/llama.cpp/gguf-split" ]; then
            GGUF_SPLIT_CMD="$SCRIPT_DIR/llama.cpp/gguf-split"
        fi
        
        RUST_BINARY="$SCRIPT_DIR/target/debug/examples/split_gguf_rust"
        PYTHON_SCRIPT="$SCRIPT_DIR/split_gguf_proper.py"
        
        if [ -n "$GGUF_SPLIT_CMD" ]; then
            SPLIT_CMD="$GGUF_SPLIT_CMD"
            USE_GGUF_SPLIT=1
            echo "✅ Using gguf-split from llama.cpp (official tool)"
        elif [ -f "$RUST_BINARY" ]; then
            SPLIT_CMD="$RUST_BINARY"
            USE_GGUF_SPLIT=0
            USE_RUST=1
            echo "✅ Using Rust splitter"
        elif [ -f "$PYTHON_SCRIPT" ]; then
            SPLIT_CMD="python3 $PYTHON_SCRIPT"
            USE_GGUF_SPLIT=0
            USE_RUST=0
            echo "✅ Using Python splitter"
        else
            echo "⚠️  No splitter found!"
            echo ""
            echo "   To install gguf-split (recommended):"
            echo "   1. Install cmake: sudo apt-get install cmake build-essential"
            echo "   2. cd llama.cpp && mkdir -p build && cd build"
            echo "   3. cmake .. -DGGUF_SPLIT=ON && make gguf-split"
            echo ""
            echo "   Or build the Rust version: cargo build --example split_gguf_rust"
        fi
        
        if [ -n "$SPLIT_CMD" ]; then
            NUM_SHARDS=8
            GGUF_COUNT=$(echo "$GGUF_FILES" | grep -v '^$' | wc -l)
            echo "✅ Found $GGUF_COUNT .gguf file(s) to split into $NUM_SHARDS shards"
            echo ""
            
            COUNTER=0
            for gguf_file in $GGUF_FILES; do
                COUNTER=$((COUNTER + 1))
                FILENAME=$(basename "$gguf_file")
                
                # Check if shards already exist (look for shard-0 through shard-7)
                SHARDS_EXIST=0
                for i in $(seq 0 $((NUM_SHARDS - 1))); do
                    if [ -f "$SHARD_OUTPUT_DIR/shard-$i.gguf" ]; then
                        SHARDS_EXIST=1
                        break
                    fi
                done
                
                if [ "$SHARDS_EXIST" -eq 1 ]; then
                    echo "⏭️  Skipping $FILENAME (shards already exist in $SHARD_OUTPUT_DIR)"
                    echo "   Delete existing shards to re-split"
                    continue
                fi
                
                echo "🔪 Splitting $COUNTER/$GGUF_COUNT: $FILENAME"
                echo "   Creating $NUM_SHARDS shards with proper tensor boundaries..."
                
                # Use splitter (llama-gguf-split, Rust, or Python)
                if [ "$USE_GGUF_SPLIT" -eq 1 ]; then
                    # Calculate tensors per shard to get approximately NUM_SHARDS shards
                    # Most models have 200-400 tensors, so for 8 shards: ~25-50 tensors per shard
                    # Using 36-37 tensors per shard should give us ~8 shards for a 291-tensor model
                    TENSORS_PER_SHARD=37
                    BASE_NAME=$(basename "$gguf_file" .gguf)
                    if $SPLIT_CMD --split --split-max-tensors $TENSORS_PER_SHARD "$gguf_file" "$SHARD_OUTPUT_DIR/${BASE_NAME}_shard" 2>&1; then
                        SUCCESS=1
                        # Rename first NUM_SHARDS files to shard-0.gguf, shard-1.gguf, etc.
                        SHARD_NUM=0
                        for shard_file in "$SHARD_OUTPUT_DIR"/${BASE_NAME}_shard-*.gguf; do
                            if [ -f "$shard_file" ] && [ $SHARD_NUM -lt $NUM_SHARDS ]; then
                                mv "$shard_file" "$SHARD_OUTPUT_DIR/shard-$SHARD_NUM.gguf"
                                SHARD_NUM=$((SHARD_NUM + 1))
                            fi
                        done
                    else
                        SUCCESS=0
                    fi
                elif [ "$USE_RUST" -eq 1 ]; then
                    if $SPLIT_CMD "$gguf_file" "$NUM_SHARDS" "$SHARD_OUTPUT_DIR" 2>&1; then
                        SUCCESS=1
                    else
                        SUCCESS=0
                    fi
                else
                    if $SPLIT_CMD "$gguf_file" "$NUM_SHARDS" "$SHARD_OUTPUT_DIR" 2>&1; then
                        SUCCESS=1
                    else
                        SUCCESS=0
                    fi
                fi
                
                if [ "$SUCCESS" -eq 1 ]; then
                    echo "   ✅ Split complete!"
                    
                    # Show created shards
                    SHARD_COUNT=0
                    for shard in "$SHARD_OUTPUT_DIR"/shard-*.gguf; do
                        if [ -f "$shard" ]; then
                            SHARD_COUNT=$((SHARD_COUNT + 1))
                        fi
                    done
                    echo "   Created $SHARD_COUNT shard(s)"
                else
                    echo "   ⚠️  Failed to split $FILENAME"
                fi
                echo ""
            done
            
            # Show all created shards
            if [ -d "$SHARD_OUTPUT_DIR" ]; then
                SHARD_FILES=$(find "$SHARD_OUTPUT_DIR" -name "shard-*.gguf" -type f 2>/dev/null | wc -l)
                if [ "$SHARD_FILES" -gt 0 ]; then
                    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
                    echo "📁 Created shards in $SHARD_OUTPUT_DIR:"
                    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
                    for shard in "$SHARD_OUTPUT_DIR"/shard-*.gguf; do
                        if [ -f "$shard" ]; then
                            SIZE_MB=$(du -m "$shard" 2>/dev/null | cut -f1 || echo "0")
                            FILENAME=$(basename "$shard")
                            printf "   %-40s %10s MB\n" "$FILENAME" "$SIZE_MB"
                        fi
                    done
                    echo ""
                fi
            fi
        fi
    fi
fi

echo ""
