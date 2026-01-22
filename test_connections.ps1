# Test connections to rendezvous server from this machine

param(
    [string]$ServerHost = "eagleoneonline.ca",
    [string]$ServerIP = "162.221.207.169"
)

$myIP = "170.203.207.66"

Write-Host ""
Write-Host "========================================" -ForegroundColor Cyan
Write-Host "  Testing Connections to Rendezvous Server" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""
Write-Host "Your IP: $myIP" -ForegroundColor Yellow
Write-Host "Server: $ServerHost ($ServerIP)" -ForegroundColor Yellow
Write-Host ""

# Test 1: Diagnostics HTTP endpoint (port 51821)
Write-Host "[1/2] Testing Diagnostics HTTP (port 51821)..." -ForegroundColor Yellow
try {
    $response = Invoke-WebRequest -Uri "http://${ServerIP}:51821/diagnostics/health" -TimeoutSec 5 -UseBasicParsing -ErrorAction Stop
    Write-Host "  ✓ Diagnostics HTTP: SUCCESS" -ForegroundColor Green
    Write-Host "    Status: $($response.StatusCode)" -ForegroundColor White
    Write-Host "    Response: $($response.Content)" -ForegroundColor Gray
} catch {
    Write-Host "  ✗ Diagnostics HTTP: FAILED" -ForegroundColor Red
    Write-Host "    Error: $($_.Exception.Message)" -ForegroundColor Red
}

Write-Host ""

# Test 2: QUIC connection (port 51820 UDP)
Write-Host "[2/2] Testing QUIC Connection (port 51820 UDP)..." -ForegroundColor Yellow
Write-Host "  Note: QUIC uses UDP, requires actual node connection" -ForegroundColor Gray
Write-Host "  Your IP ($myIP) is specifically allowed in firewall" -ForegroundColor Green
Write-Host "  Firewall rule: 51820/udp ALLOW IN 170.203.207.66" -ForegroundColor Gray
Write-Host ""

Write-Host "========================================" -ForegroundColor Cyan
Write-Host "  How You're Allowed to Connect" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""
Write-Host "1. QUIC Rendezvous (UDP 51820):" -ForegroundColor Yellow
Write-Host "   - Your IP ($myIP) has explicit ALLOW rule" -ForegroundColor Green
Write-Host "   - Rule: ufw allow 51820/udp from 170.203.207.66" -ForegroundColor Gray
Write-Host "   - This allows QUIC handshake packets" -ForegroundColor White
Write-Host ""
Write-Host "2. Diagnostics HTTP (TCP 51821):" -ForegroundColor Yellow
Write-Host "   - Port 51821 is open to ANYWHERE" -ForegroundColor Green
Write-Host "   - Rule: ufw allow 51821/tcp from Anywhere" -ForegroundColor Gray
Write-Host "   - This allows web dashboard access" -ForegroundColor White
Write-Host ""
Write-Host "3. Other HTTP/HTTPS:" -ForegroundColor Yellow
Write-Host "   - Port 80 (HTTP): BLOCKED" -ForegroundColor Red
Write-Host "   - Port 443 (HTTPS): BLOCKED" -ForegroundColor Red
Write-Host "   - Port 8080: BLOCKED" -ForegroundColor Red
Write-Host ""
Write-Host "To connect a node, use:" -ForegroundColor Cyan
Write-Host "  .\start_node_to_rendezvous.ps1 -BootstrapHost $ServerHost" -ForegroundColor White
Write-Host ""
