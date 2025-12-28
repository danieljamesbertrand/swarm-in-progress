# DHT Discovery Diagnostic Script
# Checks all aspects of DHT discovery to diagnose why nodes aren't being found

Write-Host ""
Write-Host "================================================================" -ForegroundColor Cyan
Write-Host "  DHT DISCOVERY DIAGNOSTIC" -ForegroundColor Cyan
Write-Host "================================================================" -ForegroundColor Cyan
Write-Host ""

$ErrorActionPreference = "Continue"

# Phase 1: Process Status
Write-Host "[PHASE 1] Process Status" -ForegroundColor Yellow
Write-Host "------------------------------------------------" -ForegroundColor Gray

$bootstrap = Get-Process -Name "server" -ErrorAction SilentlyContinue
$web = Get-Process -Name "web_server" -ErrorAction SilentlyContinue
$nodes = Get-Process -Name "shard_listener" -ErrorAction SilentlyContinue
$nodeCount = if ($nodes) { $nodes.Count } else { 0 }

Write-Host "Bootstrap Server: $(if ($bootstrap) { "RUNNING (PID: $($bootstrap.Id))" } else { "NOT RUNNING" })" -ForegroundColor $(if ($bootstrap) { 'Green' } else { 'Red' })
Write-Host "Web Server: $(if ($web) { "RUNNING (PID: $($web.Id))" } else { "NOT RUNNING" })" -ForegroundColor $(if ($web) { 'Green' } else { 'Red' })
Write-Host "Shard Nodes: $nodeCount/4" -ForegroundColor $(if ($nodeCount -eq 4) { 'Green' } elseif ($nodeCount -gt 0) { 'Yellow' } else { 'Red' })

if ($nodes) {
    Write-Host ""
    Write-Host "Node PIDs:" -ForegroundColor Gray
    $nodes | ForEach-Object { Write-Host "  - PID $($_.Id)" -ForegroundColor Gray }
}

Write-Host ""

# Phase 2: Network Connections
Write-Host "[PHASE 2] Network Connections" -ForegroundColor Yellow
Write-Host "------------------------------------------------" -ForegroundColor Gray

$bootstrapConnections = netstat -an | Select-String "51820" | Select-String "ESTABLISHED"
$connectionCount = ($bootstrapConnections | Measure-Object).Count

Write-Host "Connections to Bootstrap (port 51820): $connectionCount" -ForegroundColor $(if ($connectionCount -ge 4) { 'Green' } elseif ($connectionCount -gt 0) { 'Yellow' } else { 'Red' })

if ($connectionCount -gt 0) {
    Write-Host ""
    Write-Host "Connection Details:" -ForegroundColor Gray
    $bootstrapConnections | Select-Object -First 10 | ForEach-Object {
        Write-Host "  $_" -ForegroundColor Gray
    }
}

Write-Host ""

# Phase 3: Web Server Status Check
Write-Host "[PHASE 3] Web Server Pipeline Status" -ForegroundColor Yellow
Write-Host "------------------------------------------------" -ForegroundColor Gray

if ($web) {
    try {
        # Try to get pipeline status via HTTP (if there's an API endpoint)
        # For now, we'll check WebSocket connection
        $tcpClient = New-Object System.Net.Sockets.TcpClient
        $tcpClient.Connect("localhost", 8081)
        $wsConnected = $tcpClient.Connected
        $tcpClient.Close()
        
        Write-Host "WebSocket Server (8081): $(if ($wsConnected) { "ACCESSIBLE" } else { "NOT ACCESSIBLE" })" -ForegroundColor $(if ($wsConnected) { 'Green' } else { 'Red' })
        
        # Try HTTP endpoint
        try {
            $response = Invoke-WebRequest -Uri "http://localhost:8080" -TimeoutSec 3 -UseBasicParsing -ErrorAction Stop
            Write-Host "HTTP Server (8080): ACCESSIBLE (Status: $($response.StatusCode))" -ForegroundColor Green
        } catch {
            Write-Host "HTTP Server (8080): NOT ACCESSIBLE" -ForegroundColor Red
        }
    } catch {
        Write-Host "WebSocket Server (8081): NOT ACCESSIBLE" -ForegroundColor Red
    }
} else {
    Write-Host "Web server not running - cannot check status" -ForegroundColor Yellow
}

Write-Host ""

# Phase 4: DHT Discovery Checklist
Write-Host "[PHASE 4] DHT Discovery Checklist" -ForegroundColor Yellow
Write-Host "------------------------------------------------" -ForegroundColor Gray
Write-Host ""
Write-Host "To diagnose DHT discovery, check the following in console windows:" -ForegroundColor White
Write-Host ""

