# Investigate Communication Breakdown
# Traces through the entire communication flow to find where it breaks

Write-Host ""
Write-Host "================================================================" -ForegroundColor Cyan
Write-Host "  COMMUNICATION BREAKDOWN INVESTIGATION" -ForegroundColor Cyan
Write-Host "================================================================" -ForegroundColor Cyan
Write-Host ""

$ErrorActionPreference = "Continue"

# Step 1: Check cluster name consistency
Write-Host "[STEP 1] Checking Cluster Name Configuration" -ForegroundColor Yellow
Write-Host "------------------------------------------------" -ForegroundColor Gray

Write-Host "Web Server (coordinator) uses: 'llama-cluster' (hardcoded)" -ForegroundColor Gray
Write-Host "Nodes use: From --cluster argument (default: 'llama-cluster')" -ForegroundColor Gray
Write-Host ""
Write-Host "CRITICAL: If cluster names don't match, DHT keys won't match!" -ForegroundColor Red
Write-Host "  Node announces with: /llama-cluster/{node_cluster}/shard/{id}" -ForegroundColor Gray
Write-Host "  Coordinator queries: /llama-cluster/llama-cluster/shard/{id}" -ForegroundColor Gray
Write-Host ""

# Step 2: Check DHT key format
Write-Host "[STEP 2] DHT Key Format Analysis" -ForegroundColor Yellow
Write-Host "------------------------------------------------" -ForegroundColor Gray

Write-Host "DHT Key Format: /llama-cluster/{cluster_name}/shard/{shard_id}" -ForegroundColor Gray
Write-Host ""
Write-Host "Expected keys for cluster 'llama-cluster':" -ForegroundColor White
Write-Host "  Shard 0: /llama-cluster/llama-cluster/shard/0" -ForegroundColor Gray
Write-Host "  Shard 1: /llama-cluster/llama-cluster/shard/1" -ForegroundColor Gray
Write-Host "  Shard 2: /llama-cluster/llama-cluster/shard/2" -ForegroundColor Gray
Write-Host "  Shard 3: /llama-cluster/llama-cluster/shard/3" -ForegroundColor Gray
Write-Host ""
Write-Host "If nodes use different cluster name, keys won't match!" -ForegroundColor Yellow
Write-Host ""

# Step 3: Communication Flow Analysis
Write-Host "[STEP 3] Communication Flow Analysis" -ForegroundColor Yellow
Write-Host "------------------------------------------------" -ForegroundColor Gray
Write-Host ""

Write-Host "NODE SIDE FLOW:" -ForegroundColor Cyan
Write-Host "  1. Node connects to bootstrap" -ForegroundColor Gray
Write-Host "     → Should see: [CONNECT] Connection established" -ForegroundColor Gray
Write-Host ""
Write-Host "  2. Node calls kademlia.bootstrap()" -ForegroundColor Gray
Write-Host "     → Should see: [DHT] Started Kademlia bootstrap" -ForegroundColor Gray
Write-Host "     → Prerequisite: bootstrap() must succeed" -ForegroundColor Yellow
Write-Host ""
Write-Host "  3. Node receives RoutingUpdated event" -ForegroundColor Gray
Write-Host "     → Should see: [DHT] Routing updated: {peer_id}" -ForegroundColor Gray
Write-Host "     → CRITICAL: Announcement only happens AFTER this event" -ForegroundColor Red
Write-Host ""
Write-Host "  4. Node creates announcement record" -ForegroundColor Gray
Write-Host "     → Calls: create_announcement_record()" -ForegroundColor Gray
Write-Host "     → Key: /llama-cluster/{cluster}/shard/{shard_id}" -ForegroundColor Gray
Write-Host ""
Write-Host "  5. Node calls put_record()" -ForegroundColor Gray
Write-Host "     → Should see: [DHT] ANNOUNCED SHARD X TO DHT" -ForegroundColor Green
Write-Host "     → OR: [DHT] Failed to announce shard: {error}" -ForegroundColor Red
Write-Host ""

