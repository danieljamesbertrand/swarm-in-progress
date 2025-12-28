#!/bin/bash
# Download .gguf files from Hugging Face
# Usage: 
#   wsl bash download_huggingface_gguf.sh [model_name] [quantization]
#   Example: wsl bash download_huggingface_gguf.sh mistralai/Mistral-7B-Instruct-v0.2 Q4_K_M

set -e

LOCAL_CACHE="models_cache"
mkdir -p "$LOCAL_CACHE"

# Default model (can be overridden)
MODEL_NAME="${1:-mistralai/Mistral-7B-Instruct-v0.2}"
QUANTIZATION="${2:-Q4_K_M}"

echo ""
echo "╔══════════════════════════════════════════════════════════════╗"
echo "║      📥 HUGGING FACE GGUF DOWNLOADER 📥                      ║"
echo "╚══════════════════════════════════════════════════════════════╝"
echo ""
echo "Model: $MODEL_NAME"
echo "Quantization: $QUANTIZATION"
echo ""

# Check if wget or curl is available
if command -v wget &> /dev/null; then
    DOWNLOAD_CMD="wget"
elif command -v curl &> /dev/null; then
    DOWNLOAD_CMD="curl"
else
    echo "❌ Error: Neither wget nor curl is available"
    echo "   Install one: sudo apt-get install wget"
    exit 1
fi

# Function to download file
download_file() {
    local url="$1"
    local dest="$2"
    local filename=$(basename "$dest")
    
    echo "📥 Downloading: $filename"
    echo "   URL: $url"
    
    if [ "$DOWNLOAD_CMD" = "wget" ]; then
        wget --progress=bar:force -O "$dest" "$url" 2>&1 | grep -E "(%|MB|KB)" || true
    else
        curl -L --progress-bar -o "$dest" "$url"
    fi
    
    if [ -f "$dest" ]; then
        SIZE_MB=$(du -m "$dest" 2>/dev/null | cut -f1 || echo "0")
        echo "   ✅ Complete! (${SIZE_MB} MB)"
        return 0
    else
        echo "   ❌ Failed to download"
        return 1
    fi
}

# Try to find .gguf files in the Hugging Face repository
# Hugging Face API endpoint
HF_API_BASE="https://huggingface.co/api/models"
HF_FILES_BASE="https://huggingface.co"

echo "🔍 Searching for .gguf files in repository..."
echo ""

# Try to get file list from Hugging Face
# Common patterns for GGUF files
GGUF_PATTERNS=(
    "${MODEL_NAME##*/}.${QUANTIZATION}.gguf"
    "model.${QUANTIZATION}.gguf"
    "${QUANTIZATION}.gguf"
    "*.gguf"
)

# Try direct download URLs
# Hugging Face uses this pattern: https://huggingface.co/{model}/resolve/main/{filename}
echo "Trying to download common GGUF file patterns..."
echo ""

SUCCESS=0
for pattern in "${GGUF_PATTERNS[@]}"; do
    # Clean up pattern for URL
    filename=$(echo "$pattern" | sed 's/\*//g')
    
    # Try different URL patterns
    URLS=(
        "${HF_FILES_BASE}/${MODEL_NAME}/resolve/main/${filename}"
        "${HF_FILES_BASE}/${MODEL_NAME}/resolve/main/*${QUANTIZATION}*.gguf"
        "${HF_FILES_BASE}/${MODEL_NAME}/blob/main/${filename}"
    )
    
    for url in "${URLS[@]}"; do
        # Skip wildcard URLs for now
        if [[ "$url" == *"*"* ]]; then
            continue
        fi
        
        dest_path="$LOCAL_CACHE/$filename"
        
        # Skip if already exists
        if [ -f "$dest_path" ]; then
            SIZE_MB=$(du -m "$dest_path" 2>/dev/null | cut -f1 || echo "0")
            echo "⏭️  Skipping $filename (already exists, ${SIZE_MB} MB)"
            SUCCESS=1
            continue
        fi
        
        # Try to download
        if download_file "$url" "$dest_path" 2>/dev/null; then
            SUCCESS=1
            break 2
        fi
    done
done

if [ $SUCCESS -eq 0 ]; then
    echo ""
    echo "⚠️  Could not automatically find .gguf files"
    echo ""
    echo "💡 Manual download options:"
    echo "   1. Visit: https://huggingface.co/$MODEL_NAME"
    echo "   2. Look for .gguf files in the 'Files' tab"
    echo "   3. Download manually and place in: $LOCAL_CACHE/"
    echo ""
    echo "   Or try a different model/quantization:"
    echo "   wsl bash download_huggingface_gguf.sh TheBloke/Llama-2-7B-Chat-GGUF Q4_K_M"
    echo ""
fi

# Show what we have
echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "📁 Contents of $LOCAL_CACHE:"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

for file in "$LOCAL_CACHE"/*.{safetensors,gguf} 2>/dev/null; do
    if [ -f "$file" ]; then
        SIZE_MB=$(du -m "$file" 2>/dev/null | cut -f1 || echo "0")
        FILENAME=$(basename "$file")
        printf "   %-50s %10s MB\n" "$FILENAME" "$SIZE_MB"
    fi
done

echo ""
echo "✅ Done!"
echo ""






