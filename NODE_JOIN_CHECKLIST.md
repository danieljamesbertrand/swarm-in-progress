# Complete Node Join Checklist

This document lists everything a node must do to successfully join the distributed inference network.

## Table of Contents
1. [Prerequisites](#prerequisites)
2. [Node Startup Sequence](#node-startup-sequence)
3. [File Requirements](#file-requirements)
4. [Connection Requirements](#connection-requirements)
5. [Keepalive & Connection Management](#keepalive--connection-management)
6. [Reporting & Status Updates](#reporting--status-updates)
7. [Testing Checklist](#testing-checklist)

---

## Prerequisites

### Required Processes

- [ ] **Bootstrap Server** must be running
  - **Command**: `cargo run --bin server -- --listen-addr 0.0.0.0 --port 51820`
  - **Port**: 51820 (TCP)
  - **Purpose**: Entry point to DHT network, provides initial routing table
  - **Status Check**: Process named `server` must be running

- [ ] **Web Server** must be running (optional but recommended)
  - **Command**: `cargo run --bin web_server`
  - **Ports**: 8080 (HTTP), 8081 (WebSocket)
  - **Purpose**: Web console, node status monitoring, inference requests
  - **Status Check**: Process named `web_server` must be running

### Required Directories

- [ ] **Shards Directory** must exist
  - **Default Path**: `models_cache/shards/`
  - **Configurable**: Via `LLAMA_SHARDS_DIR` env var or `--shards-dir` argument
  - **Purpose**: Stores GGUF shard files
  - **Check**: Directory must exist (will be created if missing)

---

## Node Startup Sequence

### 1. Node Initialization

- [ ] **Generate Peer Identity**
  - Generate Ed25519 keypair
  - Derive PeerId from public key
  - **Output**: Peer ID printed to console

- [ ] **Initialize State**
  - Create `ShardNodeState` with:
    - Peer ID
    - Shard ID (assigned via `--shard-id` or `LLAMA_SHARD_ID`)
    - Total shards (via `--total-shards` or `LLAMA_TOTAL_SHARDS`)
    - Total layers (via `--total-layers` or `LLAMA_TOTAL_LAYERS`)
    - Model name (via `--model-name` or `LLAMA_MODEL_NAME`)
    - Cluster name (via `--cluster` or `LLAMA_CLUSTER`)
    - Shards directory path

- [ ] **Scan for GGUF Files**
  - Scan `shards_dir` for `.gguf` files
  - Create torrent metadata for each found file
  - Store in `torrent_files` HashMap
  - **Expected**: At least 4 files (shard-0.gguf through shard-3.gguf)

- [ ] **Try to Load Assigned Shard**
  - Attempt to load `shard-{shard_id}.gguf`
  - If successful: Set `capabilities.shard_loaded = true`
  - If failed: Set `capabilities.shard_loaded = false` (node still joins network)
  - **Note**: Node can join without shard loaded (will download later)

### 2. Network Transport Setup

- [ ] **Create Transport**
  - TCP transport with:
    - libp2p upgrade (Version::V1)
    - Noise authentication (using generated keypair)
    - Yamux multiplexing
  - **Purpose**: Secure, multiplexed connections

- [ ] **Initialize Kademlia DHT**
  - Create memory store
  - Configure query timeout: 60 seconds
  - **Purpose**: Distributed hash table for peer discovery

- [ ] **Initialize Protocols**
  - **Identify**: Protocol version `shard-listener/{cluster}/{shard_id}`
  - **Request-Response (JSON)**: `/json-message/1.0` protocol
  - **Request-Response (Metrics)**: `/metrics/1.0` protocol
  - **Request-Response (Torrent)**: `/torrent/1.0` protocol
  - **Relay**: For NAT traversal

- [ ] **Create Swarm**
  - Combine transport + behaviours
  - Configure idle connection timeout: **60 seconds**
  - **Purpose**: Main networking component

### 3. Network Connection

- [ ] **Listen on Port**
  - Listen on `0.0.0.0:{port}` (default port from args or env)
  - **Output**: `[LISTEN] Listening on: /ip4/0.0.0.0/tcp/{port}`

- [ ] **Connect to Bootstrap**
  - Dial bootstrap address (from `--bootstrap` or `LLAMA_BOOTSTRAP`)
  - **Default**: `/ip4/127.0.0.1/tcp/51820`
  - **Output**: `🔗 Connecting to bootstrap node...`

- [ ] **Wait for Connection Established**
  - **Event**: `SwarmEvent::ConnectionEstablished`
  - **Output**: `[CONNECT] ✓ Connection established!`
  - **Action**: Add bootstrap node's address to Kademlia routing table

- [ ] **Start Kademlia Bootstrap**
  - Call `kademlia.bootstrap()`
  - **Output**: `[DHT] ✓ Started Kademlia bootstrap`
  - **Purpose**: Populate routing table with other peers

- [ ] **Wait for Routing Table Update**
  - **Event**: `SwarmEvent::Behaviour(KademliaEvent::RoutingUpdated)`
  - **Output**: `[DHT] Routing updated: {peer_id}`
  - **Purpose**: Indicates DHT is ready for announcements

### 4. DHT Announcement

- [ ] **Announce Shard to DHT**
  - Create `ShardAnnouncement` with:
    - Peer ID
    - Shard ID
    - Total shards
    - Listen addresses
    - Model name
    - Capabilities (shard_loaded, memory, CPU, etc.)
  - Put record to DHT using shard key
  - **Key Format**: `{cluster}/shard/{shard_id}`
  - **Output**: `[DHT] ✓ Announced shard {shard_id} to DHT`

- [ ] **Register Torrent Files in DHT**
  - For each torrent file found:
    - Create DHT record with info_hash as key
    - Store file metadata (filename, size, peer_id)
    - Put record to DHT
  - **Output**: `[TORRENT] ✓ Registered torrent file in DHT: {filename}`
  - **Purpose**: Allow other nodes to discover available files

- [ ] **Set Announcement Flag**
  - Set `announced = true`
  - **Note**: Re-announce if `needs_reannounce` flag is set (e.g., after loading shard)

### 5. Periodic Announcement Refresh

- [ ] **Refresh Announcement**
  - **Interval**: Configurable via `--refresh-interval` (default: 300 seconds)
  - **Action**: Re-announce shard to DHT
  - **Purpose**: Keep DHT records fresh, update capabilities

---

## File Requirements

### Shard File Naming

- [ ] **File Format**: `shard-{shard_id}.gguf`
  - Example: `shard-0.gguf`, `shard-1.gguf`, `shard-2.gguf`, `shard-3.gguf`
  - **Location**: `{shards_dir}/shard-{shard_id}.gguf`

### File Seeding (Torrent)

- [ ] **Scan Directory on Startup**
  - Scan `shards_dir` for all `.gguf` files
  - Create torrent metadata for each file:
    - Calculate SHA256 hash of file → `info_hash`
    - Split file into pieces (default piece size)
    - Calculate SHA256 hash of each piece
  - Store in `torrent_files` HashMap

- [ ] **Seed All Found Files**
  - Each node seeds **all** GGUF files it finds (not just its assigned shard)
  - **Expected**: Each node should seed all 4 shard files (shard-0 through shard-3)
  - **Purpose**: Allow other nodes to download missing shards

- [ ] **Register Files in DHT**
  - Register each torrent file's info_hash in DHT
  - Other nodes can query DHT to find files
  - **Key Format**: `{info_hash}` (SHA256 hash of file)

### File Loading

- [ ] **Load Assigned Shard on Startup**
  - Try to load `shard-{shard_id}.gguf` from local directory
  - If successful:
    - Load model into memory
    - Set `capabilities.shard_loaded = true`
    - **Output**: `[SHARD] ✓✓✓ SHARD {shard_id} LOADED BEFORE JOINING NETWORK ✓✓✓`
  - If failed:
    - Set `capabilities.shard_loaded = false`
    - **Output**: `[SHARD] ⚠️  ASSIGNED SHARD {shard_id} NOT FOUND LOCALLY ⚠️`
    - **Note**: Node still joins network (will download later)

- [ ] **Load Shard on Command**
  - Receive `LOAD_SHARD` command from coordinator
  - Try to load from local directory first
  - If not found, download via torrent from other nodes
  - After loading, set `needs_reannounce = true`
  - Re-announce to DHT with updated capabilities

---

## Connection Requirements

### 1. Bootstrap Connection

- [ ] **Connect to Bootstrap Server**
  - **Address**: From `--bootstrap` argument or `LLAMA_BOOTSTRAP` env var
  - **Default**: `/ip4/127.0.0.1/tcp/51820`
  - **Direction**: Outbound (node dials bootstrap)
  - **Purpose**: Initial entry point to DHT network
  - **Status**: Must succeed before DHT bootstrap

- [ ] **Add Bootstrap to Routing Table**
  - After connection established, add bootstrap's address to Kademlia
  - **Action**: `kademlia.add_address(&bootstrap_peer_id, bootstrap_addr)`
  - **Purpose**: Ensure bootstrap is in routing table

### 2. DHT Connections

- [ ] **Kademlia Bootstrap**
  - After connecting to bootstrap, call `kademlia.bootstrap()`
  - **Purpose**: Populate routing table with other peers in network
  - **Timeout**: 60 seconds (query timeout)

- [ ] **Routing Table Population**
  - Wait for `RoutingUpdated` events
  - **Purpose**: Learn about other peers in network
  - **Minimum**: Should discover at least bootstrap node

### 3. Peer-to-Peer Connections

- [ ] **Direct Connections to Other Nodes**
  - After DHT bootstrap, discover other shard nodes
  - Connect directly to other nodes (not through bootstrap)
  - **Purpose**: Direct communication for inference requests
  - **Protocol**: TCP with Noise + Yamux

- [ ] **Connection for Torrent Downloads**
  - When downloading shard, connect to peer that has the file
  - **Protocol**: Torrent protocol (`/torrent/1.0`)
  - **Purpose**: Download missing shard files

### 4. Web Server Connection (Optional)

- [ ] **Web Server Discovery**
  - Web server discovers nodes via DHT (not direct connection)
  - Web server queries DHT for shard announcements
  - **Purpose**: Web server tracks node status

- [ ] **Status Reporting**
  - Nodes don't directly connect to web server
  - Web server queries nodes via DHT when needed
  - **Note**: No direct WebSocket connection from node to web server

---

## Keepalive & Connection Management

### Connection Timeouts

- [ ] **Idle Connection Timeout**
  - **Value**: 60 seconds
  - **Config**: `SwarmConfig::with_idle_connection_timeout(Duration::from_secs(60))`
  - **Purpose**: Close idle connections to free resources
  - **Note**: Connections are kept alive by activity (messages, pings)

### Keepalive Mechanisms

- [ ] **libp2p Ping Protocol** (Automatic)
  - libp2p automatically sends pings to keep connections alive
  - **Interval**: ~25 seconds (libp2p default)
  - **Purpose**: Prevent idle timeout from closing active connections

- [ ] **DHT Query Activity**
  - Periodic DHT queries keep connections active
  - **Purpose**: Maintain connections to peers in routing table

- [ ] **Announcement Refresh**
  - Periodic re-announcement to DHT
  - **Default Interval**: 300 seconds (5 minutes)
  - **Purpose**: Keep DHT records fresh, maintain connections

- [ ] **Command/Response Activity**
  - When processing commands, connections are active
  - **Purpose**: Inference requests keep connections alive

### Connection State Tracking

- [ ] **Connection Established Event**
  - Track when connections are established
  - **Event**: `SwarmEvent::ConnectionEstablished`
  - **Output**: Log connection details

- [ ] **Connection Closed Event**
  - Track when connections are closed
  - **Event**: `SwarmEvent::ConnectionClosed`
  - **Output**: Log disconnection details and cause

---

## Reporting & Status Updates

### DHT Announcements

- [ ] **Initial Announcement**
  - Announce shard to DHT on first routing table update
  - **Contains**: Peer ID, shard ID, capabilities, addresses
  - **Purpose**: Allow coordinator to discover node

- [ ] **Periodic Re-announcement**
  - Re-announce every `refresh_interval` seconds
  - **Default**: 300 seconds
  - **Purpose**: Keep DHT records fresh, update capabilities

- [ ] **Re-announcement After Shard Load**
  - When shard is loaded (via LOAD_SHARD command)
  - Set `needs_reannounce = true`
  - Re-announce on next routing update
  - **Purpose**: Update capabilities (shard_loaded = true)

### Status Information

- [ ] **Shard Announcement Contains**:
  - Peer ID
  - Shard ID
  - Total shards
  - Total layers
  - Layer range (calculated from shard_id and total_layers)
  - Listen addresses
  - Model name
  - Capabilities:
    - `shard_loaded`: Whether assigned shard is loaded
    - `memory_mb`: Available memory
    - `cpu_cores`: CPU cores
    - `gpu_available`: GPU availability
    - Other capability flags

### Metrics Reporting

- [ ] **Metrics Protocol**
  - Respond to metrics requests via `/metrics/1.0` protocol
  - **Contains**: Request counts, success rates, latency
  - **Purpose**: Allow monitoring of node performance

### Command Responses

- [ ] **Command Protocol**
  - Respond to commands via `/json-message/1.0` protocol
  - **Commands Handled**:
    - `GET_CAPABILITIES`: Return node capabilities
    - `LOAD_SHARD`: Load specified shard
    - `INFERENCE`: Process inference request
    - `LIST_FILES`: List available torrent files
    - `GET_METRICS`: Return node metrics
  - **Response Format**: `CommandResponse` with status and data

---

## Testing Checklist

### Pre-Test Setup

- [ ] **Stop All Existing Processes**
  - Stop all `server`, `web_server`, `shard_listener` processes
  - Verify no processes are running

- [ ] **Verify Shard Files Exist**
  - Check `models_cache/shards/` directory
  - Verify at least 4 shard files: `shard-0.gguf` through `shard-3.gguf`
  - Check file sizes are reasonable (>100MB each)

- [ ] **Start Bootstrap Server**
  - Run: `cargo run --bin server -- --listen-addr 0.0.0.0 --port 51820`
  - Verify process is running
  - Verify listening on port 51820

- [ ] **Start Web Server** (optional)
  - Run: `cargo run --bin web_server`
  - Verify process is running
  - Verify listening on ports 8080 and 8081

### Node Startup Tests

- [ ] **Test Node Startup**
  - Start node with: `cargo run --bin shard_listener -- --shard-id 0 --total-shards 4`
  - Verify console output shows:
    - Peer ID generated
    - Configuration printed
    - Connection to bootstrap
    - DHT bootstrap started
    - Routing table updated
    - Shard announced

- [ ] **Test Shard File Scanning**
  - Verify console shows: `[TORRENT] Found {N} GGUF file(s)`
  - Verify all 4 shard files are found
  - Verify torrent metadata created for each

- [ ] **Test Shard Loading**
  - If shard file exists: Verify `[SHARD] ✓✓✓ SHARD {id} LOADED`
  - If shard file missing: Verify `[SHARD] ⚠️  ASSIGNED SHARD {id} NOT FOUND`
  - Verify node still joins network even if shard missing

### Connection Tests

- [ ] **Test Bootstrap Connection**
  - Verify `[CONNECT] ✓ Connection established!` appears
  - Verify bootstrap peer ID is logged
  - Verify connection direction is "outbound"

- [ ] **Test DHT Bootstrap**
  - Verify `[DHT] ✓ Started Kademlia bootstrap` appears
  - Verify `[DHT] Routing updated: {peer_id}` appears
  - Verify routing table is populated

- [ ] **Test DHT Announcement**
  - Verify `[DHT] ✓ Announced shard {id} to DHT` appears
  - Verify announcement happens after routing update

- [ ] **Test Torrent File Registration**
  - Verify `[TORRENT] ✓ Registered torrent file in DHT: {filename}` for each file
  - Verify all 4 shard files are registered

### Multi-Node Tests

- [ ] **Test 4 Nodes Starting**
  - Start 4 nodes (shard-0 through shard-3)
  - Verify all nodes connect to bootstrap
  - Verify all nodes bootstrap to DHT
  - Verify all nodes announce their shards

- [ ] **Test Node Discovery**
  - From web server or coordinator, query DHT for shards
  - Verify all 4 shards are discovered
  - Verify each shard has correct peer ID and addresses

- [ ] **Test Peer-to-Peer Connections**
  - Verify nodes can discover each other via DHT
  - Verify direct connections between nodes (not through bootstrap)
  - Check connection logs for peer-to-peer connections

### Keepalive Tests

- [ ] **Test Connection Persistence**
  - Let nodes run for >60 seconds
  - Verify connections are not closed (due to keepalive)
  - Check connection logs for no unexpected disconnections

- [ ] **Test Periodic Re-announcement**
  - Wait for refresh interval (default 300 seconds)
  - Verify nodes re-announce to DHT
  - Check DHT records are updated

### File Seeding Tests

- [ ] **Test Torrent File Availability**
  - From another node, send `LIST_FILES` command
  - Verify all 4 shard files are listed
  - Verify info_hash and file sizes are correct

- [ ] **Test Shard Download**
  - Start node without shard file
  - Send `LOAD_SHARD` command
  - Verify node downloads shard via torrent
  - Verify shard is loaded after download

### Status Reporting Tests

- [ ] **Test Web Server Discovery**
  - Open web console: `http://localhost:8080`
  - Verify all 4 nodes appear in pipeline status
  - Verify node status shows "online"
  - Verify shard IDs are correct

- [ ] **Test Status Updates**
  - Check web console shows real-time status updates
  - Verify node join events are logged
  - Verify shard load events are logged

- [ ] **Test Command Responses**
  - Send `GET_CAPABILITIES` command to node
  - Verify response contains correct capabilities
  - Verify `shard_loaded` flag is correct

### Integration Tests

- [ ] **Test Complete Pipeline**
  - All 4 nodes running
  - All 4 shards loaded
  - Pipeline status shows "complete"
  - Send inference request
  - Verify request is processed through all 4 shards

- [ ] **Test Node Failure Recovery**
  - Stop one node
  - Verify web console shows node as offline
  - Restart node
  - Verify node rejoins and pipeline becomes complete again

---

## Summary

A node must successfully complete all of the following to join the network:

1. ✅ **Prerequisites**: Bootstrap server running, shards directory exists
2. ✅ **Startup**: Generate identity, initialize state, scan files, load shard
3. ✅ **Network**: Create transport, connect to bootstrap, bootstrap DHT
4. ✅ **Announcement**: Announce shard to DHT, register torrent files
5. ✅ **Connections**: Maintain connections via keepalives, handle peer-to-peer
6. ✅ **Reporting**: Announce status, respond to commands, report metrics
7. ✅ **Testing**: Verify all components work together

---

## Quick Reference

### Node Startup Command
```bash
cargo run --bin shard_listener -- \
  --bootstrap /ip4/127.0.0.1/tcp/51820 \
  --cluster llama-cluster \
  --shard-id 0 \
  --total-shards 4 \
  --total-layers 32 \
  --model-name llama-8b \
  --port 0 \
  --shards-dir models_cache/shards
```

### Environment Variables
- `LLAMA_BOOTSTRAP`: Bootstrap address
- `LLAMA_CLUSTER`: Cluster name
- `LLAMA_SHARD_ID`: Shard ID (0-3)
- `LLAMA_TOTAL_SHARDS`: Total shards (4)
- `LLAMA_TOTAL_LAYERS`: Total layers (32)
- `LLAMA_MODEL_NAME`: Model name
- `LLAMA_SHARDS_DIR`: Shards directory path

### Expected Console Output
```
Peer ID: {peer_id}
[LISTEN] Listening on: /ip4/0.0.0.0/tcp/{port}
🔗 Connecting to bootstrap node...
[CONNECT] ✓ Connection established!
[DHT] ✓ Started Kademlia bootstrap
[DHT] Routing updated: {peer_id}
[TORRENT] Found 4 GGUF file(s)
[TORRENT] ✓ Registered torrent file in DHT: shard-0.gguf
[TORRENT] ✓ Registered torrent file in DHT: shard-1.gguf
[TORRENT] ✓ Registered torrent file in DHT: shard-2.gguf
[TORRENT] ✓ Registered torrent file in DHT: shard-3.gguf
[SHARD] ✓✓✓ SHARD 0 LOADED BEFORE JOINING NETWORK ✓✓✓
[DHT] ✓ Announced shard 0 to DHT
✅ Shard listener started! Waiting for connections...
```

---

**Last Updated**: 2025-12-27

