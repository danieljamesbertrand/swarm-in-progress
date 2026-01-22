# Deep Analysis: QUIC Connection Retry Logging

## Log Message Flow

### Message 1: `[CONNECT] ↻ Retrying bootstrap connection...`

**Location:** `src/shard_listener.rs:2483`

**Code:**
```rust
// Bootstrap connection retry
_ = tokio::time::sleep_until(bootstrap_retry_timer), if !bootstrap_connected => {
    println!("[CONNECT] ↻ Retrying bootstrap connection...");
    if let Err(e) = swarm.dial(bootstrap_addr.clone()) {
        eprintln!("[CONNECT] ⚠️  Retry dial failed: {:?}", e);
    }
    bootstrap_retry_timer = tokio::time::Instant::now() + Duration::from_secs(5);
}
```

**When it fires:**
- The `tokio::select!` loop reaches the retry timer deadline
- Condition: `!bootstrap_connected` must be `true`
- Timer was set 5 seconds earlier (line 891, 2463, 2472, or 2487)

**Flow:**
1. Timer expires → retry branch executes
2. Logs "Retrying bootstrap connection..."
3. Calls `swarm.dial(bootstrap_addr.clone())` to attempt connection
4. Sets new retry timer for 5 seconds later
5. If dial fails immediately, logs "Retry dial failed"

---

### Message 2: `[CONNECT] ⚠️  Bootstrap connection failed: Transport([...HandshakeTimedOut...])`

**Location:** `src/shard_listener.rs:2461` or `2470`

**Code:**
```rust
SwarmEvent::OutgoingConnectionError { error, peer_id: failed_peer, .. } => {
    // Check if this is a bootstrap connection failure
    if let Some(peer) = failed_peer {
        // Try to determine if this was the bootstrap by checking if we're not connected
        if !bootstrap_connected {
            eprintln!("[CONNECT] ⚠️  Bootstrap connection failed: {:?}", error);
            eprintln!("[CONNECT] ↻ Will retry in 5 seconds...");
            bootstrap_retry_timer = tokio::time::Instant::now() + Duration::from_secs(5);
        } else {
            eprintln!("[ERROR] Connection failed to {:?}: {:?}", peer, error);
        }
    } else {
        // No peer_id means it might be the bootstrap (initial dial)
        if !bootstrap_connected {
            eprintln!("[CONNECT] ⚠️  Bootstrap connection failed: {:?}", error);
            eprintln!("[CONNECT] ↻ Will retry in 5 seconds...");
            bootstrap_retry_timer = tokio::time::Instant::now() + Duration::from_secs(5);
        }
    }
}
```

**When it fires:**
- libp2p's QUIC transport (Quinn) times out during handshake
- `SwarmEvent::OutgoingConnectionError` is emitted by the swarm
- The error contains `HandshakeTimedOut` from the QUIC layer
- Condition: `!bootstrap_connected` is `true`

**Error Structure:**
```
Transport([
    (/ip4/162.221.207.169/udp/51820/quic-v1, 
     Other(Custom { kind: Other, error: HandshakeTimedOut }))
])
```

This indicates:
- Address attempted: `/ip4/162.221.207.169/udp/51820/quic-v1`
- Error type: `HandshakeTimedOut` from QUIC transport
- Error kind: `Other` (not a standard IO error)

---

### Message 3: `[CONNECT] ↻ Will retry in 5 seconds...`

**Location:** `src/shard_listener.rs:2462` or `2471`

**Code:** (Same block as Message 2)

**When it fires:**
- Immediately after logging the connection failure
- Sets `bootstrap_retry_timer` to 5 seconds in the future
- This timer will trigger Message 1 again

---

## Complete Retry Cycle

```
Time 0s:  Initial dial (line 884)
         swarm.dial(bootstrap_addr.clone())?

Time ~3s: QUIC handshake times out (default timeout in Quinn)
         → SwarmEvent::OutgoingConnectionError emitted
         → Line 2461/2470: Log "Bootstrap connection failed"
         → Line 2462/2471: Log "Will retry in 5 seconds"
         → Line 2463/2472: Set timer to now + 5s

Time 5s:  Timer expires (line 2482)
         → Line 2483: Log "Retrying bootstrap connection..."
         → Line 2484: swarm.dial() again
         → Line 2487: Set timer to now + 5s

Time ~8s: Handshake times out again
         → SwarmEvent::OutgoingConnectionError
         → Log failure, set timer

Time 10s: Timer expires, retry again
         → Cycle repeats...
```

---

## Why HandshakeTimedOut Occurs

### 1. QUIC Handshake Process

The QUIC handshake involves:

