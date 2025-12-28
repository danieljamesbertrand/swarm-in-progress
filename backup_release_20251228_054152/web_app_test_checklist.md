# Web App Functionality Test Checklist

## Pre-Test Setup
- [ ] Build project successfully (`cargo build --release`)
- [ ] No compilation errors
- [ ] All dependencies installed

## Server Tests
- [ ] Web server starts without errors
- [ ] Web server listens on port 8080 (HTTP)
- [ ] Web server listens on port 8081 (WebSocket)
- [ ] Web server responds to HTTP requests

## Node Tests
- [ ] 4 shard listener nodes start successfully
- [ ] Nodes connect to DHT bootstrap
- [ ] Nodes announce their shards to DHT
- [ ] Nodes are discoverable by web server

## Web Page Tests

### AI Console Page (`/ai-console.html`)
- [ ] Page loads successfully (HTTP 200)
- [ ] No 404 errors for static resources
- [ ] HTML structure is correct
- [ ] CSS styles load properly
- [ ] JavaScript loads without errors

### UI Elements Visibility
- [ ] Input textarea is visible
- [ ] Submit button is visible
- [ ] Scrolling log container is visible
- [ ] Pipeline tracker is visible
- [ ] Response area is visible
- [ ] Clear log button is visible

### WebSocket Connection
- [ ] WebSocket connects successfully
- [ ] Connection status shows "Connected"
- [ ] Initial pipeline status received
- [ ] Initial metrics received
- [ ] Connection remains stable

## Node Discovery Tests
- [ ] Nodes appear in pipeline status
- [ ] Online node count is correct (4/4)
- [ ] Missing shards list is empty when all nodes online
- [ ] Pipeline shows as "complete" when all shards available
- [ ] Node join events are logged

## Query Submission Tests
- [ ] Input field accepts text
- [ ] Submit button is clickable
- [ ] Query is sent via WebSocket
- [ ] Loading state is shown during processing
- [ ] Query is processed by coordinator

## Preload Messages Tests
- [ ] Preload messages appear before inference
- [ ] Each shard shows a preload message
- [ ] Preload messages show correct shard ID
- [ ] Preload messages are displayed in response area

## Scrolling Log Tests
- [ ] Node inference request messages appear in log
- [ ] Each message shows:
  - [ ] Node ID (shortened)
  - [ ] Shard ID
  - [ ] Layers being processed
  - [ ] Timestamp
  - [ ] Input preview
  - [ ] Request ID
- [ ] Log auto-scrolls to bottom
- [ ] Clear log button works
- [ ] Multiple requests stack in log

## Pipeline Stage Updates Tests
- [ ] Input stage activates
- [ ] Discovery stage activates
- [ ] Shard stages (0-3) activate in sequence
- [ ] Output stage activates
- [ ] Stages show correct status (waiting/processing/complete)
- [ ] Latency is displayed for completed stages

## Inference Processing Tests
- [ ] Query is distributed to nodes
- [ ] Each node receives inference request
- [ ] Nodes process their shard layers
- [ ] Output is collected from final shard
- [ ] Response is displayed in UI
- [ ] Response contains expected content

## Real-Time Updates Tests
- [ ] Pipeline status updates every 2 seconds
- [ ] Metrics update periodically
- [ ] Node status changes are reflected immediately
- [ ] No duplicate messages
- [ ] Updates don't cause UI flicker

## Error Handling Tests
- [ ] WebSocket reconnection works if connection drops
- [ ] Error messages are displayed clearly
- [ ] Invalid queries are handled gracefully
- [ ] Missing nodes show appropriate warnings
- [ ] Network errors don't crash the app

## Edge Cases
- [ ] Empty query submission
- [ ] Very long query submission
- [ ] Special characters in query
- [ ] Multiple rapid queries
- [ ] Query while nodes are still connecting
- [ ] Query when some nodes are offline

## Performance Tests
- [ ] Page loads in < 2 seconds
- [ ] WebSocket messages process quickly
- [ ] UI remains responsive during inference
- [ ] No memory leaks during extended use
- [ ] Scrolling log doesn't slow down with many entries

## Browser Compatibility
- [ ] Works in Chrome/Edge
- [ ] Works in Firefox
- [ ] Works in Safari (if applicable)
- [ ] Mobile responsive (if applicable)

## Console Logging Tests
- [ ] Browser console shows WebSocket connection logs
- [ ] Browser console shows message receive logs
- [ ] Browser console shows error logs (if any)
- [ ] Server logs show node discovery
- [ ] Server logs show inference requests
- [ ] Server logs show routing depth updates

## Kademlia Integration Tests
- [ ] Routing depth is calculated correctly
- [ ] Node selection uses routing depth for weighting
- [ ] Closer nodes are prioritized
- [ ] Queue ordering is respected
- [ ] Depth tree information is used in scoring

## Final Verification
- [ ] All tests pass
- [ ] No console errors
- [ ] No server errors
- [ ] System is stable
- [ ] Documentation matches behavior


