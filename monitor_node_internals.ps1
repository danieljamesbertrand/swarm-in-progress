# Monitor Node and Rendezvous Server Internal State
# Continuously probes nodes and server to show what's happening internally

param(
    [string]$RendezvousHost = "eagleoneonline.ca",
    [int]$RendezvousPort = 51820,
    [int]$DiagnosticsPort = 51821,
    [int]$IntervalSeconds = 5,
    [switch]$Continuous = $true
)

$ErrorActionPreference = "Continue"

function Write-ColorOutput {
    param([string]$Message, [string]$Color = "White")
    Write-Host $Message -ForegroundColor $Color
}

function Get-RendezvousDiagnostics {
    param([string]$Host, [int]$Port)
    
    try {
        $url = "http://${Host}:${Port}/diagnostics"
        $response = Invoke-RestMethod -Uri $url -Method Get -TimeoutSec 5 -ErrorAction Stop
        return $response
    } catch {
        Write-ColorOutput "  [ERROR] Failed to query rendezvous diagnostics: $_" "Red"
        return $null
    }
}

function Get-RendezvousEvents {
    param([string]$Host, [int]$Port, [int]$Limit = 20)
    
    try {
        $url = "http://${Host}:${Port}/diagnostics/events?limit=$Limit"
        $response = Invoke-RestMethod -Uri $url -Method Get -TimeoutSec 5 -ErrorAction Stop
        return $response
    } catch {
        return $null
    }
}

function Format-Diagnostics {
    param($Diagnostics)
    
    if (-not $Diagnostics) {
        Write-ColorOutput "  [NO DATA] Rendezvous server not responding" "Yellow"
        return
    }
    
    Write-ColorOutput "  Connections:" "Cyan"
    Write-ColorOutput "    Total: $($Diagnostics.total_connections)" "White"
    Write-ColorOutput "    Active: $($Diagnostics.active_connections)" "Green"
    Write-ColorOutput "    Failed: $($Diagnostics.failed_connections)" "Red"
    
    if ($Diagnostics.recent_events -and $Diagnostics.recent_events.Count -gt 0) {
        Write-ColorOutput "  Recent Events (last 5):" "Cyan"
        $Diagnostics.recent_events | Select-Object -First 5 | ForEach-Object {
            $eventType = $_.event_type
            $peerId = if ($_.peer_id) { $_.peer_id.Substring(0, [Math]::Min(12, $_.peer_id.Length)) } else { "N/A" }
            $timestamp = if ($_.timestamp) { Get-Date -UnixTimeSeconds $_.timestamp -Format "HH:mm:ss" } else { "N/A" }
            Write-ColorOutput "    [$timestamp] $eventType (peer: $peerId...)" "Gray"
        }
    }
}

function Get-NodeProcesses {
    # Find running node processes
    $processes = Get-Process | Where-Object {
        $_.ProcessName -eq "cargo" -or 
        $_.ProcessName -eq "node" -or
        ($_.CommandLine -like "*shard*" -and $_.CommandLine -like "*--shard-id*")
    } -ErrorAction SilentlyContinue
    
    return $processes
}

function Extract-PeerIdFromLogs {
    # Try to extract peer IDs from node output/logs
    # This is a placeholder - in practice, you'd read from log files or process output
    return @()
}

function Show-NodeStatus {
    param([array]$PeerIds)
    
    Write-ColorOutput "  Node Status:" "Cyan"
    
    if ($PeerIds.Count -eq 0) {
        Write-ColorOutput "    [INFO] No peer IDs discovered yet" "Yellow"
        Write-ColorOutput "    [TIP] Check node windows for Peer ID messages" "Gray"
        return
    }
    
    foreach ($peerId in $PeerIds) {
        Write-ColorOutput "    Peer: $($peerId.Substring(0, [Math]::Min(20, $peerId.Length)))..." "White"
    }
}

function Show-SwarmReadiness {
    param($Diagnostics, [array]$NodeStatuses)
    
    Write-ColorOutput "" "White"
    Write-ColorOutput "  Swarm Readiness Analysis:" "Cyan"
    
    # Check rendezvous server connection health
    if ($Diagnostics) {
        $activeConnections = $Diagnostics.active_connections
        if ($activeConnections -ge 8) {
            Write-ColorOutput "    [OK] $activeConnections active connections (expecting 8+)" "Green"
        } else {
            Write-ColorOutput "    [WARNING] Only $activeConnections active connections (expecting 8+)" "Yellow"
        }
    }
    
    Write-ColorOutput "    [INFO] Check node windows for:" "Yellow"
    Write-ColorOutput "      - [SHARD] SHARD X LOADED messages" "White"
    Write-ColorOutput "      - [STATUS] Discovered Shards: X / 8" "White"
    Write-ColorOutput "      - [SWARM] SWARM IS READY messages" "Green"
}

function Show-Header {
    Clear-Host
    Write-ColorOutput "========================================" "Cyan"
    Write-ColorOutput "  NODE INTERNAL STATE MONITOR" "Cyan"
    Write-ColorOutput "========================================" "Cyan"
    Write-ColorOutput ""
    Write-ColorOutput "Rendezvous Server: $RendezvousHost:$RendezvousPort" "White"
    Write-ColorOutput "Diagnostics Port: $DiagnosticsPort" "White"
    Write-ColorOutput "Update Interval: $IntervalSeconds seconds" "White"
    Write-ColorOutput "Time: $(Get-Date -Format 'yyyy-MM-dd HH:mm:ss')" "Gray"
    Write-ColorOutput ""
}

function Main-Loop {
    $iteration = 0
    
    while ($true) {
        $iteration++
        Show-Header
        Write-ColorOutput "[Iteration $iteration] Probing internal state..." "Yellow"
        Write-ColorOutput ""
        
        # Query rendezvous server diagnostics
        Write-ColorOutput "Rendezvous Server Diagnostics:" "Cyan"
        $diagnostics = Get-RendezvousDiagnostics -Host $RendezvousHost -Port $DiagnosticsPort
        Format-Diagnostics -Diagnostics $diagnostics
        
        Write-ColorOutput ""
        
        # Check node processes
        Write-ColorOutput "Node Processes:" "Cyan"
        $processes = Get-NodeProcesses
        if ($processes) {
            Write-ColorOutput "  [OK] Found $($processes.Count) node-related process(es)" "Green"
        } else {
            Write-ColorOutput "  [WARNING] No node processes found" "Yellow"
        }
        
        Write-ColorOutput ""
        
        # Try to get peer IDs (would need to parse logs or connect to nodes)
        $peerIds = Extract-PeerIdFromLogs
        Show-NodeStatus -PeerIds $peerIds
        
        # Swarm readiness analysis
        Show-SwarmReadiness -Diagnostics $diagnostics -NodeStatuses @()
        
        Write-ColorOutput ""
        Write-ColorOutput "========================================" "Cyan"
        Write-ColorOutput "  Next update in $IntervalSeconds seconds..." "Gray"
        Write-ColorOutput "  Press Ctrl+C to stop" "Gray"
        Write-ColorOutput ""
        
        if (-not $Continuous) {
            break
        }
        
        Start-Sleep -Seconds $IntervalSeconds
    }
}

# Main execution
try {
    Main-Loop
} catch {
    Write-ColorOutput "[ERROR] Monitoring failed: $_" "Red"
    Write-ColorOutput "Stack trace: $($_.ScriptStackTrace)" "Red"
    exit 1
}
