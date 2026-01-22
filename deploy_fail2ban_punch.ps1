# Deploy fail2ban configuration for Punch Rendezvous Server
# Monitors and blocks suspicious connection attempts on port 51820

param(
    [string]$RemoteHost = "eagleoneonline.ca",
    [string]$RemoteUser = "dbertrand"
)

$ErrorActionPreference = "Stop"

Write-Host ""
Write-Host "========================================" -ForegroundColor Cyan
Write-Host "  Deploying fail2ban for Punch Server" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

# Check if files exist locally
$filterFile = "fail2ban_punch_filter.conf"
$jailFile = "fail2ban_punch_jail.conf"

if (-not (Test-Path $filterFile)) {
    Write-Host "[ERROR] Filter file not found: $filterFile" -ForegroundColor Red
    exit 1
}

if (-not (Test-Path $jailFile)) {
    Write-Host "[ERROR] Jail file not found: $jailFile" -ForegroundColor Red
    exit 1
}

Write-Host "[1/4] Uploading fail2ban filter..." -ForegroundColor Yellow
scp -F NUL "${filterFile}" "${RemoteUser}@${RemoteHost}:/tmp/punch-rendezvous.conf" | Out-Null
if ($LASTEXITCODE -ne 0) {
    Write-Host "[ERROR] Failed to upload filter" -ForegroundColor Red
    exit 1
}

Write-Host "[2/4] Uploading fail2ban jail configuration..." -ForegroundColor Yellow
scp -F NUL "${jailFile}" "${RemoteUser}@${RemoteHost}:/tmp/punch-rendezvous-jail.conf" | Out-Null
if ($LASTEXITCODE -ne 0) {
    Write-Host "[ERROR] Failed to upload jail config" -ForegroundColor Red
    exit 1
}

Write-Host "[3/4] Installing filter and jail on remote server..." -ForegroundColor Yellow
$installCmd = @"
sudo mv /tmp/punch-rendezvous.conf /etc/fail2ban/filter.d/punch-rendezvous.conf
sudo chmod 644 /etc/fail2ban/filter.d/punch-rendezvous.conf
sudo mv /tmp/punch-rendezvous-jail.conf /etc/fail2ban/jail.d/punch-rendezvous.conf
sudo chmod 644 /etc/fail2ban/jail.d/punch-rendezvous.conf
sudo systemctl reload fail2ban
"@

ssh -F NUL "${RemoteUser}@${RemoteHost}" $installCmd
if ($LASTEXITCODE -ne 0) {
    Write-Host "[ERROR] Failed to install fail2ban configuration" -ForegroundColor Red
    exit 1
}

Write-Host "[4/4] Verifying fail2ban configuration..." -ForegroundColor Yellow
$status = ssh -F NUL "${RemoteUser}@${RemoteHost}" "sudo fail2ban-client status punch-rendezvous 2>&1"
if ($status -match "Status for the jail: punch-rendezvous") {
    Write-Host "[OK] fail2ban jail 'punch-rendezvous' is active" -ForegroundColor Green
    Write-Host ""
    Write-Host "$status" -ForegroundColor White
} else {
    Write-Host "[WARNING] Jail may not be active yet. Checking fail2ban status..." -ForegroundColor Yellow
    ssh -F NUL "${RemoteUser}@${RemoteHost}" "sudo fail2ban-client status | grep punch"
}

Write-Host ""
Write-Host "========================================" -ForegroundColor Green
Write-Host "  fail2ban Configuration Complete" -ForegroundColor Green
Write-Host "========================================" -ForegroundColor Green
Write-Host ""
Write-Host "Configuration:" -ForegroundColor Cyan
Write-Host "  - Filter: /etc/fail2ban/filter.d/punch-rendezvous.conf" -ForegroundColor White
Write-Host "  - Jail: /etc/fail2ban/jail.d/punch-rendezvous.conf" -ForegroundColor White
Write-Host "  - Log: /home/dbertrand/punch-simple/server.log" -ForegroundColor White
Write-Host ""
Write-Host "Protection:" -ForegroundColor Cyan
Write-Host "  - Max retries: 5 failed connection attempts" -ForegroundColor White
Write-Host "  - Time window: 5 minutes" -ForegroundColor White
Write-Host "  - Ban duration: 1 hour" -ForegroundColor White
Write-Host ""
Write-Host "Monitor with:" -ForegroundColor Cyan
Write-Host "  ssh ${RemoteUser}@${RemoteHost} 'sudo fail2ban-client status punch-rendezvous'" -ForegroundColor Gray
Write-Host ""
