# Active DHT Diagnostic - Actually queries the system to see what's happening

Write-Host ""
Write-Host "================================================================" -ForegroundColor Cyan
Write-Host "  ACTIVE DHT DISCOVERY DIAGNOSTIC" -ForegroundColor Cyan
Write-Host "================================================================" -ForegroundColor Cyan
Write-Host ""

$ErrorActionPreference = "Continue"

# Phase 1: Check if processes are actually running and get their details
Write-Host "[PHASE 1] Process Verification" -ForegroundColor Yellow
Write-Host "------------------------------------------------" -ForegroundColor Gray

$bootstrap = Get-Process -Name "server" -ErrorAction SilentlyContinue
$web = Get-Process -Name "web_server" -ErrorAction SilentlyContinue
$nodes = Get-Process -Name "shard_listener" -ErrorAction SilentlyContinue

if ($bootstrap) {
    Write-Host "Bootstrap: RUNNING (PID: $($bootstrap.Id), Started: $($bootstrap.StartTime))" -ForegroundColor Green
} else {
    Write-Host "Bootstrap: NOT RUNNING" -ForegroundColor Red
    exit 1
}

if ($web) {
    Write-Host "Web Server: RUNNING (PID: $($web.Id), Started: $($web.StartTime))" -ForegroundColor Green
} else {
    Write-Host "Web Server: NOT RUNNING" -ForegroundColor Red
    exit 1
}

$nodeCount = if ($nodes) { $nodes.Count } else { 0 }
Write-Host "Nodes: $nodeCount/4" -ForegroundColor $(if ($nodeCount -eq 4) { 'Green' } else { 'Yellow' })

if ($nodes) {
    Write-Host ""
    Write-Host "Node Details:" -ForegroundColor Gray
    $nodes | ForEach-Object {
        $runtime = (Get-Date) - $_.StartTime
        Write-Host "  PID $($_.Id): Running for $([math]::Round($runtime.TotalSeconds, 0)) seconds" -ForegroundColor Gray
    }
}

Write-Host ""

# Phase 2: Check network connections in detail
Write-Host "[PHASE 2] Network Connection Analysis" -ForegroundColor Yellow
Write-Host "------------------------------------------------" -ForegroundColor Gray

$connections = netstat -an | Select-String "51820" | Select-String "ESTABLISHED"
$connectionCount = ($connections | Measure-Object).Count

Write-Host "Total connections to bootstrap: $connectionCount" -ForegroundColor $(if ($connectionCount -ge 4) { 'Green' } else { 'Yellow' })

# Count unique source ports (each node should have at least one)
$sourcePorts = $connections | ForEach-Object {
    if ($_ -match '(\d+\.\d+\.\d+\.\d+):(\d+)\s+(\d+\.\d+\.\d+\.\d+):51820') {
        $matches[2]
    }
} | Sort-Object -Unique

Write-Host "Unique source ports: $($sourcePorts.Count)" -ForegroundColor Gray
Write-Host ""

# Phase 3: Try to get actual pipeline status from web server
Write-Host "[PHASE 3] Querying Web Server for Pipeline Status" -ForegroundColor Yellow
Write-Host "------------------------------------------------" -ForegroundColor Gray

