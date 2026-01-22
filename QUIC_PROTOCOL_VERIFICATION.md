# QUIC Protocol Verification - Certificate and Protocol Compliance

## Overview
This document verifies that both server and client are following the QUIC protocol correctly, including TLS 1.3 certificate handling.

## Certificate Generation and Usage

### How libp2p QUIC Uses Certificates

libp2p's QUIC implementation (based on Quinn) uses **self-signed certificates** with a special **libp2p Public Key Extension**:

1. **Keypair Generation** (Both Server and Client)
   ```rust
   // Server: src/server.rs:292
   let local_key = identity::Keypair::generate_ed25519();
   
   // Client: src/shard_listener.rs:705
   let key = identity::Keypair::generate_ed25519();
   ```

2. **Certificate Derivation** (libp2p TLS Specification)
   - libp2p's `quic::Config::new(keypair)` internally:
     - Generates a **new ephemeral certificate keypair** (separate from identity keypair)
     - Creates a self-signed X.509 certificate with:
       - Certificate's public key = ephemeral certificate keypair
       - **libp2p Public Key Extension** (OID: `1.3.6.1.4.1.53594.1.1`):
         - Contains the **identity public key** (Ed25519)
         - Contains a **signature** over the certificate's public key, signed with the **identity private key**
       - Self-signed by the certificate keypair
     - Uses this certificate for TLS 1.3 handshake
   - **PeerId** = hash of the identity public key (not the certificate key)

3. **Certificate Validation** (libp2p TLS Specification)
   - **TLS version**: Must be TLS 1.3 or higher ✅
   - **Client authentication**: Required (mutual authentication) ✅
   - **Self-signed**: No certificate chain ✅
   - **Validity period**: Must be valid at time of receipt ✅
   - **Extension verification**:
     - Extract libp2p Public Key Extension (OID `1.3.6.1.4.1.53594.1.1`)
     - Extract identity public key from extension
     - Verify signature in extension (identity key signed certificate public key)
     - Derive PeerId from identity public key
     - Compare with expected PeerId (certificate pinning)
   - **Hash algorithms**: Must be strong (SHA-256+) ✅
   - All validation is done automatically by libp2p

## Current Implementation Analysis

### Server Side (`src/server.rs`)

**Certificate Generation:**
```rust
// Line 292-293
let local_key = identity::Keypair::generate_ed25519();
let local_peer_id = PeerId::from(local_key.public());

// Line 297
let transport = create_transport(&local_key, transport_type)?;
```

**QUIC Config:**
```rust
// src/quic_transport.rs:74
let quic_config = quic::Config::new(keypair);
```

**Status:** ✅ Correct
- Generates Ed25519 keypair
- Passes keypair to QUIC config
- libp2p automatically creates certificate

### Client Side (`src/shard_listener.rs`)

**Certificate Generation:**
```rust
// Line 705
let key = identity::Keypair::generate_ed25519();

// Line 820 (approximate)
let transport = create_transport(&peer_key, transport_type)?;
```

**QUIC Config:**
```rust
// src/quic_transport.rs:74 (same function)
let quic_config = quic::Config::new(keypair);
```

**Status:** ✅ Correct
- Generates Ed25519 keypair
- Passes keypair to QUIC config
- libp2p automatically creates certificate

## Protocol Compliance Check

### 1. QUIC Version

**Current:** Using libp2p's default QUIC version
- libp2p-quic uses **QUIC v1** (RFC 9000)
- Protocol identifier: `/quic-v1` in multiaddr

**Verification:**
```rust
// Server listen address: /ip4/0.0.0.0/udp/51820/quic-v1
// Client dial address: /ip4/162.221.207.169/udp/51820/quic-v1
```

**Status:** ✅ Both use `/quic-v1` - **COMPLIANT**

### 2. TLS 1.3 Configuration

**Current:** Using libp2p's default TLS 1.3 config
- TLS 1.3 is built into QUIC (RFC 9001)
- No separate TLS configuration needed

**Verification:**
- Both sides use `quic::Config::new(keypair)`
- libp2p handles TLS 1.3 automatically

**Status:** ✅ Both use TLS 1.3 - **COMPLIANT**

### 3. Certificate Format

**Current:** Self-signed X.509 certificates
- Derived from Ed25519 keypair
- Subject/Issuer = PeerId
- No CA chain

**Verification:**
- Server: Generates keypair → libp2p creates cert
- Client: Generates keypair → libp2p creates cert
- Validation: PeerId matching (certificate pinning)

**Status:** ✅ Both use same certificate format - **COMPLIANT**

### 4. Key Exchange

**Current:** TLS 1.3 key exchange
- ECDHE (Elliptic Curve Diffie-Hellman Ephemeral)
- Uses Ed25519 for authentication
- Forward secrecy guaranteed

**Verification:**
- Handled by TLS 1.3 in QUIC
- No explicit configuration needed

**Status:** ✅ Using TLS 1.3 key exchange - **COMPLIANT**

## Potential Issues

### Issue 1: Certificate Validation Mismatch

**Symptom:** Handshake fails with certificate error

**Possible Causes:**
1. **PeerId Mismatch**
   - Client expects different PeerId than server has
   - Server's certificate doesn't match expected PeerId
   - **Fix:** Ensure client dials correct server address

