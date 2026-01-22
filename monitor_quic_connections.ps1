# Monitor QUIC Connections - Server and Node
# Monitors both rendezvous server and local node QUIC connections

param(
    [string]$RemoteUser = "dbertrand",
    [string]$RemoteHost = "eagleoneonline.ca",
    [string]$RemoteDir = "/home/dbertrand/punch-simple",
    [int]$IntervalSeconds = 5
)

Write-Host ""
Write-Host "========================================" -ForegroundColor Cyan
Write-Host "  QUIC CONNECTION MONITOR" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""
Write-Host "Monitoring QUIC connections on:" -ForegroundColor Yellow
Write-Host "  - Rendezvous Server: $RemoteHost" -ForegroundColor White
Write-Host "  - Local Node: This machine" -ForegroundColor White
Write-Host ""
Write-Host "Press Ctrl+C to stop monitoring" -ForegroundColor Gray
Write-Host ""

$iteration = 0

try {
    while ($true) {
        $iteration++
        $timestamp = Get-Date -Format "yyyy-MM-dd HH:mm:ss"
        
        Write-Host "`n[$timestamp] === Iteration $iteration ===" -ForegroundColor Cyan
        
        # Check server status
        Write-Host "`n[SERVER] Rendezvous Server Status:" -ForegroundColor Yellow
        $serverProcess = ssh -F NUL ${RemoteUser}@${RemoteHost} 'ps aux | grep "./target/release/server" | grep -v grep'
        if ($serverProcess) {
            Write-Host "  [OK] Server process running" -ForegroundColor Green
            $serverProcess -split "`n" | ForEach-Object { Write-Host "    $_" -ForegroundColor Gray }
        } else {
            Write-Host "  [ERROR] Server process NOT running!" -ForegroundColor Red
        }
        
        # Check server UDP port
        $udpPort = ssh -F NUL ${RemoteUser}@${RemoteHost} 'ss -uln | grep 51820 || echo "not listening"'
        if ($udpPort -match "51820") {
            Write-Host "  [OK] UDP port 51820 listening" -ForegroundColor Green
        } else {
            Write-Host "  [WARNING] UDP port 51820 not listening" -ForegroundColor Yellow
        }
        
        # Check server logs for connections
        Write-Host "`n[SERVER] Recent connection events:" -ForegroundColor Yellow
        $serverLogs = ssh -F NUL ${RemoteUser}@${RemoteHost} "tail -20 $RemoteDir/server.log 2>/dev/null | grep -E '(Connection|Listening|QUIC|peer)' | tail -5"
        if ($serverLogs) {
            $serverLogs -split "`n" | ForEach-Object { 
                if ($_ -match "Connection established") {
                    Write-Host "    [CONNECT] $_" -ForegroundColor Green
                } elseif ($_ -match "Connection closed") {
                    Write-Host "    [DISCONNECT] $_" -ForegroundColor Red
                } elseif ($_ -match "Listening") {
                    Write-Host "    [LISTEN] $_" -ForegroundColor Cyan
                } else {
                    Write-Host "    $_" -ForegroundColor Gray
                }
            }
        } else {
            Write-Host "    No recent connection events" -ForegroundColor Gray
        }
        
        # Check local node process
        Write-Host "`n[NODE] Local Node Status:" -ForegroundColor Yellow
        $nodeProcess = Get-Process -Name "node" -ErrorAction SilentlyContinue
        if ($nodeProcess) {
            Write-Host "  [OK] Node process running (PID: $($nodeProcess.Id))" -ForegroundColor Green
        } else {
            Write-Host "  [WARNING] Node process not found" -ForegroundColor Yellow
            Write-Host "    (Node may be running as 'cargo' or 'rust' process)" -ForegroundColor Gray
        }
        
        # Check for cargo/rust processes (node might be running via cargo run)
        $cargoProcess = Get-Process | Where-Object { $_.ProcessName -match "cargo|rust" -or $_.CommandLine -match "shard-listener" } -ErrorAction SilentlyContinue
        if ($cargoProcess) {
            Write-Host "  [OK] Found cargo/rust process (likely the node)" -ForegroundColor Green
        }
        
        # Check local UDP connections
        Write-Host "`n[NODE] Local UDP connections to 51820:" -ForegroundColor Yellow
        $localConnections = netstat -an | Select-String "51820" | Select-String "UDP"
        if ($localConnections) {
            $localConnections | ForEach-Object { Write-Host "    $_" -ForegroundColor Gray }
        } else {
            Write-Host "    No UDP connections to port 51820" -ForegroundColor Gray
        }
        
        # Check firewall status
        Write-Host "`n[FIREWALL] UDP 51820 rules:" -ForegroundColor Yellow
        $firewallRules = ssh -F NUL ${RemoteUser}@${RemoteHost} "sudo ufw status | grep '51820/udp'"
        if ($firewallRules) {
            $firewallRules -split "`n" | ForEach-Object {
                if ($_ -match "162.221.207.169") {
                    Write-Host "    [OK] $_" -ForegroundColor Green
                } else {
                    Write-Host "    $_" -ForegroundColor Gray
                }
            }
        }
        
        Write-Host "`n--- Waiting $IntervalSeconds seconds until next check ---" -ForegroundColor DarkGray
        Start-Sleep -Seconds $IntervalSeconds
    }
} catch {
    Write-Host "`n[ERROR] Monitoring stopped: $_" -ForegroundColor Red
}
