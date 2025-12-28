#!/bin/bash
# Helper script to build gguf-split and split .gguf files

set -e

cd "$(dirname "$0")"

echo "Building gguf-split from llama.cpp..."
echo ""

# Check if cmake is installed
if ! command -v cmake &> /dev/null; then
    echo "⚠️  CMake is not installed."
    echo "   Installing cmake and build-essential..."
    sudo apt-get update -qq
    sudo apt-get install -y cmake build-essential
fi

# Build gguf-split
cd llama.cpp
mkdir -p build
cd build

echo "Configuring CMake..."
cmake .. -DGGUF_SPLIT=ON

echo "Building gguf-split..."
make gguf-split -j$(nproc)

if [ -f bin/gguf-split ]; then
    echo ""
    echo "✅ gguf-split built successfully!"
    echo "   Location: $(pwd)/bin/gguf-split"
    echo ""
    
    # Now split the GGUF file
    GGUF_FILE="../models_cache/mistral-7b-instruct-v0.2.Q4_K_M.gguf"
    if [ -f "$GGUF_FILE" ]; then
        echo "Splitting $GGUF_FILE into 8 shards..."
        mkdir -p ../models_cache/shards
        
        # Calculate tensors per shard (aim for ~8 shards)
        # Most models have 200-400 tensors, so ~25-50 per shard should give us ~8 shards
        ./bin/gguf-split --split --split-max-tensors 32 "$GGUF_FILE" "../models_cache/shards/shard"
        
        # Rename to shard-0.gguf, shard-1.gguf, etc.
        SHARD_NUM=0
        for shard_file in ../models_cache/shards/shard-*.gguf; do
            if [ -f "$shard_file" ]; then
                mv "$shard_file" "../models_cache/shards/shard-$SHARD_NUM.gguf"
                SIZE_MB=$(du -m "../models_cache/shards/shard-$SHARD_NUM.gguf" 2>/dev/null | cut -f1 || echo "0")
                echo "  Created shard-$SHARD_NUM.gguf (${SIZE_MB} MB)"
                SHARD_NUM=$((SHARD_NUM + 1))
            fi
        done
        
        echo ""
        echo "✅ Split complete! Created $SHARD_NUM shard(s) in models_cache/shards/"
    else
        echo "⚠️  GGUF file not found: $GGUF_FILE"
    fi
else
    echo "❌ Failed to build gguf-split"
    exit 1
fi






