#!/bin/bash
# WSL/Bash script to split a .gguf file into 8 shards for distributed inference
# Usage: wsl bash split_gguf_shards.sh [gguf_file] [num_shards]
# Example: wsl bash split_gguf_shards.sh models_cache/model.gguf 8

set -e

LOCAL_CACHE="models_cache"
SHARD_OUTPUT_DIR="$LOCAL_CACHE/shards"
NUM_SHARDS="${2:-8}"
GGUF_FILE="$1"

echo ""
echo "GGUF FILE SHARD SPLITTER"
echo ""

# Find .gguf file if not specified
if [ -z "$GGUF_FILE" ]; then
    GGUF_FILES=$(find "$LOCAL_CACHE" -name "*.gguf" -type f 2>/dev/null)
    
    if [ -z "$GGUF_FILES" ]; then
        echo "No .gguf files found in $LOCAL_CACHE"
        echo "Please download a .gguf file first or specify the path"
        exit 1
    fi
    
    # Count files
    FILE_COUNT=$(echo "$GGUF_FILES" | wc -l)
    
    if [ "$FILE_COUNT" -eq 1 ]; then
        GGUF_FILE=$(echo "$GGUF_FILES" | head -1)
        echo "Using: $(basename "$GGUF_FILE")"
    else
        echo "Multiple .gguf files found. Please specify which one:"
        INDEX=1
        echo "$GGUF_FILES" | while read -r file; do
            SIZE_GB=$(du -h "$file" | cut -f1)
            echo "  $INDEX. $(basename "$file") ($SIZE_GB)"
            INDEX=$((INDEX + 1))
        done
        echo "Usage: $0 <file_path> [num_shards]"
        exit 1
    fi
fi

if [ ! -f "$GGUF_FILE" ]; then
    echo "File not found: $GGUF_FILE"
    exit 1
fi

FILE_SIZE=$(stat -f%z "$GGUF_FILE" 2>/dev/null || stat -c%s "$GGUF_FILE" 2>/dev/null)
SHARD_SIZE=$((FILE_SIZE / NUM_SHARDS + 1))

FILE_SIZE_GB=$(echo "scale=2; $FILE_SIZE / 1073741824" | bc)
SHARD_SIZE_GB=$(echo "scale=2; $SHARD_SIZE / 1073741824" | bc)

echo "Input file: $(basename "$GGUF_FILE")"
echo "File size: $FILE_SIZE_GB GB"
echo "Number of shards: $NUM_SHARDS"
echo "Shard size: ~$SHARD_SIZE_GB GB each"
echo ""

# Create shards directory
mkdir -p "$SHARD_OUTPUT_DIR"

echo "Splitting file into shards..."
echo ""

# Use split command (available on Linux/WSL) or dd
if command -v split &> /dev/null; then
    # Use split command - faster for large files
    echo "Using 'split' command for efficient splitting..."
    cd "$SHARD_OUTPUT_DIR"
    split -b "$SHARD_SIZE" -d -a 1 "$GGUF_FILE" "shard-" --additional-suffix=".gguf"
    
    # Rename files to have proper numbering
    COUNTER=0
    for file in shard-*.gguf; do
        if [ -f "$file" ]; then
            mv "$file" "shard-$COUNTER.gguf"
            COUNTER=$((COUNTER + 1))
        fi
    done
    cd - > /dev/null
    
elif command -v dd &> /dev/null; then
    # Use dd as fallback
    echo "Using 'dd' command for splitting..."
    INPUT_STREAM=$(cat "$GGUF_FILE")
    SHARD_NUM=0
    OFFSET=0
    
    while [ $SHARD_NUM -lt $NUM_SHARDS ]; do
        SHARD_PATH="$SHARD_OUTPUT_DIR/shard-$SHARD_NUM.gguf"
        echo "Creating shard $((SHARD_NUM + 1))/$NUM_SHARDS: $(basename "$SHARD_PATH")"
        
        dd if="$GGUF_FILE" of="$SHARD_PATH" bs=1048576 skip=$((OFFSET / 1048576)) count=$((SHARD_SIZE / 1048576)) 2>/dev/null
        
        if [ -f "$SHARD_PATH" ]; then
            SIZE_MB=$(du -m "$SHARD_PATH" 2>/dev/null | cut -f1 || echo "0")
            echo "  Complete! (${SIZE_MB} MB)"
        fi
        echo ""
        
        OFFSET=$((OFFSET + SHARD_SIZE))
        SHARD_NUM=$((SHARD_NUM + 1))
        
        # Break if we've processed the whole file
        if [ $OFFSET -ge $FILE_SIZE ]; then
            break
        fi
    done
else
    echo "Error: Neither 'split' nor 'dd' command is available"
    echo "Install coreutils: sudo apt-get install coreutils"
    exit 1
fi

echo "Shard splitting complete!"
echo ""

# Show created shards
echo "Created shards in $SHARD_OUTPUT_DIR:"
for file in "$SHARD_OUTPUT_DIR"/shard-*.gguf; do
    if [ -f "$file" ]; then
        SIZE_MB=$(du -m "$file" 2>/dev/null | cut -f1 || echo "0")
        FILENAME=$(basename "$file")
        printf "  %-30s %10s MB\n" "$FILENAME" "$SIZE_MB"
    fi
done

echo ""
echo "Note: These are byte-level splits. For proper GGUF layer-based splitting,"
echo "you may need specialized tools that understand the GGUF format structure."
echo ""
echo "Shards are ready for distributed inference across $NUM_SHARDS nodes!"
echo ""






