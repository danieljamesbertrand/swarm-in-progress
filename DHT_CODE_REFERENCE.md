# Distributed Hash Table (DHT) Code Reference

This document shows all the code that manages the Kademlia Distributed Hash Table (DHT) in this project.

## Overview

The DHT is managed through libp2p's Kademlia implementation (`libp2p::kad`). The key components are:

1. **MemoryStore**: In-memory storage for DHT records
2. **Behaviour**: Kademlia protocol behavior that handles DHT operations
3. **Record Operations**: Storing and retrieving peer information
4. **Peer Discovery**: Querying the DHT to find peers

## 1. DHT Initialization

### Client Helper (`src/client_helper.rs`)

```rust
// Create Kademlia DHT store and behaviour
let store = kad::store::MemoryStore::new(peer_id);
let mut kademlia_config = kad::Config::default();
kademlia_config.set_query_timeout(Duration::from_secs(60));
let mut kademlia = kad::Behaviour::with_config(peer_id, store, kademlia_config);

// Add bootstrap nodes to Kademlia
let bootstrap_addrs: Result<Vec<Multiaddr>, _> = bootstrap_nodes
    .iter()
    .map(|addr| addr.parse())
    .collect();
let bootstrap_addrs = bootstrap_addrs?;

for addr in &bootstrap_addrs {
    kademlia.add_address(&peer_id, addr.clone());
}
```

**What this does:**
- Creates an in-memory DHT store for this peer
- Configures Kademlia with a 60-second query timeout
- Adds bootstrap node addresses so the peer knows where to join the network

### Listener (`src/listener.rs`)

```rust
// Kademlia DHT
let store = kad::store::MemoryStore::new(peer_id);
let mut kademlia_config = kad::Config::default();
kademlia_config.set_query_timeout(Duration::from_secs(60));
let mut kademlia = kad::Behaviour::with_config(peer_id, store, kademlia_config);

// Add bootstrap node
let bootstrap_addr: Multiaddr = args.bootstrap.parse()?;
kademlia.add_address(&peer_id, bootstrap_addr.clone());
```

### Dialer (`src/dialer.rs`)

Same initialization pattern as listener.

### Bootstrap Node (`src/server.rs`)

```rust
// Kademlia DHT behaviour (bootstrap node)
let store = kad::store::MemoryStore::new(local_peer_id);
let kademlia_config = kad::Config::default();
let kademlia = kad::Behaviour::with_config(local_peer_id, store, kademlia_config);
```

## 2. DHT Bootstrap Process

### Bootstrap Implementation (`src/client_helper.rs`)

```rust
async fn bootstrap_dht(&mut self) -> Result<(), Box<dyn Error>> {
    use tokio::time::{timeout, Duration as TokioDuration};
    
    // Connect to bootstrap nodes
    for addr in &self.bootstrap_nodes {
        if let Err(e) = self.swarm.dial(addr.clone()) {
            eprintln!("[WARN] Failed to dial bootstrap node {}: {:?}", addr, e);
        }
    }

    // Wait for at least one connection and then start bootstrap
    let mut connected = false;
    let bootstrap_timeout = TokioDuration::from_secs(30);
    
    let bootstrap_result = timeout(bootstrap_timeout, async {
        loop {
            match self.swarm.select_next_some().await {
                SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                    if !connected {
                        connected = true;
                        // Start Kademlia bootstrap
                        if let Err(e) = self.swarm.behaviour_mut().kademlia.bootstrap() {
                            eprintln!("[WARN] Bootstrap start failed: {:?}", e);
                        }
                    }
                }
                SwarmEvent::NewListenAddr { address, .. } => {
                    // Add our listening address so peers can connect to us
                    self.swarm.add_external_address(address);
                }
                SwarmEvent::Behaviour(BehaviourEvent::Kademlia(kad::Event::RoutingUpdated { .. })) => {
                    // Bootstrap completed successfully
                    self.bootstrapped = true;
                    return Ok(());
                }
                SwarmEvent::Behaviour(BehaviourEvent::Kademlia(kad::Event::OutboundQueryProgressed { result, .. })) => {
                    if let kad::QueryResult::Bootstrap(Ok(kad::BootstrapOk { .. })) = result {
                        self.bootstrapped = true;
                        return Ok(());
                    }
                }
                _ => {
                    // Continue processing events
                }
            }
        }
    }).await;

    match bootstrap_result {
        Ok(Ok(())) => Ok(()),
        Ok(Err(e)) => Err(e),
        Err(_) => {
            // Timeout - bootstrap may still work, just mark as attempted
            self.bootstrapped = true;
            Ok(())
        }
    }
}
```

