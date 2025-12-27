# Comprehensive Testing Checklist

## System Status ✅

### Prerequisites
- [ ] Bootstrap server running on port 51820
- [ ] Web server running on port 8080
- [ ] WebSocket server running on port 8081
- [ ] All 4 shard nodes spawned and running

## Test 1: Coordinated Shard Assignment ✅

### Verification Steps
1. [ ] Check web server console for coordinated assignment messages:
   - `[COORDINATOR] Last assigned shard: X`
   - `[COORDINATOR] Coordinated assignment: spawning node for shard Y`
   - `[COORDINATOR] Next after last assigned`

2. [ ] Verify sequential assignment:
   - First node gets shard 0
   - Second node gets shard 1
   - Third node gets shard 2
   - Fourth node gets shard 3

3. [ ] Check no conflicts:
   - No duplicate shard assignments
   - All shards assigned correctly

## Test 2: Web Interface Connectivity ✅

### Browser Tests
1. [ ] Open http://localhost:8080
2. [ ] Verify page loads without errors
3. [ ] Check connection status shows "Connected" (green)
4. [ ] Verify "Nodes Online" shows 4/4
5. [ ] Check all pipeline stages are visible

### WebSocket Tests
1. [ ] Open browser console (F12)
2. [ ] Look for `[WS] ✓ Connected to Promethos backend`
3. [ ] Verify no connection errors
4. [ ] Check periodic status updates are received

## Test 3: Pipeline Status Updates ✅

### Status Verification
1. [ ] Check "Nodes Online" counter updates in real-time
2. [ ] Verify pipeline stages show correct status:
   - Input stage: ready
   - Discovery stage: ready
   - Shard 0-3 stages: ready (green/waiting)
   - Output stage: ready

3. [ ] Check metrics are updating:
   - Inference requests counter
   - Average latency
   - Success/failure counts

## Test 4: Inference Query Tests ✅

### Test Query 1: Simple Math
- **Query**: "What is 2+2?"
- **Expected Behavior**:
  - [ ] Pipeline stages activate in sequence
  - [ ] Input → Discovery → Shard0 → Shard1 → Shard2 → Shard3 → Output
  - [ ] Response appears in output area
  - [ ] Response contains "4" or similar answer

### Test Query 2: Knowledge Question
- **Query**: "Who wrote Bohemian Rhapsody?"
- **Expected Behavior**:
  - [ ] All stages process successfully
  - [ ] Real-time updates visible
  - [ ] Response mentions "Queen" or "Freddie Mercury"

### Test Query 3: Geography
- **Query**: "What is the capital of Japan?"
- **Expected Behavior**:
  - [ ] Pipeline completes successfully
  - [ ] Response mentions "Tokyo"

## Test 5: Real-Time Updates ✅

### Browser Console Verification
Check for these messages in order:
1. [ ] `[WS] Stage update: input -> processing`
2. [ ] `[WS] Stage update: discovery -> processing`
3. [ ] `[WS] Stage update: discovery -> complete`
4. [ ] `[WS] Stage update: shard0 -> processing`
5. [ ] `[WS] Stage update: shard0 -> complete`
6. [ ] `[WS] Stage update: shard1 -> processing`
7. [ ] `[WS] Stage update: shard1 -> complete`
8. [ ] `[WS] Stage update: shard2 -> processing`
9. [ ] `[WS] Stage update: shard2 -> complete`
10. [ ] `[WS] Stage update: shard3 -> processing`
11. [ ] `[WS] Stage update: shard3 -> complete`
12. [ ] `[WS] Stage update: output -> processing`
13. [ ] `[WS] Stage update: output -> complete`

### Visual Verification
1. [ ] Pipeline stages light up in sequence (green/active)
2. [ ] Stages turn complete (green/complete) after processing
3. [ ] Connectors between stages activate
4. [ ] No stages show error state (red)

## Test 6: Error Handling ✅

### Failure Scenarios
1. [ ] Test with one node offline:
   - Stop one shard_listener process
   - Verify system handles gracefully
   - Check error messages are clear

2. [ ] Test with web server restart:
   - Restart web server
   - Verify nodes reconnect
   - Check WebSocket reconnects automatically

3. [ ] Test with invalid query:
   - Send empty query
   - Verify error handling
   - Check error message displayed

## Test 7: Performance ✅

### Latency Tests
1. [ ] Measure time from query submission to response
2. [ ] Check average latency in metrics
3. [ ] Verify latency is reasonable (< 5 seconds for simple queries)

### Throughput Tests
1. [ ] Send multiple queries in sequence
2. [ ] Verify all complete successfully
3. [ ] Check no performance degradation

## Test 8: Shard Loading (Torrent) ✅

### Shard Availability
1. [ ] Check nodes scan for existing shards on startup
2. [ ] Verify shards are seeded via torrent
3. [ ] Check LOAD_SHARD command works (if shards missing)

### Download Verification
1. [ ] If shard missing, verify download starts
2. [ ] Check download progress (if visible)
3. [ ] Verify shard loads after download

## Test Results Summary

### Passed Tests: ___ / 8
### Failed Tests: ___ / 8
### Notes:
- 
- 
- 

## Next Steps
- [ ] Fix any failed tests
- [ ] Document any issues found
- [ ] Optimize performance if needed
- [ ] Add additional test cases

