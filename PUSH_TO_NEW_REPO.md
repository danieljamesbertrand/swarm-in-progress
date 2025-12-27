# Push to New Repository: 16-node-burst-with-jsons-and-web-console

## Steps to Create and Push

### 1. Create the New Repository on GitHub/GitLab

**On GitHub:**
1. Go to https://github.com/new
2. Repository name: `16-node-burst-with-jsons-and-web-console`
3. Description: `Intensive 16-node Kademlia P2P network testbed with random JSON message load, latency tracking, and real-time web monitoring dashboard`
4. Choose Public or Private
5. **DO NOT** initialize with README, .gitignore, or license (we already have these)
6. Click "Create repository"

**On GitLab:**
1. Go to https://gitlab.com/projects/new
2. Project name: `16-node-burst-with-jsons-and-web-console`
3. Visibility: Public or Private
4. **DO NOT** initialize with README
5. Click "Create project"

### 2. Push to New Repository

After creating the repository, run these commands:

```powershell
# Remove old remote (if exists)
git remote remove origin

# Add new remote (replace USERNAME with your GitHub/GitLab username)
git remote add origin https://github.com/USERNAME/16-node-burst-with-jsons-and-web-console.git
# OR for GitLab:
# git remote add origin https://gitlab.com/USERNAME/16-node-burst-with-jsons-and-web-console.git

# Push to new repository
git push -u origin main
```

### 3. Alternative: Using SSH

If you prefer SSH:

```powershell
# Remove old remote
git remote remove origin

# Add new remote with SSH
git remote add origin git@github.com:USERNAME/16-node-burst-with-jsons-and-web-console.git
# OR for GitLab:
# git remote add origin git@gitlab.com:USERNAME/16-node-burst-with-jsons-and-web-console.git

# Push
git push -u origin main
```

## What's Included

This repository contains:

### Core Features
- ✅ 16-node intensive Kademlia P2P network testbed
- ✅ Random JSON message load generation
- ✅ Latency tracking and metrics
- ✅ Real-time web monitoring dashboard
- ✅ Connection heartbeat/keepalive (25-second ping)

### Files
- `start_intensive_16.ps1` / `start_intensive_16.sh` - Launch 16 nodes
- `src/monitor.rs` - Web dashboard + bootstrap node
- `src/listener.rs` / `src/dialer.rs` - P2P nodes with random load
- `web/index.html` - Real-time monitoring dashboard
- Comprehensive documentation

### Metrics Tracked
- Latency: min, max, avg, p50, p95, p99
- Throughput: messages per second
- Connection stats: active connections, total nodes
- Error tracking: message errors, timeouts

## Quick Start

```powershell
# Build
cargo build --release

# Start 16-node test
.\start_intensive_16.ps1

# Open dashboard
# http://localhost:8080
```

## Repository Description

**Suggested GitHub description:**
```
Intensive 16-node Kademlia P2P network testbed with random JSON message load, 
latency tracking, and real-time web monitoring dashboard. Features connection 
heartbeat, comprehensive metrics (latency percentiles, throughput), and 
automated load testing.
```

**Suggested topics/tags:**
- `kademlia`
- `p2p`
- `libp2p`
- `rust`
- `distributed-systems`
- `network-testing`
- `load-testing`
- `web-dashboard`
- `real-time-monitoring`