**What this does:**
1. Connects to bootstrap nodes
2. Waits for connection to be established
3. Calls `kademlia.bootstrap()` to start the bootstrap process
4. Waits for `RoutingUpdated` or `BootstrapOk` event to confirm bootstrap completion
5. Has a 30-second timeout

### Bootstrap in Listener/Dialer

```rust
// In listener.rs and dialer.rs
SwarmEvent::ConnectionEstablished { peer_id, .. } => {
    if !bootstrapped {
        // Start bootstrap after first connection
        if let Err(e) = swarm.behaviour_mut().kademlia.bootstrap() {
            eprintln!("[WARN] Bootstrap start failed: {:?}", e);
        } else {
            println!("✓ Started Kademlia bootstrap!");
        }
    }
}
```

## 3. Storing Records in DHT

### Storing Peer Information (`src/client_helper.rs`)

```rust
pub async fn connect_to_peer(&mut self) -> Result<PeerId, Box<dyn Error>> {
    // Ensure DHT is bootstrapped
    if !self.bootstrapped {
        return Err("DHT not bootstrapped yet".into());
    }

    // Store our peer info in the DHT with namespace key
    // This allows other peers to find us
    let key = kad::RecordKey::new(&self.namespace);
    let local_peer_id = *self.swarm.local_peer_id();
    let value = local_peer_id.to_bytes();
    let record = kad::Record::new(key.clone(), value);
    self.swarm.behaviour_mut().kademlia.put_record(record, kad::Quorum::One)?;
    
    // ... rest of peer discovery
}
```

**What this does:**
- Creates a DHT record key from the namespace string
- Stores the peer ID as the record value
- Uses `put_record()` with `Quorum::One` (only needs one peer to store it)
- Other peers can query this key to find peers in the same namespace

### Storing in Listener (`src/listener.rs`)

```rust
BehaviourEvent::Kademlia(kad::Event::RoutingUpdated { .. }) => {
    if !bootstrapped {
        bootstrapped = true;
        println!("✓ DHT bootstrapped! Discovering peers...");
        
        // Store our peer info in DHT
        let key = kad::RecordKey::new(&args.namespace);
        let value = peer_id.to_bytes();
        let record = kad::Record::new(key.clone(), value);
        if let Err(e) = swarm.behaviour_mut().kademlia.put_record(record, kad::Quorum::One) {
            eprintln!("[WARN] Failed to put record: {:?}", e);
        }
        
        // Query for peers in namespace
        swarm.behaviour_mut().kademlia.get_record(key);
        swarm.behaviour_mut().kademlia.get_closest_peers(peer_id);
    }
}
```

## 4. Querying the DHT

### Getting Records (`src/client_helper.rs`)

```rust
// Query for the record (to find other peers in the same namespace)
self.swarm.behaviour_mut().kademlia.get_record(key);

// Also query for closest peers to find any nearby peers
self.swarm.behaviour_mut().kademlia.get_closest_peers(local_peer_id);
```

**What this does:**
- `get_record(key)`: Queries the DHT for a specific record (namespace-based)
- `get_closest_peers(peer_id)`: Finds peers closest to a given peer ID in the DHT

### Handling Query Results (`src/client_helper.rs`)

