# QUIC Handshake Protocol Analysis

## Overview
This document analyzes the QUIC handshake protocol from both server and client sides to diagnose connection issues.

## Handshake Flow

### Server Side (Rendezvous Server)

1. **Transport Creation** (`src/server.rs:297`)
   ```rust
   let transport = create_transport(&local_key, transport_type)?;
   ```
   - For `TransportType::QuicOnly`: Creates QUIC transport using `quic::Config::new(keypair)`
   - QUIC transport uses libp2p's QUIC implementation (based on Quinn)
   - Built-in TLS 1.3 encryption (no separate Noise handshake)

2. **Listen Address Setup** (`src/server.rs:370-383`)
   ```rust
   match transport_type {
       TransportType::DualStack => {
           let (quic, tcp) = get_dual_listen_addresses(port);
           let quic_addr: Multiaddr = quic.replace("0.0.0.0", &listen_addr).parse()?;
           let tcp_addr: Multiaddr = tcp.replace("0.0.0.0", &listen_addr).parse()?;
           swarm.listen_on(quic_addr)?;
           swarm.listen_on(tcp_addr)?;
       }
       other => {
           let addr: Multiaddr =
               get_listen_address(other, port).replace("0.0.0.0", &listen_addr).parse()?;
           swarm.listen_on(addr)?;
       }
   }
   ```
   - **Issue**: When `listen_addr` is "0.0.0.0", the server listens on all interfaces (correct)
   - **Issue**: But when printing bootstrap address for clients (line 387), it uses `listen_addr` which is "0.0.0.0" - **this is wrong!**
   - Clients cannot connect to "0.0.0.0" - they need the actual IP address

3. **Event Loop** (`src/server.rs:392-404`)
   - `SwarmEvent::NewListenAddr`: Server logs when it starts listening
   - `SwarmEvent::ConnectionEstablished`: Server logs when client connects
   - **Missing**: No detailed logging of QUIC handshake steps or errors

### Client Side (Shard Listener)

1. **Transport Creation** (`src/shard_listener.rs:820-830`)
   ```rust
   let transport = create_transport(&peer_key, transport_type)?;
   ```
   - Same as server: Creates QUIC transport using `quic::Config::new(keypair)`

2. **Listen Address Setup** (`src/shard_listener.rs:851-873`)
   ```rust
   match transport_type {
       TransportType::QuicOnly => {
           let quic_addr = format!("/ip4/0.0.0.0/udp/{}/quic-v1", port);
           let listen_addr: Multiaddr = quic_addr.parse()?;
           swarm.listen_on(listen_addr)?;
       }
       // ...
   }
   ```
   - Client listens on `0.0.0.0` (all interfaces) - correct for client

3. **Bootstrap Connection** (`src/shard_listener.rs:884`)
   ```rust
   swarm.dial(bootstrap_addr.clone())?;
   ```
   - Client dials: `/ip4/162.221.207.169/udp/51820/quic-v1`
   - This triggers QUIC handshake

4. **Error Handling** (`src/shard_listener.rs:2456-2475`)
   ```rust
   SwarmEvent::OutgoingConnectionError { error, peer_id: failed_peer, .. } => {
       if !bootstrap_connected {
           eprintln!("[CONNECT] ⚠️  Bootstrap connection failed: {:?}", error);
           eprintln!("[CONNECT] ↻ Will retry in 5 seconds...");
           bootstrap_retry_timer = tokio::time::Instant::now() + Duration::from_secs(5);
       }
   }
   ```
   - Logs `HandshakeTimedOut` errors
   - Retries every 5 seconds

## QUIC Handshake Process (libp2p/Quinn)

1. **Client Initiates** (Client → Server)
   - Client sends QUIC Initial packet (UDP)
   - Contains TLS 1.3 ClientHello
   - Includes destination connection ID (DCID)

2. **Server Responds** (Server → Client)
   - Server sends QUIC Initial packet (UDP)
   - Contains TLS 1.3 ServerHello
   - Includes source connection ID (SCID)

3. **Handshake Completion**
   - TLS 1.3 handshake completes
   - Connection established
   - libp2p identifies peer via certificate

## Identified Issues

### Issue 1: Server Not Running
- **Symptom**: `HandshakeTimedOut` errors
- **Cause**: Server process not running
- **Fix**: Start server with QUIC transport

