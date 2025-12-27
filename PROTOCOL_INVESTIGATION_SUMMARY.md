# Protocol Investigation Summary

## Question: What Signifies a Shard Node Coming Online?

## Answer

A shard node is considered **"online"** when it appears in the coordinator's `discovery.get_pipeline()` list. This requires:

### Complete Protocol Flow

1. **Node Side**:
   - ✅ Bootstraps to DHT (`kademlia.bootstrap()`)
   - ✅ Adds own address to Kademlia (`kademlia.add_address(&peer_id, address)`)
   - ✅ Adds bootstrap address to Kademlia
   - ✅ Receives `RoutingUpdated` event (routing table populated)
   - ✅ Creates announcement record (`create_announcement_record()`)
   - ✅ Puts record into DHT (`kademlia.put_record(record, Quorum::One)`)

2. **Coordinator Side**:
   - ✅ Bootstraps to DHT
   - ✅ Adds bootstrap address to Kademlia
   - ✅ Queries DHT for each shard (`kademlia.get_record(key)`)
   - ✅ Receives `FoundRecord` event (`OutboundQueryProgressed { result: GetRecord(Ok(GetRecordOk::FoundRecord(...))) }`)
   - ✅ Processes record (`process_dht_record()` → `process_shard_record()`)
   - ✅ Validates freshness (within TTL)
   - ✅ Stores in `known_shards`
   - ✅ Node appears in `get_pipeline()`

3. **Status Check**:
   - `get_pipeline_status()` calls `discovery.get_pipeline()`
   - Returns `pipeline.len()` as `online_nodes`

## Key Code Locations

- **Node Announcement**: `src/shard_listener.rs:757` - `put_record()`
- **Coordinator Query**: `src/bin/web_server.rs:1150` - `get_record()`
- **Record Processing**: `src/kademlia_shard_discovery.rs:424` - `process_shard_record()`
- **Status Check**: `src/pipeline_coordinator.rs:1226` - `get_pipeline_status()`

## Current Issue

**Problem**: Coordinator reports `0 nodes online` even though 4 nodes are running.

**Root Cause**: DHT routing is broken - coordinator's queries cannot route to nodes storing records.

**Fixes Applied**:
1. ✅ Nodes add their own addresses to Kademlia
2. ✅ Nodes add bootstrap address to Kademlia
3. ✅ Coordinator adds bootstrap address to Kademlia

**Next Steps**: Verify nodes are actually putting records and coordinator is receiving `FoundRecord` events.

## Documentation

See `SHARD_NODE_ONLINE_PROTOCOL.md` for complete protocol details.

