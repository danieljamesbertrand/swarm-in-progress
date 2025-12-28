# Testbed Guide for Kademlia P2P Implementation

This guide outlines the best testbed setup for testing and developing the Kademlia P2P implementation.

## Current Implementation Status

### ✅ What's Working
- **Kademlia DHT Integration**: Fully implemented
- **Three Binaries**: `server`, `listener`, `dialer` - all functional
- **Client Helper API**: `P2PClient` ready for integration
- **Compilation**: Code compiles successfully

### ⚠️ What Needs Testing
- End-to-end peer discovery
- Message exchange reliability
- Multiple peer scenarios
- Network failure recovery
- DHT bootstrap stability

## Recommended Testbed Setup

### Option 1: Manual Testing (Current Best Approach)

**Best for**: Initial development, debugging, understanding behavior

#### Setup

**Terminal 1 - Bootstrap Node:**
```bash
cd c:\Users\dan\punch-simple
cargo run --release --bin server -- --listen-addr 0.0.0.0 --port 51820
```

**Terminal 2 - Listener (Peer A):**
```bash
cargo run --release --bin listener -- \
  --bootstrap /ip4/127.0.0.1/tcp/51820 \
  --namespace test-room
```

**Terminal 3 - Dialer (Peer B):**
```bash
cargo run --release --bin dialer -- \
  --bootstrap /ip4/127.0.0.1/tcp/51820 \
  --namespace test-room
```

**Terminal 4 - Additional Peer (Optional):**
```bash
cargo run --release --bin listener -- \
  --bootstrap /ip4/127.0.0.1/tcp/51820 \
  --namespace test-room
```

#### Advantages
- ✅ Real-time observation of behavior
- ✅ Easy to debug with verbose output
- ✅ Can test edge cases manually
- ✅ No additional dependencies

#### Disadvantages
- ❌ Manual process
- ❌ Hard to automate
- ❌ Limited scalability testing

### Option 2: Automated Integration Tests

**Best for**: Regression testing, CI/CD, reliability validation

#### Create Test Infrastructure

**File: `tests/integration_test.rs`**

```rust
use std::time::Duration;
use tokio::time::sleep;

#[tokio::test]
async fn test_basic_peer_discovery() {
    // Test that peers can discover each other
    // This would require spawning separate processes
    // or using the client helper API
}

#[tokio::test]
async fn test_message_exchange() {
    // Test that messages can be sent and received
}
```

**File: `tests/test_harness.rs`**

```rust
// Test harness that spawns bootstrap, listener, and dialer
// and validates they can communicate
```

#### Advantages
- ✅ Automated and repeatable
- ✅ Can run in CI/CD
- ✅ Catches regressions

#### Disadvantages
- ❌ More complex to set up
- ❌ Requires process management
- ❌ Harder to debug failures

### Option 3: Docker Compose Testbed

**Best for**: Multi-machine simulation, network isolation testing

#### Setup

**File: `docker-compose.test.yml`**

```yaml
version: '3.8'

services:
  bootstrap:
    build: .
    command: ["server", "--listen-addr", "0.0.0.0", "--port", "51820"]
    ports:
      - "51820:51820"
    networks:
      - p2p-network

  listener1:
    build: .
    command: ["listener", "--bootstrap", "/ip4/bootstrap/tcp/51820", "--namespace", "test"]
    depends_on:
      - bootstrap
    networks:
      - p2p-network

  listener2:
    build: .
    command: ["listener", "--bootstrap", "/ip4/bootstrap/tcp/51820", "--namespace", "test"]
    depends_on:
      - bootstrap
    networks:
      - p2p-network

  dialer:
    build: .
    command: ["dialer", "--bootstrap", "/ip4/bootstrap/tcp/51820", "--namespace", "test"]
    depends_on:
      - bootstrap
    networks:
      - p2p-network

networks:
  p2p-network:
    driver: bridge
```

#### Advantages
- ✅ Isolated network environment
- ✅ Easy to scale (add more peers)
- ✅ Reproducible across machines
- ✅ Can test network partitions

#### Disadvantages
- ❌ Requires Docker setup
- ❌ More overhead
- ❌ Harder to debug

### Option 4: Unit Tests with Mocks

**Best for**: Testing individual components, logic validation

#### Setup

**File: `src/client_helper.rs` (add tests module)**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_peer_id_generation() {
        // Test peer ID creation
    }
    
    #[tokio::test]
    async fn test_namespace_key_generation() {
        // Test DHT key creation from namespace
    }
}
```

#### Advantages
- ✅ Fast execution
- ✅ Isolated testing
- ✅ Easy to debug
- ✅ No network required

#### Disadvantages
- ❌ Doesn't test real network behavior
- ❌ Requires mocking libp2p

## Recommended Testbed: Hybrid Approach

### Phase 1: Manual Testing (Current)
**Use**: Manual terminal-based testing
**Why**: Fastest way to validate basic functionality

### Phase 2: Scripted Testing
**Use**: Shell/PowerShell scripts to automate manual tests
**Why**: Balance between automation and simplicity

### Phase 3: Integration Tests
**Use**: Rust integration tests with process spawning
**Why**: Automated regression testing

## Quick Test Script

### Windows PowerShell Test Script

**File: `test.ps1`**

```powershell
# Start bootstrap node in background
Start-Process powershell -ArgumentList "-NoExit", "-Command", "cd '$PWD'; cargo run --release --bin server"
Start-Sleep -Seconds 3

