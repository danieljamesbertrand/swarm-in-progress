# Test script for Promethos-AI Web Server
Write-Host '=== Promethos-AI Web Server Test ===' -ForegroundColor Cyan
Write-Host ''

# Test 1: HTTP Server
Write-Host '[1] Testing HTTP server on port 8080...' -ForegroundColor Yellow
try {
    $response = Invoke-WebRequest -Uri 'http://localhost:8080' -UseBasicParsing -TimeoutSec 5
    if ($response.StatusCode -eq 200) {
        Write-Host '  HTTP server responding (Status: 200)' -ForegroundColor Green
        if ($response.Content -match 'Promethos') {
            Write-Host '  Page contains Promethos content' -ForegroundColor Green
        }
    }
} catch {
    Write-Host "  HTTP server error: $_" -ForegroundColor Red
}

# Test 2: WebSocket Server
Write-Host '[2] Testing WebSocket server on port 8081...' -ForegroundColor Yellow
$wsListening = netstat -an | Select-String -Pattern '8081.*LISTENING'
if ($wsListening) {
    Write-Host '  WebSocket server is listening on port 8081' -ForegroundColor Green
} else {
    Write-Host '  WebSocket server not listening' -ForegroundColor Red
}

# Test 3: Check all pages
Write-Host '[3] Testing all web pages...' -ForegroundColor Yellow
$pages = @('/', '/ai-console.html', '/admin.html', '/index.html')
foreach ($page in $pages) {
    try {
        $response = Invoke-WebRequest -Uri "http://localhost:8080$page" -UseBasicParsing -TimeoutSec 3
        if ($response.StatusCode -eq 200) {
            Write-Host "  $page - OK" -ForegroundColor Green
        }
    } catch {
        Write-Host "  $page - Error" -ForegroundColor Red
    }
}

# Test 4: Check for clear buttons in HTML
Write-Host '[4] Checking for clear buttons in console...' -ForegroundColor Yellow
try {
    $content = (Invoke-WebRequest -Uri 'http://localhost:8080/ai-console.html' -UseBasicParsing).Content
    $clearButtons = @('clearLogBtn', 'clearResponseBtn', 'clearPipelineBtn')
    foreach ($btn in $clearButtons) {
        if ($content -match $btn) {
            Write-Host "  Found $btn" -ForegroundColor Green
        } else {
            Write-Host "  Missing $btn" -ForegroundColor Red
        }
    }
} catch {
    Write-Host '  Error checking buttons' -ForegroundColor Red
}

Write-Host ''
Write-Host '=== Test Complete ===' -ForegroundColor Cyan
Write-Host ''
Write-Host 'Manual Testing Instructions:' -ForegroundColor Yellow
Write-Host '1. Open http://localhost:8080 in your browser'
Write-Host '2. Test WebSocket connection (should auto-connect)'
Write-Host '3. Submit a query and verify response appears'
Write-Host '4. Test all clear buttons work correctly'
Write-Host '5. Verify live data streaming from nodes'
