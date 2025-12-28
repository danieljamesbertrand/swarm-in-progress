# PowerShell script to list safetensors files on rsync.net server
# Usage: .\list_rsync_files.ps1

$hostname = "zh5605.rsync.net"
$username = "zh5605"
$password = "3da393f1"

Write-Host ""
Write-Host "Searching for safetensors files on $hostname..." -ForegroundColor Cyan
Write-Host ""

# Method 1: Try with SSH (will prompt for password - enter: 3da393f1)
Write-Host "Method 1: Using SSH with find command..." -ForegroundColor Yellow
Write-Host "Note: You will be prompted for password. Enter: $password" -ForegroundColor Cyan
try {
    $result = ssh -o StrictHostKeyChecking=no -o ConnectTimeout=10 "${username}@${hostname}" "find . -name '*.safetensors' -type f" 2>&1
    if ($LASTEXITCODE -eq 0 -and $result) {
        Write-Host $result -ForegroundColor Green
    } else {
        Write-Host "SSH command completed but may need password authentication" -ForegroundColor Yellow
    }
} catch {
    Write-Host "SSH not available or requires authentication" -ForegroundColor Red
}

# Method 2: Try with gfind (GNU find) as mentioned in rsync.net docs
Write-Host ""
Write-Host "Method 2: Using gfind (GNU find)..." -ForegroundColor Yellow
try {
    $result = ssh -o StrictHostKeyChecking=no "${username}@${hostname}" "gfind . -name '*.safetensors' -type f -ls" 2>&1
    if ($LASTEXITCODE -eq 0 -and $result) {
        Write-Host $result -ForegroundColor Green
    }
} catch {
    Write-Host "gfind command failed" -ForegroundColor Red
}

# Method 3: List all files and filter
Write-Host ""
Write-Host "Method 3: Listing all files and filtering..." -ForegroundColor Yellow
try {
    $result = ssh -o StrictHostKeyChecking=no "${username}@${hostname}" "ls -lh | grep safetensors" 2>&1
    if ($LASTEXITCODE -eq 0 -and $result) {
        Write-Host $result -ForegroundColor Green
    }
} catch {
    Write-Host "ls command failed" -ForegroundColor Red
}

Write-Host ""
Write-Host "Tip: If SSH requires a password, you can:" -ForegroundColor Cyan
Write-Host "   1. Set up SSH keys: ssh-keygen -t ed25519" -ForegroundColor White
Write-Host "   2. Copy key to server: ssh-copy-id ${username}@${hostname}" -ForegroundColor White
Write-Host "   3. Or use the Rust download_shards example with SCP" -ForegroundColor White
Write-Host ""
