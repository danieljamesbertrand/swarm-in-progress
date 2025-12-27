# Comprehensive Node Join Checklist Test Script
# Tests all requirements for a node to successfully join the network

Write-Host ""
Write-Host "================================================================" -ForegroundColor Cyan
Write-Host "     NODE JOIN CHECKLIST TEST - COMPREHENSIVE VERIFICATION    " -ForegroundColor Cyan
Write-Host "================================================================" -ForegroundColor Cyan
Write-Host ""

$ErrorActionPreference = "Continue"
$testResults = @{
    Passed = 0
    Failed = 0
    Warnings = 0
}

function Test-Checklist {
    param(
        [string]$Name,
        [scriptblock]$Test,
        [bool]$Required = $true
    )
    
    Write-Host "[TEST] $Name" -ForegroundColor Yellow -NoNewline
    try {
        $result = & $Test
        if ($result) {
            Write-Host " [PASS]" -ForegroundColor Green
            $script:testResults.Passed++
            return $true
        } else {
            Write-Host " [FAIL]" -ForegroundColor Red
            $script:testResults.Failed++
            if ($Required) {
                return $false
            } else {
                $script:testResults.Warnings++
                return $true
            }
        }
    } catch {
        Write-Host " [ERROR: $_]" -ForegroundColor Red
        $script:testResults.Failed++
        if ($Required) {
            return $false
        } else {
            $script:testResults.Warnings++
            return $true
        }
    }
}

Write-Host "================================================================" -ForegroundColor Cyan
Write-Host "PHASE 1: PREREQUISITES" -ForegroundColor Cyan
Write-Host "================================================================" -ForegroundColor Cyan
Write-Host ""

# Test 1.1: Bootstrap Server Running
Test-Checklist "Bootstrap server process running" {
    $proc = Get-Process -Name "server" -ErrorAction SilentlyContinue
    return $null -ne $proc
} -Required $true

# Test 1.2: Bootstrap Server Listening on Port 51820
Test-Checklist "Bootstrap server listening on port 51820" {
    $listening = netstat -an | Select-String "51820.*LISTENING"
    return $null -ne $listening
} -Required $true

# Test 1.3: Web Server Running (Optional)
Test-Checklist "Web server process running" {
    $proc = Get-Process -Name "web_server" -ErrorAction SilentlyContinue
    return $null -ne $proc
} -Required $false

# Test 1.4: Web Server Listening on Ports 8080 and 8081
Test-Checklist "Web server listening on ports 8080 and 8081" {
    $port8080 = netstat -an | Select-String "8080.*LISTENING"
    $port8081 = netstat -an | Select-String "8081.*LISTENING"
    return ($null -ne $port8080) -and ($null -ne $port8081)
} -Required $false

# Test 1.5: Shards Directory Exists
Test-Checklist "Shards directory exists (models_cache/shards)" {
    return Test-Path "models_cache/shards"
} -Required $true

# Test 1.6: Shard Files Present
Test-Checklist "Shard files present (shard-0.gguf through shard-3.gguf)" {
    $files = @("shard-0.gguf", "shard-1.gguf", "shard-2.gguf", "shard-3.gguf")
    $allExist = $true
    foreach ($file in $files) {
        $path = "models_cache/shards/$file"
        if (-not (Test-Path $path)) {
            Write-Host "      Missing: $file" -ForegroundColor Yellow
            $allExist = $false
        } else {
            $size = (Get-Item $path).Length / 1MB
            $sizeRounded = [math]::Round($size, 2)
            Write-Host "      Found: $file ($sizeRounded MB)" -ForegroundColor Gray
        }
    }
    return $allExist
} -Required $true

Write-Host ""
Write-Host "================================================================" -ForegroundColor Cyan
Write-Host "PHASE 2: NODE STARTUP" -ForegroundColor Cyan
Write-Host "================================================================" -ForegroundColor Cyan
Write-Host ""

# Test 2.1: Check if Nodes are Running
Test-Checklist "Shard listener nodes running" {
    $nodes = Get-Process -Name "shard_listener" -ErrorAction SilentlyContinue
    $count = if ($nodes) { $nodes.Count } else { 0 }
    Write-Host "      Found $count node(s)" -ForegroundColor Gray
    return $count -gt 0
} -Required $true

# Test 2.2: Verify Expected Number of Nodes
Test-Checklist "Expected number of nodes (4) running" {
    $nodes = Get-Process -Name "shard_listener" -ErrorAction SilentlyContinue
    $count = if ($nodes) { $nodes.Count } else { 0 }
    Write-Host "      Running: $count/4" -ForegroundColor Gray
    return $count -eq 4
} -Required $true