1. **Client → Server: QUIC Initial Packet**
   - UDP packet to server
   - Contains TLS 1.3 ClientHello
   - Destination Connection ID (DCID)

2. **Server → Client: QUIC Initial Packet**
   - UDP packet from server
   - Contains TLS 1.3 ServerHello
   - Source Connection ID (SCID)

3. **TLS 1.3 Handshake Completion**
   - Additional round trips for key exchange
   - Certificate verification
   - Connection established

### 2. Default Timeout

**Location:** `src/quic_transport.rs:74`

```rust
let quic_config = quic::Config::new(keypair);
```

**Issue:** No explicit timeout configuration!

The QUIC transport uses libp2p's default configuration, which internally uses Quinn's defaults:
- **Default handshake timeout:** ~3 seconds (varies by libp2p version)
- **No keepalive during handshake**
- **No retry configuration**

### 3. Possible Causes of Timeout

1. **Server Not Running**
   - No process listening on UDP 51820
   - No response to QUIC Initial packet
   - Handshake times out waiting for ServerHello

2. **Firewall Blocking**
   - UDP packets blocked
   - Server can't receive Initial packet
   - Or server response blocked on return path

3. **Network Issues**
   - Packet loss
   - NAT traversal problems
   - Routing issues

4. **Server Not Listening on QUIC**
   - Server running but only TCP transport
   - Server crashed after startup
   - Server listening on wrong interface

5. **QUIC Configuration Mismatch**
   - Server/client using incompatible QUIC versions
   - TLS configuration mismatch

---

## Code Flow Diagram

```
┌─────────────────────────────────────────────────────────────┐
│ shard_listener.rs:884                                        │
│ swarm.dial(bootstrap_addr.clone())?                          │
│   ↓                                                          │
│ libp2p QUIC Transport                                        │
│   ↓                                                          │
│ Quinn QUIC Library                                           │
│   ↓                                                          │
│ Send QUIC Initial Packet (UDP)                              │
└─────────────────────────────────────────────────────────────┘
                          ↓
                    [Wait for response]
                          ↓
                    [Timeout after ~3s]
                          ↓
┌─────────────────────────────────────────────────────────────┐
│ libp2p emits SwarmEvent::OutgoingConnectionError             │
│   Error: HandshakeTimedOut                                   │
└─────────────────────────────────────────────────────────────┘
                          ↓
┌─────────────────────────────────────────────────────────────┐
│ shard_listener.rs:2456                                       │
│ SwarmEvent::OutgoingConnectionError handler                  │
│   ↓                                                          │
│ if !bootstrap_connected {                                    │
│     eprintln!("[CONNECT] ⚠️  Bootstrap connection failed"); │
│     eprintln!("[CONNECT] ↻ Will retry in 5 seconds...");     │
│     bootstrap_retry_timer = now + 5s;                        │
│ }                                                            │
└─────────────────────────────────────────────────────────────┘
                          ↓
                    [Wait 5 seconds]
                          ↓
┌─────────────────────────────────────────────────────────────┐
│ shard_listener.rs:2482                                       │
│ tokio::select! retry branch                                  │
│   ↓                                                          │
│ println!("[CONNECT] ↻ Retrying bootstrap connection...");   │
│ swarm.dial(bootstrap_addr.clone())?                         │
│ bootstrap_retry_timer = now + 5s;                           │
└─────────────────────────────────────────────────────────────┘
                          ↓
                    [Cycle repeats]
```

---

## Key Variables

### `bootstrap_connected` (line 890)
- **Type:** `bool`
- **Initial:** `false`
- **Set to `true`:** Line 965 when `ConnectionEstablished` event fires for bootstrap
- **Set to `false`:** Line 1060 when bootstrap connection closes
- **Purpose:** Prevents duplicate retry attempts when already connected

### `bootstrap_retry_timer` (line 891)
- **Type:** `tokio::time::Instant`
- **Initial:** `now + 5 seconds`
- **Updated:** Lines 2463, 2472, 2487, 1062
- **Purpose:** Triggers retry branch in `tokio::select!` when expired

### `bootstrap_addr` (line 884)
- **Type:** `Multiaddr`
- **Value:** `/ip4/162.221.207.169/udp/51820/quic-v1`
- **Purpose:** Target address for bootstrap connection

---

## Diagnostic Steps

### 1. Verify Server is Running
```bash
ssh dbertrand@eagleoneonline.ca 'ps aux | grep "./target/release/server"'
```

### 2. Check Server is Listening
```bash
ssh dbertrand@eagleoneonline.ca 'ss -uln | grep 51820'
```
Should show: `UNCONN 0 0 0.0.0.0:51820`

