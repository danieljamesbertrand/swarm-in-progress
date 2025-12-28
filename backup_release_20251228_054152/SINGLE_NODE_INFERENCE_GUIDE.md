# Single Node Inference Test Guide

## Quick Test: "what do a cat and a snake have in common"

### What's Running

The test script (`test_single_node_inference.ps1`) starts:
1. **Bootstrap Server** - DHT bootstrap node on port 51820
2. **Single Shard Node** - Shard 0 node for inference
3. **Web Server** - Web interface on http://localhost:8080

### How to Test

1. **Wait for startup** (10-15 seconds):
   - Bootstrap server should start first
   - Shard node should connect and register
   - Web server should be ready

2. **Open browser**: http://localhost:8080

3. **Wait for node registration** (5-10 seconds):
   - You should see the shard node appear in the pipeline status
   - The node should show as "online"

4. **Submit the question**:
   - Type: `what do a cat and a snake have in common`
   - Click "Send" or press Enter

5. **Watch for results**:
   - The inference request will be sent to the shard node
   - The node will process it (may take a few seconds)
   - Results will appear in the response area

### What to Look For

**In the Web Console:**
- Pipeline status showing 1/4 nodes online
- Shard 0 button should be visible
- Query input field at the top
- Response area below

**In Terminal Windows:**
- **Bootstrap terminal**: Should show connection from shard node
- **Shard node terminal**: Should show:
  - `[INFERENCE]` messages when processing
  - `[RESPONSE]` messages when sending results
- **Web server terminal**: Should show:
  - `[INFERENCE]` messages for coordinator activity
  - `[P2P]` messages for communication
  - `[P2P] ✓ Matched response` when response is received

### Expected Behavior

1. **Query sent**: Web server receives query via WebSocket
2. **Coordinator processes**: Pipeline coordinator checks pipeline status
3. **Command sent**: Coordinator sends `EXECUTE_TASK` command to shard node
4. **Node processes**: Shard node runs inference on the model
5. **Response returned**: Node sends response back via P2P
6. **Results displayed**: Web server receives response and displays it

### Troubleshooting

**If no response appears:**
1. Check shard node terminal for `[INFERENCE]` messages
2. Check web server terminal for `[P2P]` messages
3. Look for `[P2P] ✓ Matched response` - this confirms response matching worked
4. Check browser console (F12) for WebSocket errors

**If node doesn't appear:**
1. Check bootstrap terminal - node should connect
2. Check shard node terminal - should show DHT bootstrap success
3. Wait 10-20 seconds for DHT discovery to complete

**If inference fails:**
1. Check if shard-0.gguf exists in `models_cache/shards/`
2. Check shard node terminal for model loading errors
3. Check web server terminal for command sending errors

### Log Messages to Watch For

**Success indicators:**
- `[P2P] 📤 Sending command EXECUTE_TASK to node`
- `[INFERENCE] ✅ command_sender is set`
- `[P2P] 📥 Received response`
- `[P2P] ✓ Matched response to waiting channel`
- `[INFERENCE] ✓ Shard 0 completed`

**Error indicators:**
- `[P2P] ⚠️  No waiting channel found` - Response matching failed
- `[INFERENCE] ❌ WARNING: No command sender` - Command sender not configured
- `[INFERENCE] ❌ Pipeline is empty` - No nodes discovered
- `[P2P] ❌ Timeout waiting for response` - Node didn't respond

### Next Steps

Once single node inference works, you can:
1. Add more shard nodes (shard 1, 2, 3) for full pipeline
2. Test with different questions
3. Monitor performance and latency
4. Check distributed processing across multiple nodes