Write-Host "NODE CONSOLE WINDOWS (shard_listener):" -ForegroundColor Cyan
Write-Host "  Look for these messages:" -ForegroundColor White
Write-Host "    [DHT] Started Kademlia bootstrap" -ForegroundColor Gray
Write-Host "    [DHT] Routing updated: {peer_id}" -ForegroundColor Gray
Write-Host "    [DHT] ANNOUNCED SHARD X TO DHT" -ForegroundColor Green
Write-Host ""
Write-Host "  If you see:" -ForegroundColor White
Write-Host "    [DHT] Failed to announce shard" -ForegroundColor Red
Write-Host "    -> Node failed to put record in DHT" -ForegroundColor Yellow
Write-Host ""

Write-Host "WEB SERVER CONSOLE WINDOW:" -ForegroundColor Cyan
Write-Host "  Look for these messages:" -ForegroundColor White
Write-Host "    [DHT] Querying for 4 shards..." -ForegroundColor Gray
Write-Host "    [DHT] Discovered shard X from {peer_id}" -ForegroundColor Green
Write-Host "    [STATUS] Pipeline: X/4 shards online" -ForegroundColor Gray
Write-Host ""
Write-Host "  If you see:" -ForegroundColor White
Write-Host "    [DHT] Re-querying shards..." -ForegroundColor Yellow
Write-Host "    -> Coordinator is querying but not finding records" -ForegroundColor Yellow
Write-Host "    [DHT] Failed to process DHT record" -ForegroundColor Red
Write-Host "    -> Records found but invalid/malformed" -ForegroundColor Yellow
Write-Host ""

Write-Host "BOOTSTRAP SERVER CONSOLE WINDOW:" -ForegroundColor Cyan
Write-Host "  Look for:" -ForegroundColor White
Write-Host "    ConnectionEstablished events" -ForegroundColor Gray
Write-Host "    RoutingUpdated events (good)" -ForegroundColor Green
Write-Host "    UnroutablePeer errors (bad - indicates routing issues)" -ForegroundColor Red
Write-Host ""

# Phase 5: Manual Verification Steps
Write-Host "[PHASE 5] Manual Verification Steps" -ForegroundColor Yellow
Write-Host "------------------------------------------------" -ForegroundColor Gray
Write-Host ""

Write-Host "Step 1: Check Node Announcements" -ForegroundColor White
Write-Host "  - Open each shard_listener console window" -ForegroundColor Gray
Write-Host "  - Look for: [DHT] ✓✓✓ ANNOUNCED SHARD X TO DHT ✓✓✓" -ForegroundColor Gray
Write-Host "  - Each node should show this message once" -ForegroundColor Gray
Write-Host ""

Write-Host "Step 2: Check Coordinator Queries" -ForegroundColor White
Write-Host "  - Open web_server console window" -ForegroundColor Gray
Write-Host "  - Look for: [DHT] Querying for 4 shards..." -ForegroundColor Gray
Write-Host "  - Should see this every 10 seconds" -ForegroundColor Gray
Write-Host ""

Write-Host "Step 3: Check Record Discovery" -ForegroundColor White
Write-Host "  - In web_server console, look for:" -ForegroundColor Gray
Write-Host "    [DHT] Discovered shard X from {peer_id}" -ForegroundColor Green
Write-Host "  - If missing, DHT routing is broken" -ForegroundColor Yellow
Write-Host ""

Write-Host "Step 4: Check Pipeline Status" -ForegroundColor White
Write-Host "  - In web_server console, look for:" -ForegroundColor Gray
Write-Host "    [STATUS] Pipeline: X/4 shards online" -ForegroundColor Gray
Write-Host "  - Should show 4/4 when all nodes discovered" -ForegroundColor Gray
Write-Host ""

# Phase 6: Common Issues and Solutions
Write-Host "[PHASE 6] Common Issues and Solutions" -ForegroundColor Yellow
Write-Host "------------------------------------------------" -ForegroundColor Gray
Write-Host ""

Write-Host "Issue 1: Nodes not announcing" -ForegroundColor Red
Write-Host "  Symptoms:" -ForegroundColor White
Write-Host "    - No [DHT] ANNOUNCED messages in node consoles" -ForegroundColor Gray
Write-Host "  Solutions:" -ForegroundColor White
Write-Host "    - Check if nodes received RoutingUpdated event" -ForegroundColor Gray
Write-Host "    - Verify nodes connected to bootstrap" -ForegroundColor Gray
Write-Host "    - Check for [DHT] Failed to announce errors" -ForegroundColor Gray
Write-Host ""

Write-Host "Issue 2: Coordinator not finding records" -ForegroundColor Red
Write-Host "  Symptoms:" -ForegroundColor White
Write-Host "    - [DHT] Re-querying shards... but no discoveries" -ForegroundColor Gray
Write-Host "    - Pipeline status shows 0/4 nodes" -ForegroundColor Gray
Write-Host "  Solutions:" -ForegroundColor White
Write-Host "    - Verify coordinator bootstrapped to DHT" -ForegroundColor Gray
Write-Host "    - Check if coordinator added bootstrap address to Kademlia" -ForegroundColor Gray
Write-Host "    - Verify nodes added their addresses to Kademlia" -ForegroundColor Gray
Write-Host "    - Check bootstrap console for UnroutablePeer errors" -ForegroundColor Gray
Write-Host ""