if ($web) {
    Write-Host "Attempting to connect to WebSocket and get status..." -ForegroundColor Gray
    
    # Try using Node.js if available, otherwise provide instructions
    $nodeAvailable = Get-Command node -ErrorAction SilentlyContinue
    
    if ($nodeAvailable) {
        # Create a simple Node.js script to query WebSocket
        $nodeScript = @"
const WebSocket = require('ws');
const ws = new WebSocket('ws://localhost:8081');

let statusReceived = false;

ws.on('open', () => {
    console.log('Connected to WebSocket');
    setTimeout(() => {
        if (!statusReceived) {
            console.log('No status received within 5 seconds');
            ws.close();
            process.exit(1);
        }
    }, 5000);
});

ws.on('message', (data) => {
    try {
        const msg = JSON.parse(data.toString());
        if (msg.type === 'pipeline_status') {
            statusReceived = true;
            console.log('STATUS:', JSON.stringify({
                online_nodes: msg.online_nodes,
                total_nodes: msg.total_nodes,
                missing_shards: msg.missing_shards,
                is_complete: msg.is_complete
            }, null, 2));
            ws.close();
            process.exit(0);
        }
    } catch (e) {
        // Not JSON or not status message
    }
});

ws.on('error', (err) => {
    console.error('ERROR:', err.message);
    process.exit(1);
});
"@
        
        $nodeScript | Out-File -FilePath "temp_ws_check.js" -Encoding UTF8
        
        try {
            $result = node temp_ws_check.js 2>&1
            if ($result -match 'STATUS:') {
                Write-Host "  [SUCCESS] Retrieved pipeline status" -ForegroundColor Green
                $result | Where-Object { $_ -match 'STATUS:' } | ForEach-Object {
                    Write-Host $_ -ForegroundColor Cyan
                }
            } else {
                Write-Host "  [WARNING] Could not retrieve status via WebSocket" -ForegroundColor Yellow
                Write-Host "  $result" -ForegroundColor Gray
            }
            Remove-Item temp_ws_check.js -ErrorAction SilentlyContinue
        } catch {
            Write-Host "  [ERROR] Failed to query WebSocket: $_" -ForegroundColor Red
            Remove-Item temp_ws_check.js -ErrorAction SilentlyContinue
        }
    } else {
        Write-Host "  [INFO] Node.js not available - cannot query WebSocket directly" -ForegroundColor Yellow
        Write-Host "  [INFO] Open http://localhost:8080 in browser to check status" -ForegroundColor White
    }
} else {
    Write-Host "  [SKIP] Web server not running" -ForegroundColor Yellow
}

Write-Host ""

# Phase 4: Check for common issues
Write-Host "[PHASE 4] Common Issue Detection" -ForegroundColor Yellow
Write-Host "------------------------------------------------" -ForegroundColor Gray

# Check if nodes have been running long enough
if ($nodes) {
    $oldestNode = $nodes | Sort-Object StartTime | Select-Object -First 1
    $runtime = (Get-Date) - $oldestNode.StartTime
    
    if ($runtime.TotalSeconds -lt 30) {
        Write-Host "  [WARNING] Nodes may still be initializing (oldest node: $([math]::Round($runtime.TotalSeconds, 0))s old)" -ForegroundColor Yellow
        Write-Host "  [INFO] DHT discovery can take 30-60 seconds after startup" -ForegroundColor Gray
    } else {
        Write-Host "  [OK] Nodes have been running long enough ($([math]::Round($runtime.TotalSeconds, 0))s)" -ForegroundColor Green
    }
}

# Check if web server has been running long enough
if ($web) {
    $webRuntime = (Get-Date) - $web.StartTime
    if ($webRuntime.TotalSeconds -lt 10) {
        Write-Host "  [WARNING] Web server just started ($([math]::Round($webRuntime.TotalSeconds, 0))s ago)" -ForegroundColor Yellow
        Write-Host "  [INFO] Coordinator needs time to bootstrap to DHT" -ForegroundColor Gray
    }
}

Write-Host ""

# Phase 5: Recommendations
Write-Host "[PHASE 5] Recommendations" -ForegroundColor Yellow
Write-Host "------------------------------------------------" -ForegroundColor Gray
Write-Host ""

if ($nodeCount -eq 4 -and $connectionCount -ge 4) {
    Write-Host "System appears healthy:" -ForegroundColor Green
    Write-Host "  - All processes running" -ForegroundColor Gray
    Write-Host "  - Network connections established" -ForegroundColor Gray
    Write-Host ""
    Write-Host "If DHT discovery still shows 0 nodes:" -ForegroundColor Yellow
    Write-Host "  1. Check node console windows for: [DHT] ANNOUNCED SHARD X TO DHT" -ForegroundColor White
    Write-Host "  2. Check web server console for: [DHT] Discovered shard X" -ForegroundColor White
    Write-Host "  3. Wait 30-60 seconds after startup for DHT to populate" -ForegroundColor White
    Write-Host "  4. If still not working, restart all processes" -ForegroundColor White
} else {
    Write-Host "Issues detected:" -ForegroundColor Red
    if ($nodeCount -lt 4) {
        Write-Host "  - Only $nodeCount/4 nodes running" -ForegroundColor Yellow
    }
    if ($connectionCount -lt 4) {
        Write-Host "  - Only $connectionCount connections to bootstrap (expected 4+)" -ForegroundColor Yellow
    }
}

Write-Host ""
Write-Host "================================================================" -ForegroundColor Cyan
Write-Host "  DIAGNOSTIC COMPLETE" -ForegroundColor Cyan
Write-Host "================================================================" -ForegroundColor Cyan
Write-Host ""

