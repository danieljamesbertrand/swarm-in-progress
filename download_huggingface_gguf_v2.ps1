# PowerShell script to download .gguf files from Hugging Face
# Uses Hugging Face API to find available files
# Usage: .\download_huggingface_gguf_v2.ps1 [model_name] [quantization]
# Example: .\download_huggingface_gguf_v2.ps1 "TheBloke/Mistral-7B-Instruct-v0.2-GGUF" "Q4_K_M"

param(
    [string]$ModelName = "TheBloke/Mistral-7B-Instruct-v0.2-GGUF",
    [string]$Quantization = "Q4_K_M"
)

$LocalCache = "models_cache"
if (-not (Test-Path $LocalCache)) {
    New-Item -ItemType Directory -Path $LocalCache | Out-Null
}

Write-Host ""
Write-Host "HUGGING FACE GGUF DOWNLOADER (v2)" -ForegroundColor Cyan
Write-Host ""
Write-Host "Model: $ModelName" -ForegroundColor Yellow
Write-Host "Quantization: $Quantization" -ForegroundColor Yellow
Write-Host ""

# Hugging Face API endpoint
$HfApiBase = "https://huggingface.co/api/models"
$HfFilesBase = "https://huggingface.co"

Write-Host "Fetching file list from Hugging Face API..." -ForegroundColor Cyan

try {
    # Get file tree from Hugging Face API
    $apiUrl = "$HfApiBase/$ModelName/tree/main"
    $response = Invoke-RestMethod -Uri $apiUrl -Method Get -ErrorAction Stop
    
    # Find .gguf files
    $ggufFiles = $response | Where-Object { $_.path -like "*.gguf" }
    
    if ($ggufFiles.Count -eq 0) {
        Write-Host "No .gguf files found in repository" -ForegroundColor Yellow
        Write-Host "Visit: https://huggingface.co/$ModelName to see available files" -ForegroundColor Cyan
        exit
    }
    
    Write-Host "Found $($ggufFiles.Count) .gguf file(s):" -ForegroundColor Green
    foreach ($file in $ggufFiles) {
        $sizeGB = [math]::Round($file.size / 1GB, 2)
        Write-Host "  - $($file.path) ($sizeGB GB)" -ForegroundColor White
    }
    Write-Host ""
    
    # Filter by quantization if specified
    $filesToDownload = if ($Quantization) {
        $ggufFiles | Where-Object { $_.path -like "*$Quantization*" }
    } else {
        $ggufFiles
    }
    
    if ($filesToDownload.Count -eq 0 -and $Quantization) {
        Write-Host "No files found with quantization: $Quantization" -ForegroundColor Yellow
        Write-Host "Available quantizations:" -ForegroundColor Cyan
        $ggufFiles | ForEach-Object { 
            if ($_.path -match '\.(Q\d+_K_[MS]|Q\d_K|F16|F32)\.gguf$') {
                $_.path -replace '.*\.(Q\d+_K_[MS]|Q\d_K|F16|F32)\.gguf$', '$1'
            }
        } | Sort-Object -Unique | ForEach-Object { Write-Host "  - $_" -ForegroundColor White }
        exit
    }
    
    # Download files
    foreach ($file in $filesToDownload) {
        $filename = $file.path
        $url = "$HfFilesBase/$ModelName/resolve/main/$filename"
        $destPath = Join-Path $LocalCache $filename
        
        # Skip if already exists
        if (Test-Path $destPath) {
            $sizeMB = [math]::Round((Get-Item $destPath).Length / 1MB, 2)
            Write-Host "Skipping $filename (already exists, $sizeMB MB)" -ForegroundColor Yellow
            continue
        }
        
        $sizeGB = [math]::Round($file.size / 1GB, 2)
        Write-Host "Downloading: $filename ($sizeGB GB)..." -ForegroundColor Cyan
        Write-Host "This may take a while for large files..." -ForegroundColor Gray
        
        try {
            $ProgressPreference = 'Continue'
            Invoke-WebRequest -Uri $url -OutFile $destPath -ErrorAction Stop
            
            if (Test-Path $destPath) {
                $sizeMB = [math]::Round((Get-Item $destPath).Length / 1MB, 2)
                Write-Host "Complete! ($sizeMB MB)" -ForegroundColor Green
            }
        } catch {
            Write-Host "Failed to download: $_" -ForegroundColor Red
        }
        Write-Host ""
    }
    
} catch {
    Write-Host "Error accessing Hugging Face API: $_" -ForegroundColor Red
    Write-Host ""
    Write-Host "Trying direct download with common patterns..." -ForegroundColor Yellow
    
    # Fallback to direct download attempts
    $patterns = @(
        "mistral-7b-instruct-v0.2.$Quantization.gguf",
        "model.$Quantization.gguf",
        "$Quantization.gguf"
    )
    
    foreach ($pattern in $patterns) {
        $url = "$HfFilesBase/$ModelName/resolve/main/$pattern"
        $destPath = Join-Path $LocalCache $pattern
        
        try {
            Invoke-WebRequest -Uri $url -OutFile $destPath -ErrorAction Stop
            if (Test-Path $destPath) {
                $sizeMB = [math]::Round((Get-Item $destPath).Length / 1MB, 2)
                Write-Host "Downloaded: $pattern ($sizeMB MB)" -ForegroundColor Green
                break
            }
        } catch {
            # Continue to next pattern
        }
    }
}

# Show what we have
Write-Host ""
Write-Host "Contents of ${LocalCache}:" -ForegroundColor Cyan

Get-ChildItem "${LocalCache}\*" -Include *.safetensors,*.gguf -ErrorAction SilentlyContinue | ForEach-Object {
    $sizeMB = [math]::Round($_.Length / 1MB, 2)
    Write-Host ("  {0,-50} {1,10} MB" -f $_.Name, $sizeMB) -ForegroundColor White
}

Write-Host ""
Write-Host "Done!" -ForegroundColor Green
Write-Host ""






