# Configure firewall to block all HTTP/HTTPS except diagnostics port 51821
# Only allows access to punch-rendezvous diagnostics on port 51821

param(
    [string]$RemoteHost = "eagleoneonline.ca",
    [string]$RemoteUser = "dbertrand"
)

$ErrorActionPreference = "Stop"

Write-Host ""
Write-Host "========================================" -ForegroundColor Cyan
Write-Host "  Configuring Firewall for Diagnostics" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""
Write-Host "Policy: Block all HTTP/HTTPS except port 51821" -ForegroundColor Yellow
Write-Host ""

# Step 1: Deny HTTP (port 80)
Write-Host "[1/5] Blocking HTTP (port 80)..." -ForegroundColor Yellow
$result1 = ssh -F NUL "${RemoteUser}@${RemoteHost}" "sudo ufw deny 80/tcp 2>&1"
if ($LASTEXITCODE -eq 0) {
    Write-Host "  ✓ HTTP (port 80) blocked" -ForegroundColor Green
} else {
    Write-Host "  ⚠ HTTP rule may already exist: $result1" -ForegroundColor Yellow
}

# Step 2: Deny HTTPS (port 443)
Write-Host "[2/5] Blocking HTTPS (port 443)..." -ForegroundColor Yellow
$result2 = ssh -F NUL "${RemoteUser}@${RemoteHost}" "sudo ufw deny 443/tcp 2>&1"
if ($LASTEXITCODE -eq 0) {
    Write-Host "  ✓ HTTPS (port 443) blocked" -ForegroundColor Green
} else {
    Write-Host "  ⚠ HTTPS rule may already exist: $result2" -ForegroundColor Yellow
}

# Step 3: Allow diagnostics port 51821
Write-Host "[3/5] Allowing diagnostics port 51821..." -ForegroundColor Yellow
$result3 = ssh -F NUL "${RemoteUser}@${RemoteHost}" "sudo ufw allow 51821/tcp comment 'Punch Rendezvous Diagnostics' 2>&1"
if ($LASTEXITCODE -eq 0) {
    Write-Host "  ✓ Diagnostics port 51821 allowed" -ForegroundColor Green
} else {
    Write-Host "  ⚠ Port 51821 rule may already exist: $result3" -ForegroundColor Yellow
}

# Step 4: Ensure QUIC port 51820 is still allowed (UDP)
Write-Host "[4/5] Ensuring QUIC port 51820 (UDP) is allowed..." -ForegroundColor Yellow
$result4 = ssh -F NUL "${RemoteUser}@${RemoteHost}" "sudo ufw allow 51820/udp comment 'Punch Rendezvous QUIC' 2>&1"
if ($LASTEXITCODE -eq 0) {
    Write-Host "  ✓ QUIC port 51820 (UDP) allowed" -ForegroundColor Green
} else {
    Write-Host "  ⚠ Port 51820 rule may already exist: $result4" -ForegroundColor Yellow
}

# Step 5: Verify configuration
Write-Host "[5/5] Verifying firewall configuration..." -ForegroundColor Yellow
$status = ssh -F NUL "${RemoteUser}@${RemoteHost}" "sudo ufw status numbered 2>&1"
Write-Host ""
Write-Host "Current Firewall Rules:" -ForegroundColor Cyan
Write-Host "$status" -ForegroundColor White
Write-Host ""

# Check specific ports
Write-Host "Port Status:" -ForegroundColor Cyan
$port80 = ssh -F NUL "${RemoteUser}@${RemoteHost}" "sudo ufw status | grep '80/tcp' | head -1"
$port443 = ssh -F NUL "${RemoteUser}@${RemoteHost}" "sudo ufw status | grep '443/tcp' | head -1"
$port51821 = ssh -F NUL "${RemoteUser}@${RemoteHost}" "sudo ufw status | grep '51821' | head -1"

Write-Host "  Port 80 (HTTP): $port80" -ForegroundColor White
Write-Host "  Port 443 (HTTPS): $port443" -ForegroundColor White
Write-Host "  Port 51821 (Diagnostics): $port51821" -ForegroundColor White
Write-Host ""

Write-Host "========================================" -ForegroundColor Green
Write-Host "  Firewall Configuration Complete" -ForegroundColor Green
Write-Host "========================================" -ForegroundColor Green
Write-Host ""
Write-Host "Summary:" -ForegroundColor Cyan
Write-Host "  ✓ HTTP (port 80) - BLOCKED" -ForegroundColor Red
Write-Host "  ✓ HTTPS (port 443) - BLOCKED" -ForegroundColor Red
Write-Host "  ✓ Diagnostics (port 51821) - ALLOWED" -ForegroundColor Green
Write-Host "  ✓ QUIC (port 51820 UDP) - ALLOWED" -ForegroundColor Green
Write-Host ""
Write-Host "Access:" -ForegroundColor Cyan
Write-Host "  Allowed: http://eagleoneonline.ca:51821/" -ForegroundColor Green
Write-Host "  Blocked: http://eagleoneonline.ca/ (port 80)" -ForegroundColor Red
Write-Host "  Blocked: https://eagleoneonline.ca/ (port 443)" -ForegroundColor Red
Write-Host ""
