# Why Only 1 of 4 Shards Are Online

## Explanation

The system expects **4 shard nodes** to form a complete pipeline:
- **Shard 0**: Layers 0-7 (embeddings)
- **Shard 1**: Layers 8-15
- **Shard 2**: Layers 16-23
- **Shard 3**: Layers 24-31 (output)

## Current Situation

Only **1 shard node (shard 0)** was started for single-node testing. This is why you see "1/4 shards online" in the web UI.

## Why This Happens

1. **Single-Node Test Setup**: The test script (`test_single_node_inference.ps1`) only starts shard 0
2. **Full Pipeline Requires 4 Nodes**: For complete distributed inference, all 4 shards need to be running
3. **Pipeline Coordinator**: Expects 4 shards and will show missing shards (1, 2, 3) until they're started

## Options

### Option 1: Single-Node Testing (Current)
- **Status**: 1/4 shards online
- **Works for**: Basic inference testing
- **Limitation**: May not work for full pipeline inference
- **Use case**: Quick testing, development

### Option 2: Full Pipeline (4/4 Shards)
- **Status**: 4/4 shards online
- **Works for**: Complete distributed inference
- **Requires**: All 4 shard nodes running
- **Use case**: Production, full feature testing

## How to Get All 4 Shards Online

Run this command:
```powershell
powershell -ExecutionPolicy Bypass -File start_all_4_shards.ps1
```

This will:
1. ✅ Start bootstrap server (if not running)
2. ✅ Clean up any existing shard nodes
3. ✅ Start all 4 shard nodes (shard 0, 1, 2, 3)
4. ✅ Wait for them to register
5. ✅ Show final status

## What Each Shard Does

- **Shard 0**: Processes input embeddings (first 8 layers)
- **Shard 1**: Processes middle layers 8-15
- **Shard 2**: Processes middle layers 16-23
- **Shard 3**: Processes output layers 24-31 (final layers)

## Pipeline Flow

```
Input → Shard 0 → Shard 1 → Shard 2 → Shard 3 → Output
```

Each shard processes its portion and passes the result to the next shard in sequence.

## Checking Status

In the web UI, you should see:
- **1/4 nodes online**: Only shard 0 is running
- **4/4 nodes online**: All shards are running (full pipeline)

## Troubleshooting

**If shards don't appear:**
1. Wait 10-20 seconds for DHT discovery
2. Check each shard node terminal for errors
3. Verify bootstrap server is running
4. Check that shard files exist (shard-0.gguf, shard-1.gguf, etc.)

**If only some shards appear:**
- Nodes may still be compiling (first run takes time)
- Check terminal windows for compilation progress
- Wait for "Peer ID: ..." message in each terminal

