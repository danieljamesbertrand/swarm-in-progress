# Script to add SSH key to rsync.net server using password
# Usage: .\add_ssh_key.ps1

$hostname = "zh5605.rsync.net"
$username = "zh5605"
$password = "3da393f1"

Write-Host ""
Write-Host "Adding SSH public key to $hostname..." -ForegroundColor Cyan
Write-Host ""

# Read the public key
$pubkeyPath = "$env:USERPROFILE\.ssh\id_ed25519.pub"
if (-not (Test-Path $pubkeyPath)) {
    Write-Host "Error: SSH public key not found at $pubkeyPath" -ForegroundColor Red
    Write-Host "Generate one with: ssh-keygen -t ed25519" -ForegroundColor Yellow
    exit 1
}

$pubkey = Get-Content $pubkeyPath
Write-Host "Public key to add:" -ForegroundColor Yellow
Write-Host $pubkey -ForegroundColor White
Write-Host ""

# Method 1: Try using echo with SSH (will prompt for password)
Write-Host "Method 1: Using SSH to add key (you'll be prompted for password)..." -ForegroundColor Yellow
Write-Host "Run this command manually and enter password when prompted:" -ForegroundColor Cyan
Write-Host "echo '$pubkey' | ssh ${username}@${hostname} 'mkdir -p .ssh && chmod 700 .ssh && cat >> .ssh/authorized_keys && chmod 600 .ssh/authorized_keys'" -ForegroundColor White
Write-Host ""

# Method 2: Create a temporary script file for plink
Write-Host "Method 2: Using plink (if available)..." -ForegroundColor Yellow
$plinkPath = where.exe plink 2>$null
if ($plinkPath) {
    Write-Host "Plink found at: $plinkPath" -ForegroundColor Green
    Write-Host "You can use plink with password to add the key" -ForegroundColor Cyan
} else {
    Write-Host "Plink not found. Install PuTTY to use this method." -ForegroundColor Yellow
}

Write-Host ""
Write-Host "Alternative: Use the Rust downloader which handles SCP authentication:" -ForegroundColor Cyan
Write-Host "  cargo run --example download_shards" -ForegroundColor White
Write-Host ""