Write-Host "COORDINATOR SIDE FLOW:" -ForegroundColor Cyan
Write-Host "  1. Coordinator connects to bootstrap" -ForegroundColor Gray
Write-Host "     → Should see: [DHT] Connecting to bootstrap" -ForegroundColor Gray
Write-Host ""
Write-Host "  2. Coordinator calls kademlia.bootstrap()" -ForegroundColor Gray
Write-Host "     → Should see: [DHT] Started Kademlia bootstrap" -ForegroundColor Gray
Write-Host "     → Sets bootstrapped = true" -ForegroundColor Gray
Write-Host ""
Write-Host "  3. Coordinator queries every 10 seconds" -ForegroundColor Gray
Write-Host "     → Should see: [DHT] Querying for 4 shards..." -ForegroundColor Gray
Write-Host "     → Queries: /llama-cluster/llama-cluster/shard/{0,1,2,3}" -ForegroundColor Gray
Write-Host ""
Write-Host "  4. Coordinator receives FoundRecord event" -ForegroundColor Gray
Write-Host "     → Should see: [DHT] Discovered shard X from {peer_id}" -ForegroundColor Green
Write-Host "     → OR: No FoundRecord events (queries not routing)" -ForegroundColor Red
Write-Host ""
Write-Host "  5. Coordinator processes record" -ForegroundColor Gray
Write-Host "     → Calls: process_dht_record()" -ForegroundColor Gray
Write-Host "     → Should see: [STATUS] Pipeline: X/4 shards online" -ForegroundColor Green
Write-Host ""

# Step 4: Potential Failure Points
Write-Host "[STEP 4] Potential Failure Points" -ForegroundColor Yellow
Write-Host "------------------------------------------------" -ForegroundColor Gray
Write-Host ""

Write-Host "FAILURE POINT 1: Nodes not receiving RoutingUpdated" -ForegroundColor Red
Write-Host "  Symptom: Nodes connect but never announce" -ForegroundColor White
Write-Host "  Cause: Kademlia bootstrap not completing" -ForegroundColor Gray
Write-Host "  Check: Bootstrap server console for routing issues" -ForegroundColor Yellow
Write-Host ""

Write-Host "FAILURE POINT 2: Cluster name mismatch" -ForegroundColor Red
Write-Host "  Symptom: Nodes announce but coordinator never finds them" -ForegroundColor White
Write-Host "  Cause: Different cluster names = different DHT keys" -ForegroundColor Gray
Write-Host "  Check: Node startup arguments for --cluster value" -ForegroundColor Yellow
Write-Host ""

Write-Host "FAILURE POINT 3: Coordinator not bootstrapping" -ForegroundColor Red
Write-Host "  Symptom: Coordinator never queries DHT" -ForegroundColor White
Write-Host "  Cause: bootstrapped flag never set to true" -ForegroundColor Gray
Write-Host "  Check: Web server console for bootstrap messages" -ForegroundColor Yellow
Write-Host ""

Write-Host "FAILURE POINT 4: DHT routing broken" -ForegroundColor Red
Write-Host "  Symptom: Coordinator queries but no FoundRecord events" -ForegroundColor White
Write-Host "  Cause: Routing table doesn't know about nodes" -ForegroundColor Gray
Write-Host "  Check: Bootstrap console for UnroutablePeer errors" -ForegroundColor Yellow
Write-Host ""

Write-Host "FAILURE POINT 5: Record processing fails" -ForegroundColor Red
Write-Host "  Symptom: FoundRecord received but not processed" -ForegroundColor White
Write-Host "  Cause: Record validation fails (stale, malformed, etc.)" -ForegroundColor Gray
Write-Host "  Check: Web server console for 'Failed to process DHT record'" -ForegroundColor Yellow
Write-Host ""

# Step 5: Diagnostic Commands
Write-Host "[STEP 5] Diagnostic Commands" -ForegroundColor Yellow
Write-Host "------------------------------------------------" -ForegroundColor Gray
Write-Host ""

Write-Host "To check node cluster names:" -ForegroundColor White
Write-Host "  Look in node console windows for startup arguments" -ForegroundColor Gray
Write-Host "  Or check process command line:" -ForegroundColor Gray
Write-Host "    Get-WmiObject Win32_Process | Where-Object {`$_.Name -eq 'shard_listener.exe'} | Select-Object CommandLine" -ForegroundColor Cyan
Write-Host ""