### 3. Check Server Logs
```bash
ssh dbertrand@eagleoneonline.ca 'tail -f /home/dbertrand/punch-simple/server.log'
```
Look for:
- `[SERVER] Listening on /ip4/0.0.0.0/udp/51820/quic-v1`
- `[SERVER] ✓ Connection established from peer: ...`

### 4. Check Firewall
```bash
ssh dbertrand@eagleoneonline.ca 'sudo ufw status | grep 51820'
```
Should allow: `162.221.207.169`

### 5. Test UDP Connectivity
```bash
# From local machine
Test-NetConnection -ComputerName eagleoneonline.ca -Port 51820
```
Note: UDP is connectionless, so this test is limited.

---

## Recommended Fixes

### Fix 1: Add QUIC Handshake Timeout Configuration

**File:** `src/quic_transport.rs`

```rust
pub fn create_quic_transport(
    keypair: &Keypair,
) -> Result<libp2p::core::transport::Boxed<(PeerId, libp2p::core::muxing::StreamMuxerBox)>, TransportError> {
    let mut quic_config = quic::Config::new(keypair);
    
    // Configure handshake timeout (default is ~3s, increase to 10s)
    // Note: This may require accessing underlying Quinn config
    // libp2p's quic::Config may not expose all Quinn settings directly
    
    let transport = quic::tokio::Transport::new(quic_config)
        .map(|(peer_id, muxer), _| (peer_id, libp2p::core::muxing::StreamMuxerBox::new(muxer)))
        .boxed();
    
    Ok(transport)
}
```

**Note:** libp2p's `quic::Config` may not expose Quinn's timeout settings directly. May need to check libp2p version and available configuration options.

### Fix 2: Enhanced Error Logging

**File:** `src/shard_listener.rs:2456`

```rust
SwarmEvent::OutgoingConnectionError { error, peer_id: failed_peer, .. } => {
    if !bootstrap_connected {
        eprintln!("[CONNECT] ⚠️  Bootstrap connection failed: {:?}", error);
        
        // Enhanced error details
        if let libp2p::swarm::DialError::Transport(errors) = &error {
            for (addr, transport_error) in errors {
                eprintln!("[CONNECT]   Address: {}", addr);
                eprintln!("[CONNECT]   Transport Error: {:?}", transport_error);
                
                // Check for specific QUIC errors
                if format!("{:?}", transport_error).contains("HandshakeTimedOut") {
                    eprintln!("[CONNECT]   QUIC handshake timed out - possible causes:");
                    eprintln!("[CONNECT]     - Server not running or not listening on QUIC");
                    eprintln!("[CONNECT]     - Firewall blocking UDP packets");
                    eprintln!("[CONNECT]     - Network connectivity issues");
                    eprintln!("[CONNECT]     - Server transport type mismatch");
                }
            }
        }
        
        eprintln!("[CONNECT] ↻ Will retry in 5 seconds...");
        bootstrap_retry_timer = tokio::time::Instant::now() + Duration::from_secs(5);
    }
}
```

### Fix 3: Add Connection State Logging

**File:** `src/shard_listener.rs:2483`

```rust
_ = tokio::time::sleep_until(bootstrap_retry_timer), if !bootstrap_connected => {
    let attempt_count = // Track retry attempts
    println!("[CONNECT] ↻ Retrying bootstrap connection... (attempt {})", attempt_count);
    println!("[CONNECT]   Target: {}", bootstrap_addr);
    println!("[CONNECT]   Transport: QUIC");
    
    if let Err(e) = swarm.dial(bootstrap_addr.clone()) {
        eprintln!("[CONNECT] ⚠️  Retry dial failed: {:?}", e);
    } else {
        println!("[CONNECT]   Dial initiated, waiting for handshake...");
    }
    bootstrap_retry_timer = tokio::time::Instant::now() + Duration::from_secs(5);
}
```

### Fix 4: Verify Server Before Retrying

Add a check to verify server is actually running before retrying:

```rust
// Before retry, check if server is reachable
// (This would require additional network check or server status endpoint)
```

---

## Current Status

- ✅ **Retry mechanism:** Working correctly
- ✅ **Error logging:** Captures HandshakeTimedOut
- ⚠️ **Error details:** Limited (just prints `{:?}`)
- ❌ **Root cause:** Server likely not running or not listening on QUIC
- ⚠️ **Timeout config:** Using defaults (no explicit configuration)

---

## Next Steps

1. **Verify server is running** with QUIC transport
2. **Check server logs** for listening confirmation
3. **Verify firewall** allows client IP
4. **Add enhanced logging** to diagnose handshake step failure
5. **Consider increasing timeout** if network is slow
