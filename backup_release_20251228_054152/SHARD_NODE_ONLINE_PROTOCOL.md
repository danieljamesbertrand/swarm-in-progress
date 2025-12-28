# Shard Node "Online" Protocol - Complete Flow

## What Signifies a Node is "Online"

A shard node is considered **online** when:
1. ✅ Node has put its shard announcement record into Kademlia DHT
2. ✅ Coordinator has successfully queried and found the record
3. ✅ Record passes freshness validation
4. ✅ Node appears in `discovery.get_pipeline()`

## Complete Protocol Flow

### Phase 1: Node Announces to DHT

**File**: `src/shard_listener.rs:757`

```rust
// Node creates announcement record
let record = s.create_announcement_record();

// Node puts record into DHT
swarm.behaviour_mut().kademlia.put_record(record, kad::Quorum::One)
```

**Prerequisites:**
- Node must have bootstrapped: `kademlia.bootstrap()` called
- Node must have received `RoutingUpdated` event (routing table populated)
- Node must have added its own address to Kademlia: `kademlia.add_address(&peer_id, address)`
- Node must have added bootstrap address: `kademlia.add_address(&bootstrap_peer_id, bootstrap_addr)`

**Key**: Record is stored with key: `dht_keys::shard_key("llama-cluster", shard_id)`

### Phase 2: Coordinator Queries DHT

**File**: `src/bin/web_server.rs:1150`

```rust
// Coordinator queries for each shard (0-3)
for shard_id in 0..total_shards {
    let key = kad::RecordKey::new(&dht_keys::shard_key(&cluster_name, shard_id));
    swarm_guard.behaviour_mut().kademlia.get_record(key);
}
```

**Prerequisites:**
- Coordinator must have bootstrapped: `kademlia.bootstrap()` called
- Coordinator must have added bootstrap address: `kademlia.add_address(&bootstrap_peer_id, bootstrap_addr)`
- Queries run periodically (every 10 seconds after initial bootstrap)

### Phase 3: Coordinator Receives Found Record

**File**: `src/bin/web_server.rs:1036`

```rust
DiscoveryBehaviourEvent::Kademlia(kad::Event::OutboundQueryProgressed {
    result: kad::QueryResult::GetRecord(Ok(kad::GetRecordOk::FoundRecord(peer_record))),
    ..
}) => {
    // Process discovered shard
    if let Some(announcement) = coordinator.process_dht_record(&peer_record.record).await {
        println!("[DHT] ✓ Discovered shard {} from {}", announcement.shard_id, announcement.peer_id);
    }
}
```

### Phase 4: Record Processing

**File**: `src/kademlia_shard_discovery.rs:424`

```rust
pub fn process_shard_record(&mut self, record: &kad::Record) -> Option<ShardAnnouncement> {
    // 1. Deserialize announcement
    let announcement = ShardAnnouncement::from_bytes(&record.value).ok()?;
    
    // 2. Validate freshness (must be within TTL)
    if !announcement.is_fresh(self.ttl_seconds) {
        return None; // Stale record ignored
    }
    
    // 3. Store in known_shards
    let replicas = self.known_shards.entry(announcement.shard_id).or_insert_with(Vec::new);
    replicas.push(announcement.clone());
    
    // 4. Rebuild pipeline order
    self.rebuild_pipeline();
    
    Some(announcement)
}
```

### Phase 5: Status Check

**File**: `src/pipeline_coordinator.rs:1226`

```rust
pub async fn get_pipeline_status(&self) -> (u32, u32, Vec<u32>, bool) {
    let discovery = self.discovery.read().await;
    let pipeline = discovery.get_pipeline(); // Returns nodes from known_shards
    let online_nodes = pipeline.len() as u32; // Count of discovered nodes
    // ...
}
```

**File**: `src/kademlia_shard_discovery.rs:503`

```rust
pub fn get_pipeline(&self) -> Vec<&ShardAnnouncement> {
    self.pipeline_order
        .iter()
        .filter_map(|id| self.get_best_node_for_shard(*id))
        .collect()
}
```

## Critical Requirements

### For Nodes to be Discoverable:

1. **DHT Routing Must Work**:
   - Nodes must add their addresses to Kademlia
   - Bootstrap node address must be registered
   - Routing table must be populated

2. **Records Must be Fresh**:
   - Default TTL: 3600 seconds (1 hour)
   - Records older than TTL are ignored

3. **Coordinator Must Query**:
   - Queries run every 10 seconds after bootstrap
   - Must successfully route to nodes storing records

## Current Issue

**Problem**: Coordinator reports `0 nodes online` even though 4 nodes are running.

**Root Cause**: DHT routing is broken:
- Nodes show as `UnroutablePeer` in bootstrap logs
- Coordinator's `get_record()` queries cannot route to nodes
- Records may be stored but unreachable

**Solution**: 
1. ✅ Fixed: Nodes add their own addresses to Kademlia
2. ✅ Fixed: Nodes add bootstrap address to Kademlia  
3. ✅ Fixed: Coordinator adds bootstrap address to Kademlia
4. ⏳ **Need to verify**: Nodes are actually putting records
5. ⏳ **Need to verify**: Coordinator is receiving `FoundRecord` events

## Diagnostic Checklist

To verify a node is online:

1. **Check node logs** for:
   ```
   [DHT] ✓✓✓ ANNOUNCED SHARD X TO DHT ✓✓✓
   ```

2. **Check coordinator logs** for:
   ```
   [DHT] ✓ Discovered shard X from <peer_id>
   [STATUS] Pipeline: X/4 shards online
   ```

3. **Check DHT routing**:
   - Bootstrap logs should show `RoutingUpdated` not `UnroutablePeer`
   - Nodes should have routing table entries

4. **Check query results**:
   - Coordinator should receive `FoundRecord` events
   - Records should pass freshness check

