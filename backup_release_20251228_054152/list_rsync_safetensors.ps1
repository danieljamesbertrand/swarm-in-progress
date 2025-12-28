# PowerShell script to list safetensors files on rsync.net
# Based on: https://www.rsync.net/resources/howto/remote_commands.html
# Usage: .\list_rsync_safetensors.ps1

$hostname = "zh5605.rsync.net"
$username = "zh5605"

Write-Host "`n🔍 Searching for safetensors files on $hostname...`n" -ForegroundColor Cyan
Write-Host "According to rsync.net docs, you can run commands over SSH like:" -ForegroundColor Yellow
Write-Host "  ssh user@rsync.net 'command'`n" -ForegroundColor Gray

# Try find command (as per rsync.net docs)
Write-Host "Trying: find . -name '*.safetensors' -type f" -ForegroundColor Cyan
ssh -o StrictHostKeyChecking=no "$username@$hostname" "find . -name '*.safetensors' -type f"

# Try gfind (GNU find) as mentioned in docs
Write-Host "`nTrying: gfind . -name '*.safetensors' -type f -ls" -ForegroundColor Cyan
ssh -o StrictHostKeyChecking=no "$username@$hostname" "gfind . -name '*.safetensors' -type f -ls"

# Try ls with grep
Write-Host "`nTrying: ls -lh | grep safetensors" -ForegroundColor Cyan
ssh -o StrictHostKeyChecking=no "$username@$hostname" "ls -lh | grep safetensors"

Write-Host "`n💡 If SSH prompts for password, you can:" -ForegroundColor Yellow
Write-Host "   1. Set up SSH keys: ssh-keygen -t ed25519" -ForegroundColor White
Write-Host "   2. Copy key: ssh-copy-id $username@$hostname" -ForegroundColor White
Write-Host "   3. Or use the Rust example: cargo run --example download_shards (with RSYNC_LIST=1)" -ForegroundColor White
Write-Host ""

