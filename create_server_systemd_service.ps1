# Create systemd service for rendezvous server auto-start

param(
    [string]$RemoteUser = "dbertrand",
    [string]$RemoteHost = "eagleoneonline.ca",
    [string]$RemoteDir = "/home/dbertrand/punch-simple",
    [string]$ServiceName = "punch-rendezvous"
)

Write-Host ""
Write-Host "========================================" -ForegroundColor Cyan
Write-Host "  CREATE SYSTEMD SERVICE" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

# Create systemd service file
$serviceContent = @"
[Unit]
Description=Punch Rendezvous Server (QUIC Bootstrap Node)
After=network.target

[Service]
Type=simple
User=dbertrand
WorkingDirectory=$RemoteDir
ExecStart=$RemoteDir/target/release/server --listen-addr 0.0.0.0 --port 51820 --transport quic --seed-dir $RemoteDir/shards
Restart=always
RestartSec=10
StandardOutput=append:$RemoteDir/server.log
StandardError=append:$RemoteDir/server.log

[Install]
WantedBy=multi-user.target
"@

Write-Host "[1/4] Creating systemd service file..." -ForegroundColor Yellow
$serviceFile = "/tmp/punch-rendezvous.service"
ssh -F NUL ${RemoteUser}@${RemoteHost} "cat > $serviceFile << 'EOFSERVICE'
$serviceContent
EOFSERVICE
"

Write-Host "[2/4] Installing service..." -ForegroundColor Yellow
ssh -F NUL ${RemoteUser}@${RemoteHost} "sudo mv $serviceFile /etc/systemd/system/${ServiceName}.service && sudo chmod 644 /etc/systemd/system/${ServiceName}.service"

Write-Host "[3/4] Reloading systemd..." -ForegroundColor Yellow
ssh -F NUL ${RemoteUser}@${RemoteHost} "sudo systemctl daemon-reload"

Write-Host "[4/4] Enabling service for auto-start..." -ForegroundColor Yellow
ssh -F NUL ${RemoteUser}@${RemoteHost} "sudo systemctl enable ${ServiceName}.service"

Write-Host ""
Write-Host "========================================" -ForegroundColor Green
Write-Host "  SERVICE CREATED" -ForegroundColor Green
Write-Host "========================================" -ForegroundColor Green
Write-Host ""
Write-Host "Service: ${ServiceName}.service" -ForegroundColor Cyan
Write-Host "Status: Enabled for auto-start on reboot" -ForegroundColor Green
Write-Host ""
Write-Host "To start the service now:" -ForegroundColor Yellow
Write-Host "  ssh ${RemoteUser}@${RemoteHost} 'sudo systemctl start ${ServiceName}'" -ForegroundColor White
Write-Host ""
Write-Host "To check service status:" -ForegroundColor Yellow
Write-Host "  ssh ${RemoteUser}@${RemoteHost} 'sudo systemctl status ${ServiceName}'" -ForegroundColor White
Write-Host ""
Write-Host "To view logs:" -ForegroundColor Yellow
Write-Host "  ssh ${RemoteUser}@${RemoteHost} 'sudo journalctl -u ${ServiceName} -f'" -ForegroundColor White
Write-Host ""
