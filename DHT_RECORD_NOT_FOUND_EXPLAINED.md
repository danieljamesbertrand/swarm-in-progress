# DHT "Record Not Found" - Explained

## The Message

```
[DHT] [QUERY 40] ⚠️  Record not found in DHT (node may not have announced yet)
```

---

## Translation: What This Means

### In Plain English

**"A node queried the DHT looking for a shard, but couldn't find it yet."**

### Breakdown

1. **`[QUERY 40]`**
   - This is DHT query #40
   - Nodes periodically query the DHT to discover other shards

2. **`Record not found in DHT`**
   - The query was looking for a shard announcement record
   - The record doesn't exist in the DHT yet (or hasn't been found)

3. **`node may not have announced yet`**
   - The shard node might not have announced itself yet
   - Or it might still be connecting/bootstrapping
   - Or the record hasn't propagated through the DHT yet

---

## Is This a Problem?

### ✅ NO - This is Normal!

**Why it's OK:**
- Nodes **actively query** the DHT every 10-15 seconds
- They're looking for shards that might not exist yet
- This is **expected behavior** during discovery phase
- As nodes join and announce, these messages decrease

**What it means:**
- Nodes are doing their job ✅
- Discovery mechanism is working ✅
- Just means some shards aren't found yet ⚠️

---

## When You See This

### During Initial Discovery

**What's happening:**
1. Node queries DHT: "Who has shard X?"
2. DHT searches for the record
3. Record not found → This message appears
4. Node will query again in 10-15 seconds

**This is normal** when:
- Nodes are still connecting
- Nodes haven't announced yet
- Records haven't propagated yet
- Network is still forming

---

### As Network Grows

**What changes:**
- **Before:** Many "Record not found" messages
- **After:** More "Record found" messages
- **Eventually:** All records found, fewer queries needed

**Example progression:**
```
[QUERY 1] ⚠️  Record not found (shard 5)
[QUERY 2] ⚠️  Record not found (shard 6)
[QUERY 3] ⚠️  Record not found (shard 7)
... (nodes join and announce)
[QUERY 20] ✓ Record found (shard 5)
[QUERY 21] ✓ Record found (shard 6)
[QUERY 22] ✓ Record found (shard 7)
```

---

## What to Expect

### With 8 Nodes Running

**Initially (first 30 seconds):**
- Many "Record not found" messages
- Nodes are still connecting and announcing
- Discovery is in progress

**After 30-60 seconds:**
- Fewer "Record not found" messages
- More "Record found" messages
- Most/all shards discovered

**When all discovered:**
- Very few "Record not found" messages
- Mostly "Record found" messages
- Status shows: `Discovered Shards: 8 / 8`

---

## Related Messages

### You Might Also See

**Record found:**
```
[DHT] [QUERY 21] ✓ Record found in DHT for shard 1
[DHT] ✓ Discovered shard 1 from peer: 12D3KooW...
```

**QuorumFailed (also normal):**
```
[DHT] [EVENT] PutRecord(Err(QuorumFailed { ... }))
```

**All are normal** during the discovery phase!

---

## What You Should Do

### ✅ Nothing - This is Expected!

**During discovery:**
- "Record not found" is **normal**
- Nodes are actively searching
- Discovery takes 10-15 seconds per cycle
- Be patient - it will find records as nodes announce

**After discovery:**
- Most records should be found
- Fewer "Record not found" messages
- Status should show all shards discovered

---

## Timeline Context

**With your 8 nodes:**

**T+0s:** Nodes starting
**T+5s:** Nodes connecting to server
**T+10s:** Nodes announcing to DHT
**T+15s:** First "Record found" messages appear
**T+30s:** Most records found
**T+60s:** All records should be found

**During T+0 to T+30:**
- Expect many "Record not found" messages
- This is **normal** - nodes are still joining

**After T+30:**
- Should see mostly "Record found" messages
- Fewer "Record not found" messages

---

## Summary

**Translation:**
> "A node queried the DHT for a shard record, but it doesn't exist yet (or hasn't been found). The node will keep querying until it finds it."

**Status:**
- ✅ **Normal** during discovery phase
- ✅ **Not a problem** - nodes are working correctly
- ✅ **Will decrease** as more nodes join and announce

**Action:**
- ✅ **No action needed** - just wait for discovery to complete

---

## When to Worry

**Only worry if:**
- After 2-3 minutes, you still see many "Record not found" for shards that should exist
- Nodes aren't connecting to the server
- No "Record found" messages at all

**Otherwise:**
- This is **expected behavior**
- Discovery is working as designed
- Just wait for nodes to finish joining

---

## See Also

- `DHT_QUORUM_FAILED_EXPLAINED.md` - QuorumFailed errors
- `SWARM_DISCOVERY_MECHANISM.md` - How discovery works
- `CONNECTION_LOG_ANALYSIS.md` - Full connection breakdown