### Issue 2: Firewall Blocking UDP
- **Symptom**: No response from server
- **Cause**: Firewall rules only allow specific IPs
- **Fix**: Add client IP to firewall allow list
- **Status**: ✅ Fixed (IP 162.221.207.169 added)

### Issue 3: Address Mismatch
- **Symptom**: Server listening on wrong interface
- **Cause**: Server prints "0.0.0.0" as bootstrap address
- **Impact**: Clients can't connect if they use printed address
- **Fix**: Server should print actual external IP, not "0.0.0.0"

### Issue 4: Transport Type Mismatch
- **Symptom**: Handshake fails
- **Cause**: Server and client must use compatible transports
   - Server: `quic` → Client: `quic` or `dual` ✅
   - Server: `dual` → Client: `quic` or `dual` ✅
   - Server: `tcp` → Client: `quic` ❌ (incompatible)

### Issue 5: Missing Handshake Logging
- **Symptom**: Hard to diagnose handshake failures
- **Cause**: No detailed QUIC handshake logging
- **Impact**: Can't see which step fails (Initial packet, TLS handshake, etc.)

## Diagnostic Steps

1. **Verify Server is Running**
   ```bash
   ssh dbertrand@eagleoneonline.ca 'ps aux | grep "./target/release/server"'
   ```

2. **Check Server Listen Address**
   ```bash
   ssh dbertrand@eagleoneonline.ca 'ss -ulnp | grep 51820'
   ```
   Should show: `UNCONN 0 0 0.0.0.0:51820`

3. **Check Firewall Rules**
   ```bash
   ssh dbertrand@eagleoneonline.ca 'sudo ufw status | grep 51820'
   ```
   Should include client IP: `162.221.207.169`

4. **Check Server Logs**
   ```bash
   ssh dbertrand@eagleoneonline.ca 'tail -f /home/dbertrand/punch-simple/server.log'
   ```
   Look for:
   - `[SERVER] Listening on /ip4/0.0.0.0/udp/51820/quic-v1`
   - `[SERVER] ✓ Connection established from peer: ...`

5. **Check Client Logs**
   Look for:
   - `[CONNECT] Bootstrap address: /ip4/162.221.207.169/udp/51820/quic-v1`
   - `[CONNECT] ⚠️  Bootstrap connection failed: Transport([...HandshakeTimedOut...])`

## Recommended Fixes

### Fix 1: Server Address Reporting
Modify `src/server.rs:387` to use actual external IP instead of `listen_addr`:
```rust
// Get external IP or use listen_addr if it's not 0.0.0.0
let external_ip = if listen_addr == "0.0.0.0" {
    // Try to get external IP (or use a configurable value)
    std::env::var("EXTERNAL_IP").unwrap_or_else(|_| "YOUR_EXTERNAL_IP".to_string())
} else {
    listen_addr.clone()
};
println!("  --bootstrap /ip4/{}/udp/{}/quic-v1  (QUIC)", external_ip, port);
```

### Fix 2: Enhanced Handshake Logging
Add detailed QUIC handshake logging:
```rust
SwarmEvent::OutgoingConnectionError { error, peer_id, .. } => {
    eprintln!("[QUIC_HANDSHAKE] Connection error details:");
    eprintln!("  Error: {:?}", error);
    eprintln!("  Peer: {:?}", peer_id);
    // Log transport-specific errors
    if let libp2p::swarm::DialError::Transport(errors) = &error {
        for (addr, err) in errors {
            eprintln!("  Address: {}", addr);
            eprintln!("  Transport Error: {:?}", err);
        }
    }
}
```

### Fix 3: Server Startup Verification
Add startup verification to ensure server is actually listening:
```rust
// After swarm.listen_on()
tokio::time::sleep(Duration::from_secs(1)).await;
let listen_addrs: Vec<_> = swarm.listeners().collect();
if listen_addrs.is_empty() {
    eprintln!("[ERROR] Server failed to start listening!");
    return Err("Failed to start listening".into());
}
println!("[SERVER] Successfully listening on {} addresses", listen_addrs.len());
```

## Current Status

- ✅ Firewall rule added for client IP
- ❌ Server not running (needs to be started)
- ⚠️ Server address reporting uses "0.0.0.0" (should use external IP)
- ⚠️ Limited handshake error logging

## Next Steps

1. Start server with QUIC transport
2. Verify server is listening on UDP 51820
3. Test client connection
4. If still failing, add enhanced logging to diagnose handshake step
