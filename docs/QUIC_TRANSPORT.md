# QUIC Transport for Promethos-AI Swarm

## Overview

This document describes the QUIC transport implementation for the Promethos-AI Swarm distributed inference network. QUIC provides significant advantages over TCP for P2P networking.

## Why QUIC?

### Protocol Comparison

| Feature | TCP + Noise + Yamux | QUIC |
|---------|---------------------|------|
| Connection Setup | 3-way handshake + TLS + Yamux | 0-RTT / 1-RTT with built-in TLS |
| Encryption | Noise protocol (separate layer) | TLS 1.3 (built-in) |
| Multiplexing | Yamux (separate layer) | Native stream multiplexing |
| Transport | TCP (reliable, ordered) | UDP (with reliability) |
| NAT Traversal | Port mapping required | Hole punching friendly |
| Head-of-Line Blocking | Yes (at TCP level) | No (per-stream) |
| Connection Migration | No | Yes (survives IP changes) |

### Performance Benefits

1. **Faster Connection Establishment**
   - TCP+Noise+Yamux: ~3-5 round trips
   - QUIC: 1 round trip (0-RTT for resumed connections)

2. **No Head-of-Line Blocking**
   - TCP: A lost packet blocks ALL streams
   - QUIC: Lost packet only blocks that specific stream

3. **Better NAT Traversal**
   - UDP-based QUIC works better with residential NATs
   - Supports connection migration when IP changes

## Usage

### Transport Types

```rust
use punch_simple::quic_transport::{create_transport, TransportType};
use libp2p::identity::Keypair;

let key = Keypair::generate_ed25519();

// QUIC only (recommended for new deployments)
let transport = create_transport(&key, TransportType::QuicOnly)?;

// TCP only (legacy compatibility)
let transport = create_transport(&key, TransportType::TcpOnly)?;

// Dual-stack: QUIC preferred, TCP fallback (recommended for mixed networks)
let transport = create_transport(&key, TransportType::DualStack)?;
```

### Listen Addresses

```rust
use punch_simple::quic_transport::{get_listen_address, get_dual_listen_addresses};

// QUIC listen address
let quic_addr = get_listen_address(TransportType::QuicOnly, 51820);
// Result: "/ip4/0.0.0.0/udp/51820/quic-v1"

// TCP listen address
let tcp_addr = get_listen_address(TransportType::TcpOnly, 51820);
// Result: "/ip4/0.0.0.0/tcp/51820"

// Dual-stack addresses
let (quic, tcp) = get_dual_listen_addresses(51820);
// QUIC: "/ip4/0.0.0.0/udp/51820/quic-v1"
// TCP:  "/ip4/0.0.0.0/tcp/51820"
```

### Command Line Configuration

Nodes can be configured via command line or environment variables:

```bash
# QUIC-only node
listener --transport quic --port 51820

# TCP-only node (legacy)
listener --transport tcp --port 51820

# Dual-stack node (default, recommended)
listener --transport dual --port 51820
```

Environment variables:
```bash
export TRANSPORT_TYPE=quic
export LISTEN_PORT=51820
```

## Implementation Details

### Module Structure

```
src/
├── quic_transport.rs     # Transport creation and configuration
├── lib.rs                # Module exports
└── ...

tests/
└── transport_tests.rs    # Comprehensive transport tests (18 tests)
```

### Key Functions

| Function | Description |
|----------|-------------|
| `create_quic_transport()` | Creates a QUIC-only transport |
| `create_tcp_transport()` | Creates a TCP+Noise+Yamux transport |
| `create_dual_transport()` | Creates a dual-stack transport |
| `create_transport()` | Creates transport by type enum |
| `get_listen_address()` | Gets appropriate listen address |
| `get_dual_listen_addresses()` | Gets both QUIC and TCP addresses |

### Transport Statistics

```rust
use punch_simple::quic_transport::TransportStats;

let mut stats = TransportStats::new();
stats.quic_connections = 100;
stats.tcp_connections = 25;

println!("Total connections: {}", stats.total_connections());  // 125
println!("QUIC ratio: {:.1}%", stats.quic_ratio() * 100.0);   // 80.0%
```

## Testing

The transport implementation includes comprehensive tests:

```bash
# Run all transport tests
cargo test --test transport_tests

# Run specific test
cargo test test_quic_peer_connection
```

### Test Categories

1. **Unit Tests** (6 tests)
   - Transport type parsing
   - Address generation
   - Statistics tracking

2. **Transport Creation** (4 tests)
   - QUIC transport creation
   - TCP transport creation
   - Dual transport creation
   - All transport types

3. **Swarm Integration** (6 tests)
   - TCP swarm listen
   - QUIC swarm listen
   - Dual-stack listen (QUIC)
   - Dual-stack listen (TCP)
   - Peer connection tests

4. **Request/Response** (2 tests)
   - TCP request/response
   - QUIC request/response

5. **Stress Tests** (2 tests)
   - Multiple messages over TCP
   - Multiple messages over QUIC

6. **Regression Tests** (2 tests)
   - TCP not broken by QUIC addition
   - QUIC parallel to TCP

## Migration Guide

### From TCP-Only to Dual-Stack

1. **Update transport creation:**
```rust
// Before
let transport = create_tcp_transport(&key)?;

// After
let transport = create_dual_transport(&key)?;
```

2. **Add QUIC listen address:**
```rust
// Before
swarm.listen_on("/ip4/0.0.0.0/tcp/51820".parse()?)?;

// After
swarm.listen_on("/ip4/0.0.0.0/udp/51820/quic-v1".parse()?)?;
swarm.listen_on("/ip4/0.0.0.0/tcp/51820".parse()?)?;
```

3. **Update firewall rules:**
   - Allow UDP port 51820 (in addition to TCP)

### Full QUIC Migration

Once all nodes support QUIC, you can switch to QUIC-only:

```rust
let transport = create_quic_transport(&key)?;
swarm.listen_on("/ip4/0.0.0.0/udp/51820/quic-v1".parse()?)?;
```

## Firewall Configuration

### Required Ports

| Protocol | Port | Purpose |
|----------|------|---------|
| UDP | 51820 | QUIC transport |
| TCP | 51820 | TCP fallback |

### Example (Linux/ufw)

```bash
# Allow QUIC
sudo ufw allow 51820/udp

# Allow TCP fallback
sudo ufw allow 51820/tcp
```

### Example (Windows Firewall)

```powershell
# Allow QUIC
New-NetFirewallRule -DisplayName "Promethos QUIC" -Direction Inbound -Protocol UDP -LocalPort 51820 -Action Allow

# Allow TCP
New-NetFirewallRule -DisplayName "Promethos TCP" -Direction Inbound -Protocol TCP -LocalPort 51820 -Action Allow
```

## Troubleshooting

### QUIC Connection Fails

1. **Check firewall:** Ensure UDP port is open
2. **NAT type:** Some strict NATs block UDP
3. **Fallback:** Use `TransportType::DualStack` for TCP fallback

### High Latency

1. **Check MTU:** QUIC performance degrades with small MTU
2. **Congestion:** QUIC uses different congestion control

### Connection Drops

1. **NAT timeout:** QUIC keep-alives may be needed
2. **Connection migration:** Ensure libp2p QUIC is up to date

## References

- [libp2p QUIC Specification](https://github.com/libp2p/specs/tree/master/quic)
- [Quinn QUIC Implementation](https://github.com/quinn-rs/quinn)
- [QUIC RFC 9000](https://datatracker.ietf.org/doc/html/rfc9000)







