# Using Cursor Browser for Inference Test

## Quick Start

1. **Open Cursor's Simple Browser:**
   - Press `Ctrl+Shift+P` (Windows/Linux) or `Cmd+Shift+P` (Mac)
   - Type: `Simple Browser: Show`
   - Press Enter

2. **Enter the URL:**
   - Type: `http://localhost:8080`
   - Press Enter

3. **Wait for Node Registration:**
   - Wait 10-15 seconds for the shard node to register
   - You should see pipeline status showing 1/4 nodes online

4. **Submit Inference Query:**
   - In the query input field, type: `what do a cat and a snake have in common`
   - Click "Send" or press Enter

5. **View Results:**
   - Results will appear in the response area below the input field
   - Check terminal windows for detailed logs

## Alternative: Use Default Browser

If Cursor's Simple Browser doesn't work, you can also:
- Press `Ctrl+Click` on `http://localhost:8080` in any terminal output
- Or run: `Start-Process "http://localhost:8080"` in PowerShell

## What You'll See

### In the Browser:
- **Pipeline Status**: Shows 1/4 nodes online (your shard node)
- **Query Input**: Text field at the top
- **Response Area**: Where inference results will appear
- **Status Updates**: Real-time updates during inference

### In Terminal Windows:
- **Web Server Terminal**: Shows `[P2P] [OK] Matched response` - confirms fix is working
- **Shard Node Terminal**: Shows `[RESPONSE] [OK] Response sent successfully`

## Success Indicators

✅ **RequestId Matching Fix Working:**
- Look for: `[P2P] [OK] Matched response to waiting channel`
- This confirms responses are being matched correctly

✅ **Inference Processing:**
- Look for: `[INFERENCE] [OK] Shard 0 completed`
- This confirms the coordinator received the response

✅ **Response Displayed:**
- The response text appears in the browser
- Shows the answer to "what do a cat and a snake have in common"