```rust
SwarmEvent::Behaviour(BehaviourEvent::Kademlia(kad::Event::OutboundQueryProgressed { result, .. })) => {
    match result {
        kad::QueryResult::GetClosestPeers(Ok(ok)) => {
            for peer_id in ok.peers {
                // Don't try to connect to ourselves
                if peer_id != local_peer_id && !self.connected_peers.contains_key(&peer_id) {
                    // Kademlia will automatically try to connect when we query
                    // We'll wait for ConnectionEstablished event
                }
            }
        }
        kad::QueryResult::GetRecord(Ok(kad::GetRecordOk::FoundRecord(record))) => {
            // Found a record - try to extract peer ID from it
            // The record value should contain a peer ID
            // For now, we'll rely on GetClosestPeers for connections
        }
        _ => {}
    }
}
```

**What this does:**
- Handles `GetClosestPeers` results: processes the list of discovered peers
- Handles `GetRecord` results: processes found records (could contain peer info)
- Filters out self and already-connected peers

### Query Results in Dialer (`src/dialer.rs`)

```rust
BehaviourEvent::Kademlia(kad::Event::OutboundQueryProgressed { result, .. }) => {
    match result {
        kad::QueryResult::GetClosestPeers(Ok(ok)) => {
            println!("[VERBOSE] ✓ Found {} peer(s) in DHT", ok.peers.len());
            for discovered_peer in ok.peers {
                if !discovered_peers.contains(&discovered_peer) && discovered_peer != peer_id {
                    discovered_peers.push(discovered_peer);
                    println!("[VERBOSE]   Found peer: {}", discovered_peer);
                    // Kademlia will handle connection automatically
                }
            }
        }
        kad::QueryResult::GetRecord(Ok(kad::GetRecordOk::FoundRecord(_record))) => {
            println!("[VERBOSE] ✓ Found record in DHT");
            // Record contains peer info - connection will be established automatically
        }
        kad::QueryResult::Bootstrap(Ok(kad::BootstrapOk { .. })) => {
            if !bootstrapped {
                bootstrapped = true;
                println!("✓ DHT bootstrapped!");
            }
        }
        _ => {}
    }
}
```

## 5. DHT Event Handling

### Event Types

The DHT generates several event types:

1. **RoutingUpdated**: DHT routing table was updated (bootstrap complete)
2. **OutboundQueryProgressed**: A DHT query made progress or completed
3. **InboundRequest**: Another peer is querying us

### Event Handling Structure

```rust
#[derive(NetworkBehaviour)]
#[behaviour(out_event = "BehaviourEvent")]
struct Behaviour {
    kademlia: kad::Behaviour<kad::store::MemoryStore>,
    identify: libp2p::identify::Behaviour,
    request_response: request_response::Behaviour<JsonCodec>,
}

#[derive(Debug)]
enum BehaviourEvent {
    Kademlia(kad::Event),
    Identify(libp2p::identify::Event),
    RequestResponse(request_response::Event<JsonCodec>),
}

impl From<kad::Event> for BehaviourEvent {
    fn from(event: kad::Event) -> Self {
        BehaviourEvent::Kademlia(event)
    }
}
```

### Processing Events

```rust
match self.swarm.select_next_some().await {
    SwarmEvent::Behaviour(BehaviourEvent::Kademlia(kad::Event::RoutingUpdated { .. })) => {
        // DHT routing table updated - bootstrap likely complete
        self.bootstrapped = true;
    }
    SwarmEvent::Behaviour(BehaviourEvent::Kademlia(kad::Event::OutboundQueryProgressed { result, .. })) => {
        match result {
            kad::QueryResult::Bootstrap(Ok(_)) => { /* Bootstrap complete */ }
            kad::QueryResult::GetClosestPeers(Ok(ok)) => { /* Peers found */ }
            kad::QueryResult::GetRecord(Ok(ok)) => { /* Record found */ }
            kad::QueryResult::PutRecord(Ok(_)) => { /* Record stored */ }
            _ => {}
        }
    }
    // ... other events
}
```

## 6. DHT Store Implementation

The DHT uses `kad::store::MemoryStore` which is an in-memory implementation:

