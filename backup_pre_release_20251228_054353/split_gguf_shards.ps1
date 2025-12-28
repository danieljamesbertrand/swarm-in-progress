# PowerShell script to split a .gguf file into 8 shards for distributed inference
# Usage: .\split_gguf_shards.ps1 [gguf_file] [num_shards]
# Example: .\split_gguf_shards.ps1 "models_cache\model.gguf" 8

param(
    [string]$GgufFile = "",
    [int]$NumShards = 8
)

$LocalCache = "models_cache"
$ShardOutputDir = Join-Path $LocalCache "shards"

Write-Host ""
Write-Host "GGUF FILE SHARD SPLITTER" -ForegroundColor Cyan
Write-Host ""

# Find .gguf file if not specified
if ([string]::IsNullOrEmpty($GgufFile)) {
    $ggufFiles = Get-ChildItem "$LocalCache\*.gguf" -ErrorAction SilentlyContinue
    if ($ggufFiles.Count -eq 0) {
        Write-Host "No .gguf files found in $LocalCache" -ForegroundColor Red
        Write-Host "Please download a .gguf file first or specify the path" -ForegroundColor Yellow
        exit 1
    } elseif ($ggufFiles.Count -eq 1) {
        $GgufFile = $ggufFiles[0].FullName
        Write-Host "Using: $($ggufFiles[0].Name)" -ForegroundColor Green
    } else {
        Write-Host "Multiple .gguf files found. Please specify which one:" -ForegroundColor Yellow
        $index = 1
        foreach ($file in $ggufFiles) {
            $sizeGB = [math]::Round($file.Length / 1GB, 2)
            Write-Host "  $index. $($file.Name) ($sizeGB GB)" -ForegroundColor White
            $index++
        }
        $choice = Read-Host "Enter number"
        $GgufFile = $ggufFiles[$choice - 1].FullName
    }
}

if (-not (Test-Path $GgufFile)) {
    Write-Host "File not found: $GgufFile" -ForegroundColor Red
    exit 1
}

$fileInfo = Get-Item $GgufFile
$fileSize = $fileInfo.Length
$shardSize = [math]::Ceiling($fileSize / $NumShards)

Write-Host "Input file: $($fileInfo.Name)" -ForegroundColor Cyan
Write-Host "File size: $([math]::Round($fileSize / 1GB, 2)) GB" -ForegroundColor Cyan
Write-Host "Number of shards: $NumShards" -ForegroundColor Cyan
Write-Host "Shard size: $([math]::Round($shardSize / 1GB, 2)) GB each" -ForegroundColor Cyan
Write-Host ""

# Create shards directory
if (-not (Test-Path $ShardOutputDir)) {
    New-Item -ItemType Directory -Path $ShardOutputDir | Out-Null
}

Write-Host "Splitting file into shards..." -ForegroundColor Yellow
Write-Host ""

# Read the file in chunks and write shards
$inputStream = [System.IO.File]::OpenRead($GgufFile)
$buffer = New-Object byte[] 10485760  # 10MB buffer for faster I/O
$shardNumber = 0
$bytesRead = 0
$shardBytesWritten = 0
$shardStream = $null

try {
    while ($shardNumber -lt $NumShards) {
        $shardPath = Join-Path $ShardOutputDir "shard-$shardNumber.gguf"
        
        if ($shardStream) {
            $shardStream.Close()
        }
        
        $shardStream = [System.IO.File]::Create($shardPath)
        $shardBytesWritten = 0
        
        Write-Host "Creating shard $($shardNumber + 1)/$NumShards: $(Split-Path $shardPath -Leaf)" -ForegroundColor Cyan
        
        while ($shardBytesWritten -lt $shardSize -and ($bytesRead = $inputStream.Read($buffer, 0, $buffer.Length)) -gt 0) {
            $bytesToWrite = [Math]::Min($bytesRead, $shardSize - $shardBytesWritten)
            $shardStream.Write($buffer, 0, $bytesToWrite)
            $shardBytesWritten += $bytesToWrite
            
            # Show progress
            $percent = [math]::Round(($shardBytesWritten / $shardSize) * 100, 1)
            Write-Progress -Activity "Writing shard $($shardNumber + 1)" -Status "$percent% complete" -PercentComplete $percent
            
            # If we've written enough for this shard, break
            if ($shardBytesWritten -ge $shardSize) {
                break
            }
        }
        
        $shardStream.Close()
        $shardStream = $null
        
        $shardFileInfo = Get-Item $shardPath
        $shardSizeMB = [math]::Round($shardFileInfo.Length / 1MB, 2)
        Write-Host "  Complete! ($shardSizeMB MB)" -ForegroundColor Green
        Write-Host ""
        
        $shardNumber++
        
        # If we've read all the file, break
        if ($bytesRead -eq 0) {
            break
        }
    }
} finally {
    if ($shardStream) {
        $shardStream.Close()
    }
    $inputStream.Close()
}

Write-Host "Shard splitting complete!" -ForegroundColor Green
Write-Host ""

# Show created shards
Write-Host "Created shards in ${ShardOutputDir}:" -ForegroundColor Cyan
Get-ChildItem "$ShardOutputDir\shard-*.gguf" | Sort-Object Name | ForEach-Object {
    $sizeMB = [math]::Round($_.Length / 1MB, 2)
    Write-Host ("  {0,-30} {1,10} MB" -f $_.Name, $sizeMB) -ForegroundColor White
}

Write-Host ""
Write-Host "Note: These are byte-level splits. For proper GGUF layer-based splitting," -ForegroundColor Yellow
Write-Host "you may need specialized tools that understand the GGUF format structure." -ForegroundColor Yellow
Write-Host ""
Write-Host "Shards are ready for distributed inference across $NumShards nodes!" -ForegroundColor Green
Write-Host ""
