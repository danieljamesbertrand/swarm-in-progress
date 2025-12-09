# Diagnostic Steps for Error 10054

## Current Status
✅ **TCP connectivity works** - Port 51820 is reachable  
❌ **libp2p connection fails** - Connection reset during handshake

This means the network path is open, but the application layer is rejecting the connection.

## Possible Causes

### 1. Wrong Service Running on Port 51820
The port might be occupied by a different service, not the rndz server.

**Check on server:**
```bash
# See what's listening on port 51820
sudo lsof -i :51820
# or
sudo netstat -tulpn | grep 51820

# Check if rndz is actually running
ps aux | grep rndz
```

**Solution:** Stop any other service on port 51820, then start rndz server.

### 2. Server Not Running rndz
The rndz server might not be running at all.

**Check on server:**
```bash
# Check if rndz process exists
ps aux | grep rndz

# If not running, start it:
~/.cargo/bin/rndz server --listen-addr 0.0.0.0:51820
```

### 3. Protocol Mismatch
The server might be expecting a different protocol or version.

**Verify rndz version:**
```bash
# On server
~/.cargo/bin/rndz --version

# Check if it's the libp2p rendezvous server
~/.cargo/bin/rndz server --help
```

### 4. Server Configuration Issue
The server might need additional configuration.

**Try starting server with verbose logging:**
```bash
RUST_LOG=debug ~/.cargo/bin/rndz server --listen-addr 0.0.0.0:51820
```

## Step-by-Step Verification

### Step 1: Verify Server is Running
```bash
# SSH to server
ssh user@162.221.207.169

# Check for rndz process
ps aux | grep rndz

# Check what's on port 51820
sudo lsof -i :51820
```

### Step 2: Start/Restart Server Correctly
```bash
# Kill any existing rndz
pkill rndz

# Start with correct binding
~/.cargo/bin/rndz server --listen-addr 0.0.0.0:51820

# Verify it's listening
netstat -tulpn | grep 51820
# Should show: 0.0.0.0:51820
```

### Step 3: Test from Server Itself
```bash
# On the server, test local connection
# You might need to install a test client or use telnet
telnet localhost 51820
# Should connect (even if it doesn't speak libp2p, TCP should work)
```

### Step 4: Check Server Logs
When you start the server, it should show:
- Listening address
- Incoming connection attempts
- Any errors

Watch the server output when the client tries to connect.

## Quick Test Script

Create this on the server to verify everything:

```bash
#!/bin/bash
# test_rndz_server.sh

echo "Checking rndz server status..."

# Check if running
if pgrep -f "rndz server" > /dev/null; then
    echo "✅ rndz server is running"
    ps aux | grep rndz | grep -v grep
else
    echo "❌ rndz server is NOT running"
    echo "Starting server..."
    ~/.cargo/bin/rndz server --listen-addr 0.0.0.0:51820 &
    sleep 2
    if pgrep -f "rndz server" > /dev/null; then
        echo "✅ Server started"
    else
        echo "❌ Failed to start server"
    fi
fi

# Check port
echo ""
echo "Checking port 51820..."
if sudo lsof -i :51820 > /dev/null 2>&1; then
    echo "✅ Port 51820 is in use:"
    sudo lsof -i :51820
else
    echo "❌ Nothing listening on port 51820"
fi

# Check firewall
echo ""
echo "Checking firewall..."
if command -v ufw > /dev/null; then
    sudo ufw status | grep 51820 || echo "Port 51820 not explicitly allowed (may still work)"
fi
```

## Expected Server Output

When the server is running correctly, you should see something like:
```
Listening on /ip4/0.0.0.0/tcp/51820
Peer connected: 12D3KooW...
```

When a client connects, the server should log the connection attempt.

## If Still Failing

1. **Check rndz version compatibility** - Ensure server and client use compatible libp2p versions
2. **Try a different port** - Test with port 8080 or 9000 to rule out port-specific issues
3. **Check server firewall rules** - Even if port is open, firewall might be filtering packets
4. **Review server logs** - Look for any error messages when connection is attempted