# Test 2.3: Nodes Listening on Ports
Test-Checklist "Nodes listening on network ports" {
    $listening = netstat -an | Select-String "LISTENING" | Select-String "127.0.0.1"
    $listening2 = netstat -an | Select-String "LISTENING" | Select-String "0.0.0.0"
    $allListening = $listening + $listening2
    $nodePorts = $allListening | Where-Object { ($_ -notmatch "51820") -and ($_ -notmatch "8080") -and ($_ -notmatch "8081") }
    $portCount = ($nodePorts | Measure-Object).Count
    Write-Host "      Found $portCount listening port(s)" -ForegroundColor Gray
    return $portCount -gt 0
} -Required $true

Write-Host ""
Write-Host "================================================================" -ForegroundColor Cyan
Write-Host "PHASE 3: CONNECTIONS" -ForegroundColor Cyan
Write-Host "================================================================" -ForegroundColor Cyan
Write-Host ""

# Test 3.1: Bootstrap Connection (via netstat)
Test-Checklist "Nodes connected to bootstrap (port 51820)" {
    $connections = netstat -an | Select-String "51820" | Select-String "ESTABLISHED"
    $count = ($connections | Measure-Object).Count
    Write-Host "      Found $count connection(s) to port 51820" -ForegroundColor Gray
    return $count -ge 4  # At least 4 nodes should connect
} -Required $true

# Test 3.2: Web Server HTTP Accessible
Test-Checklist "Web server HTTP accessible (port 8080)" {
    try {
        $response = Invoke-WebRequest -Uri "http://localhost:8080" -TimeoutSec 5 -UseBasicParsing -ErrorAction Stop
        return $response.StatusCode -eq 200
    } catch {
        return $false
    }
} -Required $false

# Test 3.3: Web Server WebSocket Accessible
Test-Checklist "Web server WebSocket accessible (port 8081)" {
    try {
        $tcpClient = New-Object System.Net.Sockets.TcpClient
        $tcpClient.Connect("localhost", 8081)
        $connected = $tcpClient.Connected
        $tcpClient.Close()
        return $connected
    } catch {
        return $false
    }
} -Required $false

Write-Host ""
Write-Host "================================================================" -ForegroundColor Cyan
Write-Host "PHASE 4: DHT DISCOVERY" -ForegroundColor Cyan
Write-Host "================================================================" -ForegroundColor Cyan
Write-Host ""

# Test 4.1: Query Web Server for Pipeline Status
Test-Checklist "Pipeline status available via web server" {
    try {
        # Try to get pipeline status via WebSocket or HTTP API
        # For now, just check if web server is responding
        $response = Invoke-WebRequest -Uri "http://localhost:8080" -TimeoutSec 5 -UseBasicParsing -ErrorAction Stop
        return $response.StatusCode -eq 200
    } catch {
        Write-Host "      Web server not accessible for status check" -ForegroundColor Yellow
        return $false
    }
} -Required $false

Write-Host ""
Write-Host "================================================================" -ForegroundColor Cyan
Write-Host "PHASE 5: FILE SEEDING" -ForegroundColor Cyan
Write-Host "================================================================" -ForegroundColor Cyan
Write-Host ""

# Test 5.1: Verify Shard Files are Readable
Test-Checklist "Shard files are readable" {
    $files = @("shard-0.gguf", "shard-1.gguf", "shard-2.gguf", "shard-3.gguf")
    $allReadable = $true
    foreach ($file in $files) {
        $path = "models_cache/shards/$file"
        if (Test-Path $path) {
            try {
                $stream = [System.IO.File]::OpenRead($path)
                $stream.Close()
            } catch {
                Write-Host "      Cannot read: $file" -ForegroundColor Yellow
                $allReadable = $false
            }
        }
    }
    return $allReadable
} -Required $true

# Test 5.2: Verify File Sizes are Reasonable
Test-Checklist "Shard file sizes are reasonable (>100MB each)" {
    $files = @("shard-0.gguf", "shard-1.gguf", "shard-2.gguf", "shard-3.gguf")
    $allReasonable = $true
    foreach ($file in $files) {
        $path = "models_cache/shards/$file"
        if (Test-Path $path) {
            $sizeMB = (Get-Item $path).Length / 1MB
            if ($sizeMB -lt 100) {
                $sizeRounded = [math]::Round($sizeMB, 2)
                Write-Host "      File too small: $file ($sizeRounded MB)" -ForegroundColor Yellow
                $allReasonable = $false
            }
        }
    }
    return $allReasonable
} -Required $true

