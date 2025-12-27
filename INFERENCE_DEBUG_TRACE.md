# Line-by-Line Inference Flow Debug Trace

## Flow Overview

1. **Web UI** → WebSocket message with query
2. **Web Server** → `process_query()` → Creates `InferenceRequest`
3. **Web Server** → `coordinator.submit_inference()` 
4. **Pipeline Coordinator** → Checks pipeline status → Processes inference
5. **Pipeline Coordinator** → Sends `EXECUTE_TASK` commands to shard nodes
6. **Shard Nodes** → Process inference → Return `CommandResponse`
7. **Web Server** → Receives response → Returns to WebSocket
8. **Web UI** → Displays result

---

## Critical Points to Check

### Point 1: WebSocket Message Reception
**File**: `src/bin/web_server.rs`
**Line**: 1915-1922
```rust
let request: QueryRequest = match serde_json::from_str(&text) {
    Ok(r) => r,
    Err(_) => QueryRequest { query: text, request_id: None },
};

// Process query
println!("[WS] Processing query: {}", request.query);
let mut response = engine.process_query(&request.query, Some(&update_tx)).await;
```
**Check**: Is the query being received? Check logs for `[WS] Processing query:`

### Point 2: Pipeline Status Check
**File**: `src/bin/web_server.rs`
**Line**: 1296-1305
```rust
let (online_nodes, total_nodes, missing_shards, is_complete) = self.coordinator.get_pipeline_status().await;
println!("[INFERENCE] Pipeline status: {}/{} nodes online, complete: {}, missing: {:?}", 
         online_nodes, total_nodes, is_complete, missing_shards);

if online_nodes == 0 {
    eprintln!("[INFERENCE] ⚠️  No nodes online! Cannot process inference.");
```
**Check**: Are nodes online? Check logs for pipeline status

### Point 3: Coordinator Submit
**File**: `src/bin/web_server.rs`
**Line**: 1307
```rust
let result = self.coordinator.submit_inference(inference_request).await;
```
**Check**: Does this succeed or fail? Check error logs

### Point 4: Coordinator Pipeline Check
**File**: `src/pipeline_coordinator.rs`
**Line**: 1549-1555
```rust
if status.is_complete {
    // Pipeline ready - process immediately
    println!("[COORDINATOR] Pipeline is complete, processing inference immediately");
    return self.process_inference(request, start).await;
} else {
    println!("[COORDINATOR] Pipeline incomplete (missing: {:?}), applying strategy: {:?}", missing_shards, self.strategy);
}
```
**Check**: Is pipeline complete? If not, what strategy is applied?

### Point 5: Pipeline Empty Check
**File**: `src/pipeline_coordinator.rs`
**Line**: 2027-2045
```rust
let pipeline_clone: Vec<ShardAnnouncement> = {
    let discovery = self.discovery.read().await;
    let pipeline = discovery.get_pipeline();
    let cloned: Vec<ShardAnnouncement> = pipeline.iter().map(|s| (*s).clone()).collect();
    if cloned.is_empty() {
        drop(discovery);
        eprintln!("[INFERENCE] ❌ Pipeline is empty - cannot process inference");
        return Err(PipelineError::Internal {
            message: "Pipeline is empty - no shards available for processing".to_string(),
        });
    }
    cloned
};
```
**Check**: Is pipeline empty? This would cause failure

### Point 6: Command Sender Check
**File**: `src/pipeline_coordinator.rs`
**Line**: 2083
```rust
let shard_output = if let Some(ref sender) = self.command_sender {
```
**Check**: Is command_sender set? If None, it falls back to simulation

### Point 7: Command Sending
**File**: `src/pipeline_coordinator.rs`
**Line**: 2111
```rust
match sender(shard.peer_id.clone(), cmd).await {
```
**Check**: Does this succeed? Check for timeout or connection errors

### Point 8: Web Server Command Sender
**File**: `src/bin/web_server.rs`
**Line**: 674
```rust
let request_id = swarm.behaviour_mut().request_response.send_request(&target_peer, msg);
```
**Check**: Is the request being sent? Check logs for `[P2P] 📤 Sending command`

### Point 9: Response Matching
**File**: `src/bin/web_server.rs`
**Line**: 1021-1033
```rust
// Convert RequestId to u64 to match storage
use std::hash::{Hash, Hasher};
let mut hasher = std::collections::hash_map::DefaultHasher::new();
format!("{:?}", request_id).hash(&mut hasher);
let request_id_u64 = hasher.finish();

let mut pending = pending_for_events.lock().await;
if let Some(tx) = pending.remove(&request_id_u64) {
    println!("[P2P] ✓ Sending response to waiting channel");
    let _ = tx.send(cmd_response);
} else {
    println!("[P2P] ⚠️  No waiting channel found for request_id {:?}", request_id);
}
```
**Check**: Is the response being matched? If not, channel won't receive response

### Point 10: Shard Node Response
**File**: `src/shard_listener.rs`
**Line**: 1334-1341
```rust
if let Err(e) = swarm.behaviour_mut().request_response.send_response(
    channel,
    response_msg,
) {
    eprintln!("[RESPONSE] ❌ Failed to send response: {:?}", e);
} else {
    println!("[RESPONSE] ✓ Response sent successfully");
}
```
**Check**: Is response being sent? Check logs for `[RESPONSE] ✓ Response sent successfully`

---

## Potential Issues Found

### Issue 1: Response ID Mismatch
**Location**: `src/bin/web_server.rs` lines 672-680 vs 1021-1025
**Problem**: Using hash of RequestId to match, but hash might not match exactly
**Fix Needed**: Use a more reliable matching mechanism

### Issue 2: Command Sender Not Set
**Location**: `src/pipeline_coordinator.rs` line 2083
**Problem**: If `command_sender` is None, it simulates instead of real inference
**Check**: Verify `coordinator.with_command_sender()` is called

### Issue 3: Pipeline Empty
**Location**: `src/pipeline_coordinator.rs` line 2031
**Problem**: If pipeline is empty, inference fails immediately
**Check**: Verify DHT discovery is finding shard nodes

### Issue 4: Response Not Matched
**Location**: `src/bin/web_server.rs` line 1032
**Problem**: Response might not match pending request
**Check**: Verify request_id hashing matches

---

## Debugging Steps

1. Check if query is received: Look for `[WS] Processing query:`
2. Check pipeline status: Look for `[INFERENCE] Pipeline status:`
3. Check if coordinator processes: Look for `[COORDINATOR] Pipeline is complete` or `[COORDINATOR] Pipeline incomplete`
4. Check if commands are sent: Look for `[INFERENCE] 📤 Sending JSON command to node`
5. Check if responses are received: Look for `[P2P] 📥 Received response`
6. Check if response is matched: Look for `[P2P] ✓ Sending response to waiting channel` or `[P2P] ⚠️  No waiting channel found`
7. Check shard node logs: Look for `[RESPONSE] ✓ Response sent successfully`