```rust
let store = kad::store::MemoryStore::new(peer_id);
```

**Characteristics:**
- **In-memory**: Data is lost when peer disconnects
- **Per-peer**: Each peer has its own store
- **Distributed**: Records are replicated across multiple peers
- **Automatic**: libp2p handles replication and expiration

## 7. Key DHT Operations Summary

### Storing Data

```rust
let key = kad::RecordKey::new("my-namespace");
let value = b"peer-data".to_vec();
let record = kad::Record::new(key, value);
kademlia.put_record(record, kad::Quorum::One)?;
```

### Retrieving Data

```rust
let key = kad::RecordKey::new("my-namespace");
kademlia.get_record(key);
```

### Finding Peers

```rust
let target_peer_id = PeerId::from_bytes(&hash)?;
kademlia.get_closest_peers(target_peer_id);
```

### Bootstrap

```rust
kademlia.bootstrap()?;
```

## 8. DHT Configuration

### Current Configuration

```rust
let mut kademlia_config = kad::Config::default();
kademlia_config.set_query_timeout(Duration::from_secs(60));
let kademlia = kad::Behaviour::with_config(peer_id, store, kademlia_config);
```

**Configuration options:**
- `query_timeout`: How long to wait for DHT queries (60 seconds)
- Other options available in `kad::Config` (replication factor, etc.)

## 9. How Namespace Works with DHT

The namespace is used as the DHT record key:

```rust
// Create key from namespace
let key = kad::RecordKey::new(&self.namespace);
// key is now a hash of the namespace string

// Store peer info with this key
let record = kad::Record::new(key.clone(), peer_id_bytes);
kademlia.put_record(record, kad::Quorum::One)?;

// Other peers query with the same key
kademlia.get_record(key);  // Finds all peers in this namespace
```

**How it works:**
1. Namespace string is hashed to create a DHT key
2. All peers in the same namespace use the same key
3. Multiple peers can store records with the same key
4. Querying the key returns all stored records (all peers in namespace)

## 10. Complete DHT Flow

### Initialization Flow

```
1. Create MemoryStore
2. Create Kademlia Behaviour with store
3. Add bootstrap node addresses
4. Add to Swarm Behaviour
```

### Bootstrap Flow

```
1. Connect to bootstrap node
2. Call kademlia.bootstrap()
3. Wait for RoutingUpdated event
4. Mark as bootstrapped
```

### Peer Discovery Flow

```
1. Store own peer info in DHT (put_record)
2. Query DHT for namespace key (get_record)
3. Query for closest peers (get_closest_peers)
4. Process query results
5. Connect to discovered peers
```

### Record Storage Flow

```
1. Create RecordKey from namespace
2. Create Record with peer data
3. Call put_record() with Quorum::One
4. DHT replicates record to k closest peers
5. Record stored in multiple peers' stores
```

## 11. DHT Maintenance

The DHT automatically:
- **Replicates records** to k closest peers (k-bucket replication)
- **Refreshes records** periodically to keep them alive
- **Expires old records** that aren't refreshed
- **Updates routing tables** as peers join/leave
- **Handles peer failures** by finding alternative peers

All of this is handled internally by libp2p's Kademlia implementation.

## 12. Key Files and Locations

| File | DHT Code Location |
|------|------------------|
| `src/client_helper.rs` | Lines 208-223 (init), 275-340 (bootstrap), 386-435 (discovery) |
| `src/listener.rs` | Lines 67-74 (init), 147-157 (bootstrap & store) |
| `src/dialer.rs` | Lines 67-74 (init), 180-195 (bootstrap & store), 198-214 (queries) |
| `src/server.rs` | Lines 61-63 (init) |

## Notes

- The DHT store is **in-memory** - records are lost when peers disconnect
- Records are **automatically replicated** across multiple peers
- The DHT uses **XOR distance metric** for peer discovery
- **Quorum::One** means only one peer needs to store the record (faster but less reliable)
- For production, consider using **Quorum::Majority** for better reliability