Write-Host "Issue 3: Records found but invalid" -ForegroundColor Red
Write-Host "  Symptoms:" -ForegroundColor White
Write-Host "    - [DHT] ⚠️  Failed to process DHT record" -ForegroundColor Gray
Write-Host "  Solutions:" -ForegroundColor White
Write-Host "    - Check record format/serialization" -ForegroundColor Gray
Write-Host "    - Verify cluster name matches" -ForegroundColor Gray
Write-Host ""

# Phase 7: Quick Test
Write-Host "[PHASE 7] Quick Test" -ForegroundColor Yellow
Write-Host "------------------------------------------------" -ForegroundColor Gray
Write-Host ""

Write-Host "Testing WebSocket connection and pipeline status..." -ForegroundColor White

try {
    Add-Type -AssemblyName System.Net.WebSockets
    Add-Type -AssemblyName System.Threading
    
    $uri = New-Object System.Uri("ws://localhost:8081")
    $client = New-Object System.Net.WebSockets.ClientWebSocket
    $cancellationToken = New-Object System.Threading.CancellationToken
    
    $connectTask = $client.ConnectAsync($uri, $cancellationToken)
    $connectTask.Wait(5000)
    
    if ($client.State -eq [System.Net.WebSockets.WebSocketState]::Open) {
        Write-Host "  [SUCCESS] Connected to WebSocket" -ForegroundColor Green
        
        # Wait for a pipeline status message
        $receiveBuffer = New-Object byte[] 4096
        $receiveSegment = New-Object System.ArraySegment[byte]($receiveBuffer, 0, $receiveBuffer.Length)
        
        $timeout = 5000 # 5 seconds
        $startTime = Get-Date
        
        while (((Get-Date) - $startTime).TotalMilliseconds -lt $timeout) {
            try {
                $receiveTask = $client.ReceiveAsync($receiveSegment, $cancellationToken)
                $receiveTask.Wait(1000)
                
                if ($receiveTask.IsCompleted -and -not $receiveTask.IsFaulted) {
                    $result = $receiveTask.Result
                    if ($result.MessageType -eq [System.Net.WebSockets.WebSocketMessageType]::Text) {
                        $responseText = [System.Text.Encoding]::UTF8.GetString($receiveBuffer, 0, $result.Count)
                        try {
                            $obj = $responseText | ConvertFrom-Json
                            if ($obj.message_type -eq "pipeline_status") {
                                Write-Host ""
                                Write-Host "  Current Pipeline Status:" -ForegroundColor Cyan
                                if ($obj.online_nodes -eq 4) {
                                    Write-Host "    Online Nodes: $($obj.online_nodes)/$($obj.total_nodes)" -ForegroundColor Green
                                } elseif ($obj.online_nodes -gt 0) {
                                    Write-Host "    Online Nodes: $($obj.online_nodes)/$($obj.total_nodes)" -ForegroundColor Yellow
                                } else {
                                    Write-Host "    Online Nodes: $($obj.online_nodes)/$($obj.total_nodes)" -ForegroundColor Red
                                }
                                $missingShardsStr = $obj.missing_shards -join ", "
                                if ($obj.missing_shards.Count -eq 0) {
                                    Write-Host "    Missing Shards: $missingShardsStr" -ForegroundColor Green
                                } else {
                                    Write-Host "    Missing Shards: $missingShardsStr" -ForegroundColor Yellow
                                }
                                if ($obj.is_complete) {
                                    Write-Host "    Pipeline Complete: $($obj.is_complete)" -ForegroundColor Green
                                } else {
                                    Write-Host "    Pipeline Complete: $($obj.is_complete)" -ForegroundColor Yellow
                                }
                                break
                            }
                        } catch {
                            # Not JSON, continue
                        }
                    }
                }
            } catch {
                # Timeout, continue
            }
        }
        
        $client.CloseAsync([System.Net.WebSockets.WebSocketCloseStatus]::NormalClosure, "Done", $cancellationToken).Wait(2000)
        $client.Dispose()
    } else {
        Write-Host "  [FAILED] Could not connect to WebSocket" -ForegroundColor Red
    }
} catch {
    Write-Host "  [ERROR] $($_.Exception.Message)" -ForegroundColor Red
}

Write-Host ""
Write-Host "================================================================" -ForegroundColor Cyan
Write-Host "  DIAGNOSTIC COMPLETE" -ForegroundColor Cyan
Write-Host "================================================================" -ForegroundColor Cyan
Write-Host ""
Write-Host "Next Steps:" -ForegroundColor Yellow
Write-Host "  1. Review console windows for the messages listed above" -ForegroundColor White
Write-Host "  2. Check if nodes are announcing to DHT" -ForegroundColor White
Write-Host "  3. Check if coordinator is finding records" -ForegroundColor White
Write-Host "  4. If issues persist, restart all processes" -ForegroundColor White
Write-Host ""