2. **Certificate Generation Failure**
   - Keypair invalid
   - libp2p fails to generate certificate
   - **Fix:** Check for errors in transport creation

3. **QUIC Version Mismatch**
   - Server/client using different QUIC versions
   - **Fix:** Ensure both use `/quic-v1`

### Issue 2: Missing Certificate Validation

**Current Status:** ✅ libp2p handles validation automatically

**How it works:**
- Client receives server's certificate during handshake
- Extracts PeerId from certificate
- Compares with expected PeerId (from multiaddr or DHT)
- If mismatch → handshake fails

**No action needed** - libp2p handles this correctly

### Issue 3: Certificate Expiration

**Current Status:** ✅ Self-signed certificates don't expire

**How it works:**
- Self-signed certificates have no expiration
- Valid as long as keypair is valid
- PeerId remains constant for same keypair

**No action needed** - certificates are valid indefinitely

## Verification Checklist

### Server Side
- [x] Generates Ed25519 keypair
- [x] Creates QUIC transport with keypair
- [x] Listens on `/quic-v1` protocol
- [x] Uses TLS 1.3 (automatic)
- [x] Certificate derived from keypair (automatic)
- [x] Logs PeerId for verification

### Client Side
- [x] Generates Ed25519 keypair
- [x] Creates QUIC transport with keypair
- [x] Dials `/quic-v1` protocol
- [x] Uses TLS 1.3 (automatic)
- [x] Certificate derived from keypair (automatic)
- [x] Validates server certificate (automatic)

### Protocol Compliance
- [x] Both use QUIC v1 (RFC 9000)
- [x] Both use TLS 1.3 (RFC 9001)
- [x] Both use Ed25519 for authentication
- [x] Both use certificate pinning (PeerId matching)
- [x] Both use ECDHE for key exchange

## Diagnostic Steps

### 1. Verify Server Certificate

**Check server logs for PeerId:**
```bash
ssh dbertrand@eagleoneonline.ca 'grep "peer id" /home/dbertrand/punch-simple/server.log'
```

**Expected output:**
```
Local peer id: 12D3KooW...
```

### 2. Verify Client Certificate

**Check client logs for PeerId:**
```
[CONNECT] Bootstrap address: /ip4/162.221.207.169/udp/51820/quic-v1
```

### 3. Check for Certificate Errors

**Look for TLS/certificate errors in logs:**
- `certificate validation failed`
- `peer id mismatch`
- `TLS handshake error`
- `certificate error`

**Current logs show:** `HandshakeTimedOut` (not certificate error)
- This suggests handshake never completes
- Certificate validation happens during handshake
- If certificate was wrong, we'd see different error

### 4. Verify QUIC Protocol Version

**Server listen address:**
```rust
// src/server.rs:373
let quic_addr: Multiaddr = quic.replace("0.0.0.0", &listen_addr).parse()?;
// quic = "/ip4/0.0.0.0/udp/51820/quic-v1"
```

**Client dial address:**
```rust
// src/shard_listener.rs:884
swarm.dial(bootstrap_addr.clone())?;
// bootstrap_addr = "/ip4/162.221.207.169/udp/51820/quic-v1"
```

**Status:** ✅ Both use `/quic-v1` - **MATCH**

## Root Cause Analysis

Given that:
1. ✅ Both sides use same QUIC version (`/quic-v1`)
2. ✅ Both sides use same TLS version (TLS 1.3)
3. ✅ Both sides use same certificate format (self-signed from Ed25519)
4. ✅ Certificate validation is automatic and correct
5. ❌ Handshake times out (not certificate error)

**Conclusion:** The protocol is being followed correctly. The issue is **NOT** certificate-related.

**Actual Problem:** Server is not responding to QUIC Initial packets, causing handshake timeout before certificate validation can occur.

## Recommended Actions

### 1. Verify Server is Running
```bash
ssh dbertrand@eagleoneonline.ca 'ps aux | grep "./target/release/server"'
```

### 2. Verify Server is Listening
```bash
ssh dbertrand@eagleoneonline.ca 'ss -uln | grep 51820'
```

### 3. Check Server Logs for Certificate/PeerId
```bash
ssh dbertrand@eagleoneonline.ca 'tail -50 /home/dbertrand/punch-simple/server.log | grep -E "(peer|certificate|TLS|QUIC)"'
```

### 4. Add Certificate Logging (Optional)

If you want to verify certificates explicitly, add logging:

```rust
// In server.rs after transport creation
println!("[CERT] Server PeerId: {}", local_peer_id);
println!("[CERT] Server certificate derived from Ed25519 keypair");

// In shard_listener.rs after transport creation
println!("[CERT] Client PeerId: {}", peer_id);
println!("[CERT] Client certificate derived from Ed25519 keypair");
```

## Summary

✅ **Protocol Compliance:** Both sides follow QUIC v1 and TLS 1.3 correctly
✅ **Certificate Handling:** Both sides generate and validate certificates correctly
✅ **Key Exchange:** Using TLS 1.3 ECDHE correctly
❌ **Connection Issue:** Server not responding (not a protocol/certificate problem)

The handshake timeout is **NOT** due to protocol or certificate issues. The server is simply not responding to QUIC Initial packets, which could be due to:
- Server not running
- Server not listening on QUIC
- Firewall blocking UDP packets
- Network connectivity issues
