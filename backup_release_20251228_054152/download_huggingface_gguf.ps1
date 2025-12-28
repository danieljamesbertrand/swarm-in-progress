# PowerShell script to download .gguf files from Hugging Face
# Usage: .\download_huggingface_gguf.ps1 [model_name] [quantization]
# Example: .\download_huggingface_gguf.ps1 "mistralai/Mistral-7B-Instruct-v0.2" "Q4_K_M"

param(
    [string]$ModelName = "TheBloke/Mistral-7B-Instruct-v0.2-GGUF",
    [string]$Quantization = "Q4_K_M"
)

$LocalCache = "models_cache"
if (-not (Test-Path $LocalCache)) {
    New-Item -ItemType Directory -Path $LocalCache | Out-Null
}

Write-Host ""
Write-Host "HUGGING FACE GGUF DOWNLOADER" -ForegroundColor Cyan
Write-Host ""
Write-Host "Model: $ModelName" -ForegroundColor Yellow
Write-Host "Quantization: $Quantization" -ForegroundColor Yellow
Write-Host ""

# Hugging Face base URLs
$HfFilesBase = "https://huggingface.co"
$ModelShortName = $ModelName.Split('/')[-1]

# Common GGUF filename patterns
$GgufPatterns = @(
    "$ModelShortName.$Quantization.gguf",
    "model.$Quantization.gguf",
    "$Quantization.gguf"
)

Write-Host "Trying to download .gguf files..." -ForegroundColor Cyan
Write-Host ""

$Success = $false
foreach ($pattern in $GgufPatterns) {
    $url = "$HfFilesBase/$ModelName/resolve/main/$pattern"
    $destPath = Join-Path $LocalCache $pattern
    
    # Skip if already exists
    if (Test-Path $destPath) {
        $sizeMB = [math]::Round((Get-Item $destPath).Length / 1MB, 2)
        Write-Host "Skipping $pattern (already exists, $sizeMB MB)" -ForegroundColor Yellow
        $Success = $true
        continue
    }
    
    Write-Host "Trying: $pattern" -ForegroundColor Cyan
    Write-Host "URL: $url" -ForegroundColor Gray
    
    try {
        $ProgressPreference = 'SilentlyContinue'
        Invoke-WebRequest -Uri $url -OutFile $destPath -ErrorAction Stop
        
        if (Test-Path $destPath) {
            $sizeMB = [math]::Round((Get-Item $destPath).Length / 1MB, 2)
            Write-Host "Complete! ($sizeMB MB)" -ForegroundColor Green
            $Success = $true
            break
        }
    } catch {
        Write-Host "Not found at this URL" -ForegroundColor Yellow
    }
}

if (-not $Success) {
    Write-Host ""
    Write-Host "Could not automatically find .gguf files" -ForegroundColor Yellow
    Write-Host ""
    Write-Host "Manual download options:" -ForegroundColor Cyan
    Write-Host "  1. Visit: https://huggingface.co/$ModelName" -ForegroundColor White
    Write-Host "  2. Look for .gguf files in the Files tab" -ForegroundColor White
    Write-Host "  3. Download manually and place in: $LocalCache\" -ForegroundColor White
    Write-Host ""
    Write-Host "Popular models with GGUF:" -ForegroundColor Cyan
    Write-Host "  - TheBloke/Llama-2-7B-Chat-GGUF" -ForegroundColor White
    Write-Host "  - TheBloke/Mistral-7B-Instruct-v0.2-GGUF" -ForegroundColor White
    Write-Host "  - TheBloke/Llama-2-13B-Chat-GGUF" -ForegroundColor White
    Write-Host ""
}

# Show what we have
Write-Host ""
Write-Host "Contents of ${LocalCache}:" -ForegroundColor Cyan

Get-ChildItem "$LocalCache\*" -Include *.safetensors,*.gguf -ErrorAction SilentlyContinue | ForEach-Object {
    $sizeMB = [math]::Round($_.Length / 1MB, 2)
    Write-Host ("  {0,-50} {1,10} MB" -f $_.Name, $sizeMB) -ForegroundColor White
}

Write-Host ""
Write-Host "Done!" -ForegroundColor Green
Write-Host ""