# Start listener
Start-Process powershell -ArgumentList "-NoExit", "-Command", "cd '$PWD'; cargo run --release --bin listener -- --namespace test"
Start-Sleep -Seconds 5

# Start dialer
Start-Process powershell -ArgumentList "-NoExit", "-Command", "cd '$PWD'; cargo run --release --bin dialer -- --namespace test"

Write-Host "Test environment started. Check the windows for results."
```

### Linux/Mac Bash Test Script

**File: `test.sh`**

```bash
#!/bin/bash

# Start bootstrap node
cargo run --release --bin server &
SERVER_PID=$!
sleep 3

# Start listener
cargo run --release --bin listener -- --namespace test &
LISTENER_PID=$!
sleep 5

# Start dialer
cargo run --release --bin dialer -- --namespace test &
DIALER_PID=$!

echo "Test environment started. PIDs:"
echo "  Server: $SERVER_PID"
echo "  Listener: $LISTENER_PID"
echo "  Dialer: $DIALER_PID"
echo ""
echo "Press Ctrl+C to stop all processes"

# Wait for interrupt
trap "kill $SERVER_PID $LISTENER_PID $DIALER_PID; exit" INT
wait
```

## Test Scenarios

### Scenario 1: Basic Connectivity
1. Start bootstrap node
2. Start listener
3. Start dialer
4. **Expected**: Dialer discovers and connects to listener

### Scenario 2: Multiple Peers
1. Start bootstrap node
2. Start 3 listeners
3. Start 1 dialer
4. **Expected**: Dialer can discover all listeners

### Scenario 3: Namespace Isolation
1. Start bootstrap node
2. Start listener in namespace "room-1"
3. Start listener in namespace "room-2"
4. Start dialer in namespace "room-1"
5. **Expected**: Dialer only finds listener in "room-1"

### Scenario 4: Bootstrap Failure
1. Start listener (no bootstrap node)
2. **Expected**: Graceful failure or retry

### Scenario 5: Message Exchange
1. Start bootstrap, listener, dialer
2. Wait for connection
3. **Expected**: Messages are exchanged successfully

## Current Best Testbed Recommendation

### For Immediate Testing: **Manual Terminal Setup**

**Why:**
- ✅ Already working
- ✅ No additional setup needed
- Best for debugging
- ✅ Fast iteration cycle

**Steps:**
1. Open 3-4 terminal windows
2. Run bootstrap, listener, dialer in separate terminals
3. Observe behavior and logs
4. Test different scenarios manually

### For Development: **Add Test Scripts**

**Why:**
- ✅ Automates repetitive setup
- ✅ Still allows manual observation
- ✅ Easy to modify

**Next Steps:**
1. Create `test.ps1` (Windows) or `test.sh` (Linux/Mac)
2. Use scripts to quickly spin up test environment
3. Manually verify results

### For Production: **Add Integration Tests**

**Why:**
- ✅ Automated validation
- ✅ CI/CD integration
- ✅ Regression prevention

**Future Work:**
1. Create `tests/` directory
2. Add integration tests
3. Set up CI/CD pipeline

## Testing Checklist

- [ ] Bootstrap node starts successfully
- [ ] Listener can bootstrap to DHT
- [ ] Dialer can bootstrap to DHT
- [ ] Dialer discovers listener in same namespace
- [ ] Peers in different namespaces don't see each other
- [ ] Messages can be sent and received
- [ ] Multiple peers can connect simultaneously
- [ ] Network handles peer disconnections
- [ ] DHT recovers from bootstrap node failure
- [ ] Performance with 10+ peers

## Debugging Tips

### Enable Verbose Logging

Add to your code:
```rust
env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("debug")).init();
```

### Check DHT State

Add logging to see DHT operations:
```rust
match event {
    BehaviourEvent::Kademlia(e) => {
        println!("[DHT] {:?}", e);
    }
}
```

### Monitor Connections

Watch for connection events:
```rust
SwarmEvent::ConnectionEstablished { peer_id, .. } => {
    println!("[CONN] Established: {}", peer_id);
}
```

## Conclusion

**Current Best Testbed**: Manual terminal-based testing with 3-4 terminals

**Recommended Next Steps**:
1. Create test scripts for automation
2. Add integration tests for regression testing
3. Consider Docker setup for complex scenarios

The manual approach gives you the best visibility into what's happening and is the fastest way to iterate during development.


