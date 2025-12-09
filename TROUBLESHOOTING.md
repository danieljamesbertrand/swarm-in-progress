# Troubleshooting Connection Issues

## Common Problem: Connection Reset (Error 10054)

If you're seeing connection reset errors when trying to connect to the rendezvous server, the most common cause is **incorrect server binding**.

### ❌ WRONG - Server Binding to Specific IP

```bash
~/.cargo/bin/rndz server --listen-addr 162.221.207.169:51820
```

**Problem**: When you bind to a specific IP address, the server only accepts connections **to** that IP. External clients cannot connect.

### ✅ CORRECT - Server Binding to All Interfaces

```bash
~/.cargo/bin/rndz server --listen-addr 0.0.0.0:51820
```

**Solution**: Bind to `0.0.0.0` which means "listen on all network interfaces". This allows:
- Local connections (127.0.0.1)
- LAN connections (192.168.x.x)
- External connections (162.221.207.169)

## Verification Steps

### 1. Check if Server is Running

On the server machine:
```bash
ps aux | grep rndz
# or
netstat -tulpn | grep 51820
```

### 2. Test Network Connectivity

From your client machine:
```powershell
# Windows PowerShell
Test-NetConnection -ComputerName 162.221.207.169 -Port 51820

# Or using telnet (if available)
telnet 162.221.207.169 51820
```

### 3. Check Firewall

On the server machine, ensure port 51820 is open:
```bash
# Ubuntu/Debian
sudo ufw allow 51820/tcp
sudo ufw status

# Or check iptables
sudo iptables -L -n | grep 51820
```

### 4. Verify Server is Listening Correctly

On the server machine:
```bash
# Should show the server listening on 0.0.0.0:51820 (all interfaces)
netstat -tulpn | grep 51820
# Output should show: 0.0.0.0:51820 or :::51820
```

## Server Configuration

### Correct Server Startup

```bash
# On the server (162.221.207.169)
~/.cargo/bin/rndz server --listen-addr 0.0.0.0:51820
```

The server will:
- Listen on all network interfaces (0.0.0.0)
- Accept connections on port 51820
- Be accessible via the server's public IP (162.221.207.169)

### Client Configuration

The client code is already correct - it connects to:
```
162.221.207.169:51820
```

## Additional Troubleshooting

### If Connection Still Fails

1. **Check server logs**: Look for errors in the server output
2. **Check network routing**: Ensure packets can reach the server
3. **Check NAT/firewall**: If behind NAT, ensure port forwarding is configured
4. **Try different port**: Test with a well-known port like 80 or 443 to rule out firewall issues

### Network Architecture

```
Client (Your Machine)
    ↓ TCP Connection
    ↓ Port 51820
Internet
    ↓
Server Firewall (must allow port 51820)
    ↓
Server (162.221.207.169)
    ↓ Listening on 0.0.0.0:51820
Rendezvous Server Process
```

## Quick Fix Summary

**On the server machine, run:**
```bash
# Stop any existing rndz server
pkill rndz

# Start server with correct binding
~/.cargo/bin/rndz server --listen-addr 0.0.0.0:51820
```

**Then retry your client connection.**