Write-Host "To check if nodes are announcing:" -ForegroundColor White
Write-Host "  Search node console for: 'ANNOUNCED SHARD'" -ForegroundColor Gray
Write-Host ""

Write-Host "To check if coordinator is querying:" -ForegroundColor White
Write-Host "  Search web server console for: 'Querying for 4 shards'" -ForegroundColor Gray
Write-Host ""

Write-Host "To check if coordinator is finding records:" -ForegroundColor White
Write-Host "  Search web server console for: 'Discovered shard'" -ForegroundColor Gray
Write-Host ""

# Step 6: Code Locations to Verify
Write-Host "[STEP 6] Code Locations to Verify" -ForegroundColor Yellow
Write-Host "------------------------------------------------" -ForegroundColor Gray
Write-Host ""

Write-Host "Node Announcement:" -ForegroundColor White
Write-Host "  File: src/shard_listener.rs" -ForegroundColor Gray
Write-Host "  Line 775: RoutingUpdated event handler" -ForegroundColor Gray
Write-Host "  Line 838: put_record() call" -ForegroundColor Gray
Write-Host "  Line 859: DHT key creation (uses cluster_name variable)" -ForegroundColor Gray
Write-Host ""

Write-Host "Coordinator Query:" -ForegroundColor White
Write-Host "  File: src/bin/web_server.rs" -ForegroundColor Gray
Write-Host "  Line 483: Discovery created with 'llama-cluster' (hardcoded)" -ForegroundColor Gray
Write-Host "  Line 1161: DHT key creation (uses 'llama-cluster' hardcoded)" -ForegroundColor Gray
Write-Host "  Line 1037: FoundRecord event handler" -ForegroundColor Gray
Write-Host ""

Write-Host "DHT Key Format:" -ForegroundColor White
Write-Host "  File: src/kademlia_shard_discovery.rs" -ForegroundColor Gray
Write-Host "  Line 320: shard_key() function" -ForegroundColor Gray
Write-Host "  Format: /llama-cluster/{cluster_name}/shard/{shard_id}" -ForegroundColor Gray
Write-Host ""

# Step 7: Most Likely Issue
Write-Host "[STEP 7] Most Likely Issue" -ForegroundColor Yellow
Write-Host "------------------------------------------------" -ForegroundColor Gray
Write-Host ""

Write-Host "Based on code analysis, the most likely issues are:" -ForegroundColor White
Write-Host ""
Write-Host "1. Nodes not receiving RoutingUpdated events" -ForegroundColor Red
Write-Host "   → Kademlia bootstrap may not be completing" -ForegroundColor Gray
Write-Host "   → Routing table may be empty" -ForegroundColor Gray
Write-Host "   → Nodes wait forever for RoutingUpdated before announcing" -ForegroundColor Gray
Write-Host ""
Write-Host "2. Coordinator queries not routing to nodes" -ForegroundColor Red
Write-Host "   → Coordinator's routing table doesn't know about nodes" -ForegroundColor Gray
Write-Host "   → Queries can't find nodes storing records" -ForegroundColor Gray
Write-Host "   → Even if records exist, queries fail to route" -ForegroundColor Gray
Write-Host ""
Write-Host "3. Cluster name mismatch (less likely but possible)" -ForegroundColor Yellow
Write-Host "   → If nodes started with different --cluster argument" -ForegroundColor Gray
Write-Host "   → Keys won't match and discovery fails" -ForegroundColor Gray
Write-Host ""

Write-Host "================================================================" -ForegroundColor Cyan
Write-Host "  INVESTIGATION COMPLETE" -ForegroundColor Cyan
Write-Host "================================================================" -ForegroundColor Cyan
Write-Host ""
Write-Host "Next Steps:" -ForegroundColor Yellow
Write-Host "  1. Check node console for RoutingUpdated events" -ForegroundColor White
Write-Host "  2. Check web server console for query/discovery messages" -ForegroundColor White
Write-Host "  3. Verify cluster names match between nodes and coordinator" -ForegroundColor White
Write-Host "  4. Check bootstrap console for routing errors" -ForegroundColor White
Write-Host ""

