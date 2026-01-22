# Node-to-Node Communication Guide

## ✅ YES - Nodes Talk to Each Other Automatically!

When you start multiple nodes, they **automatically discover and communicate** with each other through:

1. **DHT Discovery** (automatic, every 10-15 seconds)
2. **Direct P2P Connections** (QUIC preferred, TCP fallback)
3. **Multiple Communication Protocols**

---

## How They Discover Each Other

### Timeline (Automatic)

**T+0s: Shard 1 starts**
- Connects to rendezvous server
- Bootstraps to DHT
- Announces shard 1 to DHT

**T+5s: Shard 1 announces**
- DHT record stored: `/llama-cluster/llama-cluster/shard/1`
- Contains: Peer ID, addresses, shard status

**T+10-15s: Shard 0 queries DHT**
- Periodic query finds Shard 1's record
- Processes discovery event

**T+15s: Direct connection**
- Shard 0 dials Shard 1 directly
- QUIC connection established (if both support it)
- **Now they can talk!**

---

## Communication Methods

### 1. JSON Messages (Request-Response Protocol)

**Protocol**: `/json-message/1.0`

**What it's used for:**
- General peer-to-peer messaging
- Status updates
- Coordination messages

**Example:**
```rust
// Node A sends to Node B
let json_msg = JsonMessage {
    from: "shard-0",
    message: "Hello from shard 0!",
    timestamp: "..."
};
swarm.behaviour_mut().request_response.send_request(&peer_id, json_msg);
```

**Node B receives:**
```
[MSG] 📥 RECEIVED MESSAGE FROM PEER: 12D3KooW...
[MSG]   From: shard-0
[MSG]   Message: Hello from shard 0!
```

---

### 2. Command Protocol

**Protocol**: `/json-message/1.0` (same as JSON messages, but structured commands)

**Commands nodes can send to each other:**

- **`LOAD_SHARD`**: Request a node to load a specific shard
- **`INFERENCE`**: Request inference on a specific shard
- **`SYNC_TORRENTS`**: Synchronize available torrent files
- **`SWARM_READY`**: Broadcast that swarm is ready
- **`GET_STATUS`**: Request node status

**Example:**
```rust
let cmd = Command::new("LOAD_SHARD", "shard-0", Some(&target_peer_id))
    .with_param("shard_id", json!(1))
    .with_param("model_name", json!("llama-8b"));
```

---

### 3. Torrent File Transfers

**Protocol**: libp2p request-response with custom torrent codec

**What it's used for:**
- Downloading shard files from other nodes
- Sharing GGUF files across the network
- Automatic file propagation

**How it works:**
1. Node A announces: "I have shard-1.gguf (hash: abc123)"
2. Node B queries DHT: "Who has file with hash abc123?"
3. DHT returns: "Node A has it"
4. Node B connects to Node A
5. Node B requests file via torrent protocol
6. Node A sends file pieces
7. Node B assembles complete file

---

### 4. DHT Queries and Announcements

**Protocol**: Kademlia DHT

**What it's used for:**
- Peer discovery
- File location queries
- Shard availability announcements

**Keys used:**
- `/llama-cluster/llama-cluster/shard/{shard_id}` - Shard announcements
- `{file_hash}` - Torrent file locations
- `/llama-cluster/swarm-ready` - Swarm readiness status

---

## What You'll See in Logs

### On Shard 0 (Existing Node)

**When Shard 1 joins:**
```
[DHT] [QUERY 21] ✓ Record found in DHT for shard 1
[DHT] ✓ Discovered shard 1 from peer: 12D3KooW...
[DISCOVERY] 📡 Dialing discovered peer 12D3KooW... using QUIC transport
[CONNECT] ✓✓✓ CONNECTED TO PEER ✓✓✓
[CONNECT]   Peer ID: 12D3KooW... (shard 1 node)
[CONNECT]   Transport: QUIC
[STATUS] Discovered Shards: 0 → 1
```

**When receiving messages:**
```
[MSG] 📥 RECEIVED MESSAGE FROM PEER: 12D3KooW...
[MSG]   Command: SYNC_TORRENTS
[MSG]   Message: {"command":"SYNC_TORRENTS",...}
```

### On Shard 1 (New Node)

**When connecting:**
```
[CONNECT] ═══════════════════════════════════════════════════════════════════════════
[CONNECT] ✓✓✓ CONNECTED TO BOOTSTRAP NODE ✓✓✓
[DHT] ✓✓✓ ANNOUNCED SHARD 1 TO DHT ✓✓✓
```

**When discovered by Shard 0:**
```
[CONNECT] ✓✓✓ CONNECTED TO PEER ✓✓✓
[CONNECT]   Peer ID: 12D3KooW... (shard 0 node)
[CONNECT]   Transport: QUIC
```

---

## Direct P2P Connection Details

### Connection Flow

1. **Discovery**: Node A finds Node B via DHT
2. **Address Resolution**: DHT provides Node B's multiaddr
3. **Direct Dial**: Node A dials Node B directly (not through rendezvous server)
4. **Transport Selection**: 
   - QUIC preferred (if both support it)
   - TCP fallback
5. **Connection Established**: Direct P2P link active

### Why Direct P2P?

- **Faster**: No server relay needed
- **Lower Latency**: Direct connection
- **Scalable**: Server not a bottleneck
- **Resilient**: Works even if server goes down

### Rendezvous Server Role

- **Bootstrap only**: Helps nodes join the network
- **DHT bootstrap**: Initial routing table population
- **Not needed for messaging**: Once nodes connect, they talk directly

---

## Testing Communication

### Method 1: Watch the Logs

**Shard 0 window:**
- Look for `[DHT] ✓ Discovered shard 1`
- Look for `[CONNECT] ✓✓✓ CONNECTED TO PEER`
- Look for `[MSG] 📥 RECEIVED MESSAGE`

**Shard 1 window:**
- Look for `[CONNECT] ✓✓✓ CONNECTED TO PEER`
- Look for `[MSG] 📥 RECEIVED MESSAGE`

### Method 2: Check Status Reports

Both nodes periodically print status:
```
[STATUS] Discovered Shards: 1
[STATUS] Shard Online Status:
[STATUS]   Shard 0: ★ LOCAL (or ✓ ONLINE)
[STATUS]   Shard 1: ✓ ONLINE
```

### Method 3: Send Test Message

Nodes can send test messages to each other via the command protocol or JSON messages.

---

## Network Topology

### After Both Nodes Connect

```
Rendezvous Server (bootstrap only)
  ├─ Shard 0 Node
  │  └─ Direct P2P ←→ Shard 1 Node
  └─ Shard 1 Node
     └─ Direct P2P ←→ Shard 0 Node
```

**Key Points:**
- Both nodes connected to rendezvous server (for DHT bootstrap)
- **Direct P2P connection** between nodes (for messaging)
- Messages flow directly between nodes (not through server)

---

## Summary

✅ **YES - Nodes talk to each other automatically!**

1. **Discovery**: Automatic via DHT (10-15 seconds)
2. **Connection**: Direct P2P (QUIC preferred)
3. **Communication**: Multiple protocols available
4. **No manual intervention**: Fully automatic

**Watch the node windows** - you'll see discovery and connection messages within 15 seconds!
