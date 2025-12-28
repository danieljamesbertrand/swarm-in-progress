# How to See the Changes

## Quick Steps

### 1. Start the System (if not running)

**Terminal 1 - Bootstrap Server:**
```powershell
cargo run --bin bootstrap_server
```

**Terminal 2 - Web Server (spawns 4 nodes automatically):**
```powershell
cargo run --bin web_server
```

Wait for:
- Bootstrap server to show: `[SERVER] Listening on...`
- Web server to show: `[SERVER] Ensuring minimal pipeline is ready...`
- 4 nodes to spawn (you'll see 4 shard_listener processes)

### 2. Open Web Console

1. Open your browser
2. Navigate to: **http://localhost:8080**
3. **IMPORTANT:** If the page is already open, **refresh it** (F5 or Ctrl+R)
   - This loads the updated JavaScript code

### 3. Find the Pipeline Status Section

1. Scroll down on the web page
2. Look for the **"Pipeline Status"** section
3. You should see buttons for:
   - Input
   - Discovery
   - **Shard 0** ← This should turn RED
   - **Shard 1** ← This should turn RED
   - **Shard 2** ← This should turn RED
   - **Shard 3** ← This should turn RED
   - Output

### 4. What You Should See

When nodes register (happens automatically when they start):

**Before:**
```
[⚫ Gray Button]
Shard 0
Layers 0-7
```

**After (when node registers):**
```
[🔴 RED BUTTON] ← Pulsing red glow
Shard 0
Node: 12D3KooW...NisphD ← Red text, unique identifier
```

### 5. Check Browser Console (Optional)

Press **F12** to open developer tools, then:

1. Click **Console** tab
2. Look for messages like:
   ```
   [WS] Node joined - Shard 0 button turned red (stage 2)
   [WS]   Node ID: 12D3KooW...
   ```

### 6. Hover Over Node ID

- Hover your mouse over the node identifier text
- You should see a tooltip with:
  - Full Peer ID
  - Shard number
  - Registration time

## Troubleshooting

### No Red Buttons?

1. **Check if nodes are running:**
   ```powershell
   Get-Process | Where-Object {$_.ProcessName -eq "shard_listener"}
   ```
   Should show 4 processes

2. **Check WebSocket connection:**
   - Open browser console (F12)
   - Look for `[WS] Connected` message
   - If not connected, refresh the page

3. **Check for node_joined events:**
   - In browser console, look for `[WS] Received node event: node_joined`
   - If you don't see these, nodes aren't registering properly

4. **Make sure you refreshed the page:**
   - The JavaScript changes require a page refresh
   - Hard refresh: Ctrl+Shift+R (or Cmd+Shift+R on Mac)

### Buttons Turn Red But No Node ID?

- Check browser console for errors
- Make sure the `node_id` is included in the `node_joined` event
- Verify WebSocket messages are being received

### Still Not Working?

1. **Restart everything:**
   - Stop all processes
   - Start bootstrap server
   - Start web server
   - Refresh web page

2. **Check web server logs:**
   - Look for `[DHT] ✓ Discovered shard X from...` messages
   - These indicate nodes are being discovered

3. **Verify files:**
   - Make sure `web/ai-console.html` has the latest changes
   - Check that JavaScript includes `registeredNodes` Map

## Expected Timeline

1. **0-5 seconds:** Web server starts, spawns 4 nodes
2. **5-10 seconds:** Nodes connect to bootstrap, join DHT
3. **10-15 seconds:** Web server discovers nodes via DHT
4. **15-20 seconds:** `node_joined` events sent to web console
5. **Immediately:** Buttons turn red, node IDs appear

## Visual Guide

```
┌─────────────────────────────────────┐
│  Pipeline Status                    │
├─────────────────────────────────────┤
│  [📝] Input                         │
│  [🔍] Discovery                     │
│  [🔴] Shard 0                       │ ← RED BUTTON
│       Node: 12D3KooW...NisphD       │ ← Node ID (red text)
│  [🔴] Shard 1                       │
│       Node: 12D3KooW...XXXXX       │
│  [🔴] Shard 2                       │
│       Node: 12D3KooW...YYYYY       │
│  [🔴] Shard 3                       │
│       Node: 12D3KooW...ZZZZZ       │
│  [✨] Output                        │
└─────────────────────────────────────┘
```

## Summary

**To see the changes:**
1. ✅ System running (bootstrap + web server + 4 nodes)
2. ✅ Open http://localhost:8080
3. ✅ **Refresh the page** (important!)
4. ✅ Scroll to Pipeline Status section
5. ✅ Watch buttons turn red as nodes register
6. ✅ See node IDs appear below each red button