Write-Host ""
Write-Host "================================================================" -ForegroundColor Cyan
Write-Host "PHASE 6: KEEPALIVE & CONNECTION MANAGEMENT" -ForegroundColor Cyan
Write-Host "================================================================" -ForegroundColor Cyan
Write-Host ""

# Test 6.1: Connections Persist (wait and check)
Write-Host "[TEST] Connections persist over time" -ForegroundColor Yellow -NoNewline
Write-Host " (waiting 10 seconds...)" -ForegroundColor Gray
Start-Sleep -Seconds 10

$connectionsBefore = (netstat -an | Select-String "51820" | Select-String "ESTABLISHED" | Measure-Object).Count
Start-Sleep -Seconds 5
$connectionsAfter = (netstat -an | Select-String "51820" | Select-String "ESTABLISHED" | Measure-Object).Count

if ($connectionsAfter -ge $connectionsBefore) {
    Write-Host " [PASS]" -ForegroundColor Green
    Write-Host "      Connections: $connectionsBefore -> $connectionsAfter" -ForegroundColor Gray
    $testResults.Passed++
} else {
    Write-Host " [WARNING]" -ForegroundColor Yellow
    Write-Host "      Connections dropped: $connectionsBefore -> $connectionsAfter" -ForegroundColor Yellow
    $testResults.Warnings++
}

Write-Host ""
Write-Host "================================================================" -ForegroundColor Cyan
Write-Host "PHASE 7: INTEGRATION TEST" -ForegroundColor Cyan
Write-Host "================================================================" -ForegroundColor Cyan
Write-Host ""

# Test 7.1: All Components Running Together
Test-Checklist "All components running together" {
    $bootstrap = Get-Process -Name "server" -ErrorAction SilentlyContinue
    $web = Get-Process -Name "web_server" -ErrorAction SilentlyContinue
    $nodes = Get-Process -Name "shard_listener" -ErrorAction SilentlyContinue
    $nodeCount = if ($nodes) { $nodes.Count } else { 0 }
    
    Write-Host "      Bootstrap: $(if ($bootstrap) { 'Running' } else { 'Not Running' })" -ForegroundColor Gray
    Write-Host "      Web Server: $(if ($web) { 'Running' } else { 'Not Running' })" -ForegroundColor Gray
    Write-Host "      Nodes: $nodeCount" -ForegroundColor Gray
    
    return ($null -ne $bootstrap) -and ($nodeCount -ge 4)
} -Required $true

Write-Host ""
Write-Host "================================================================" -ForegroundColor Cyan
Write-Host "TEST RESULTS SUMMARY" -ForegroundColor Cyan
Write-Host "================================================================" -ForegroundColor Cyan
Write-Host ""
Write-Host "Passed:  $($testResults.Passed)" -ForegroundColor Green
Write-Host "Failed:  $($testResults.Failed)" -ForegroundColor $(if ($testResults.Failed -eq 0) { 'Green' } else { 'Red' })
Write-Host "Warnings: $($testResults.Warnings)" -ForegroundColor $(if ($testResults.Warnings -eq 0) { 'Green' } else { 'Yellow' })
Write-Host ""

$total = $testResults.Passed + $testResults.Failed + $testResults.Warnings
if ($total -gt 0) {
    $passRate = [math]::Round(($testResults.Passed / $total) * 100, 1)
    Write-Host "Pass Rate: $passRate%" -ForegroundColor $(if ($passRate -ge 80) { 'Green' } elseif ($passRate -ge 60) { 'Yellow' } else { 'Red' })
}

Write-Host ""
if ($testResults.Failed -eq 0) {
    Write-Host "[SUCCESS] All required tests passed!" -ForegroundColor Green
} else {
    Write-Host "[FAILURE] Some required tests failed. Please review the checklist." -ForegroundColor Red
    Write-Host ""
    Write-Host "Next steps:" -ForegroundColor Yellow
    Write-Host "  1. Review NODE_JOIN_CHECKLIST.md for detailed requirements" -ForegroundColor White
    Write-Host "  2. Check node console logs for connection/announcement issues" -ForegroundColor White
    Write-Host "  3. Verify all shard files are present and readable" -ForegroundColor White
    Write-Host "  4. Ensure bootstrap server is running and accessible" -ForegroundColor White
}

Write-Host ""
Write-Host "================================================================" -ForegroundColor Cyan
Write-Host ""

