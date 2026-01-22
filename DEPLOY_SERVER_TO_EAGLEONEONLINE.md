# Deploying Bootstrap Server to eagleoneonline.ca

## Current Local Server Features

The local `src/server.rs` implementation includes:

### ✅ Key Features
1. **QUIC Transport Support** - Full QUIC/QUIC-v1 support
2. **Dual-Stack Transport** - Can listen on both QUIC and TCP simultaneously
3. **Kademlia DHT Bootstrap** - Acts as bootstrap node for the DHT network
4. **Relay Protocol** - NAT traversal support for peers behind firewalls
5. **Identify Protocol** - Allows peers to discover server's addresses
6. **Ping Keepalive** - Maintains connections with periodic pings
7. **Connection Tracking** - Logs all connections and disconnections

### Current Implementation
- **File**: `src/server.rs`
- **Default Transport**: Dual-stack (QUIC + TCP)
- **Default Port**: 51820
- **Default Listen Address**: 0.0.0.0 (all interfaces)

## Deployment Steps

**Note:** The remote server is Ubuntu and has its own Rust toolchain, so we should build on the server rather than cross-compiling.

### Prerequisites (Ubuntu Server)

1. **Ensure Rust is installed:**
   ```bash
   # Check if Rust is installed
   rustc --version
   cargo --version
   
   # If not installed:
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   source $HOME/.cargo/env
   ```

2. **Install build dependencies (if needed):**
   ```bash
   sudo apt-get update
   sudo apt-get install -y build-essential pkg-config libssl-dev
   ```

### Option 1: Deploy Source Code and Build on Server (Recommended)

1. **SSH to remote Ubuntu server:**
   ```bash
   ssh user@eagleoneonline.ca
   ```

2. **Navigate to project directory:**
   ```bash
   cd ~/punch-simple  # or wherever the project is located
   ```

3. **Pull latest code:**
   ```bash
   git pull origin main  # or your branch
   # OR if first time:
   # git clone <your-repo-url> ~/punch-simple
   # cd ~/punch-simple
   ```

4. **Build on remote server (using its Rust toolchain):**
   ```bash
   cargo build --release --bin server
   ```

5. **Run the server (QUIC-only mode):**
   ```bash
   ./target/release/server --listen-addr 0.0.0.0 --port 51820 --transport quic
   ```

### Option 2: Build and Deploy with Systemd Service (Ubuntu)

1. **SSH to server, pull code, and build:**
   ```bash
   ssh user@eagleoneonline.ca
   cd ~/punch-simple
   git pull origin main
   cargo build --release --bin server
   ```

2. **Create systemd service file:**
   ```bash
   sudo nano /etc/systemd/system/punch-bootstrap.service
   ```
   
   Add this content (adjust paths as needed):
   ```ini
   [Unit]
   Description=Punch Simple Bootstrap Server
   After=network.target

   [Service]
   Type=simple
   User=your-username
   WorkingDirectory=/home/your-username/punch-simple
   ExecStart=/home/your-username/punch-simple/target/release/server --listen-addr 0.0.0.0 --port 51820 --transport quic
   Restart=always
   RestartSec=10
   StandardOutput=journal
   StandardError=journal

   [Install]
   WantedBy=multi-user.target
   ```

3. **Enable and start the service:**
   ```bash
   sudo systemctl daemon-reload
   sudo systemctl enable punch-bootstrap
   sudo systemctl start punch-bootstrap
   sudo systemctl status punch-bootstrap
   ```

4. **View logs:**
   ```bash
   sudo journalctl -u punch-bootstrap -f
   ```

### Option 3: Deploy via Git (if code is in repo)

1. **SSH to remote server:**
   ```bash
   ssh user@eagleoneonline.ca
   ```

2. **Clone/Pull latest code:**
   ```bash
   cd /path/to/punch-simple
   git pull origin main  # or your branch
   ```

3. **Build and run:**
   ```bash
   cargo build --release --bin server
   ./target/release/server --listen-addr 0.0.0.0 --port 51820 --transport quic
   ```

## Verification

### Check if Server is Running (Ubuntu)

**On Ubuntu server:**
```bash
# Check if process is running
ps aux | grep server

# Check if port is listening (UDP for QUIC)
sudo netstat -ulnp | grep 51820
# OR
sudo ss -ulnp | grep 51820

# Check systemd service status
sudo systemctl status punch-bootstrap
```

**From local Windows machine:**
```powershell
Test-NetConnection -ComputerName eagleoneonline.ca -Port 51820
```

### Test QUIC Connection
The server should accept QUIC connections on:
- `/ip4/eagleoneonline.ca/udp/51820/quic-v1`

## Important Notes

1. **Ubuntu Server**: The remote server is Ubuntu - use Ubuntu-specific commands
2. **Remote Rust Toolchain**: The server has its own Rust toolchain - build on the server, not locally
3. **Firewall (UFW)**: Ensure UDP port 51820 is open:
   ```bash
   sudo ufw allow 51820/udp
   sudo ufw status
   ```
4. **QUIC vs TCP**: The remote server is QUIC-only, so use `--transport quic`
5. **Dual-Stack**: If you want both QUIC and TCP, use `--transport dual` (default)
6. **Version Check**: Compare the remote server version with local code to ensure feature parity
7. **Build Time**: Building on the server may take several minutes depending on server resources
8. **Systemd Logs**: Use `journalctl -u punch-bootstrap -f` to monitor logs in real-time

## Version Comparison

To check if remote server needs update, compare:
- QUIC transport support
- Relay protocol support
- Kademlia DHT features
- Connection keepalive (ping) settings
- Error handling improvements
