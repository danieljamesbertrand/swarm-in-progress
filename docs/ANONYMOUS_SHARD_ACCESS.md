# Anonymous Model Shard Access

This document describes how to configure and use anonymous (public) access to Llama model shards for the Promethos-AI Swarm network.

## Overview

For rapid node onboarding and public model distribution, Promethos-AI supports anonymous access to model shards. This allows any node to quickly download the necessary model files without authentication.

---

## Server Setup (rsync.net)

### Option 1: Read-Only Anonymous SSH Key (Recommended)

Create a **public shared key** that anyone can use for read-only access:

#### 1. Generate a Shared Key Pair

```bash
# Generate key pair (do this once, share the private key publicly)
ssh-keygen -t ed25519 -f promethos_public_key -N "" -C "promethos-anonymous-readonly"
```

#### 2. Add to rsync.net authorized_keys

SSH into your rsync.net account:

```bash
ssh zh5605@zh5605.rsync.net
```

Edit `~/.ssh/authorized_keys` and add:

```bash
# Promethos-AI Anonymous Read-Only Access
# This key is PUBLIC - share freely
command="rrsync -ro /llama-shards",restrict,no-pty ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAA... promethos-anonymous-readonly
```

Key restrictions explained:
- `command="rrsync -ro /llama-shards"` - Can ONLY rsync, read-only, chrooted to `/llama-shards`
- `restrict` - Disables port forwarding, agent forwarding, X11
- `no-pty` - No interactive shell

#### 3. Publish the Private Key

Embed the private key in the application or publish it:

```
-----BEGIN OPENSSH PRIVATE KEY-----
b3BlbnNzaC1rZXktdjEAAAAABG5vbmUAAAAEbm9uZQAAAAAAAAABAAAAMwAAAAtzc2gtZW
QyNTUxOQAAACBxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx==
-----END OPENSSH PRIVATE KEY-----
```

> ⚠️ **Security Note**: This key provides READ-ONLY access to a specific folder. It cannot modify, delete, or access other files.

---

### Option 2: rsync Daemon Mode (Alternative)

If using a dedicated server (not rsync.net), you can run rsync in daemon mode:

#### `/etc/rsyncd.conf`

```ini
# Promethos-AI Shard Server Configuration
uid = nobody
gid = nogroup
use chroot = yes
max connections = 100
timeout = 300
read only = yes
log file = /var/log/rsyncd.log

# Anonymous access to model shards
[llama-shards]
    path = /data/llama-shards
    comment = Promethos-AI Llama Model Shards
    read only = yes
    list = yes
    # No auth required for read access
    auth users = 
    # Optional: restrict by IP
    # hosts allow = 10.0.0.0/8, 192.168.0.0/16
```

Start the daemon:

```bash
rsync --daemon --config=/etc/rsyncd.conf
```

---

## Client Configuration

### Using the Public Key

#### Environment Variable

```bash
# Linux/macOS
export PROMETHOS_SHARD_KEY="$(cat promethos_public_key)"

# Windows PowerShell
$env:PROMETHOS_SHARD_KEY = Get-Content promethos_public_key -Raw
```

#### Embedded in Application

The public key can be embedded directly in the `ScpConfig`:

```rust
use punch_simple::{ScpConfig, LlamaModelManager};
use std::path::PathBuf;
use std::io::Write;

/// Create anonymous access configuration
pub fn anonymous_shard_config() -> ScpConfig {
    // The public anonymous key (read-only, restricted to /llama-shards)
    let anonymous_key = r#"-----BEGIN OPENSSH PRIVATE KEY-----
b3BlbnNzaC1rZXktdjEAAAAABG5vbmUAAAAEbm9uZQAAAAAAAAABAAAAMwAAAAtzc2gtZW
QyNTUxOQAAACBxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx==
-----END OPENSSH PRIVATE KEY-----"#;

    // Write key to temp file
    let key_path = std::env::temp_dir().join("promethos_anon_key");
    let mut file = std::fs::File::create(&key_path).unwrap();
    file.write_all(anonymous_key.as_bytes()).unwrap();
    
    // Set permissions (Unix only)
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&key_path, std::fs::Permissions::from_mode(0o600)).unwrap();
    }

    ScpConfig::with_ssh_key(
        "zh5605.rsync.net",
        "zh5605",
        key_path,
    )
    .remote_path(".")  // Already chrooted to /llama-shards
    .cache_dir(PathBuf::from("./models_cache"))
}
```

---

## Quick Start Commands

### List Available Shards

```bash
# Using the anonymous key
ssh -i promethos_public_key -o StrictHostKeyChecking=no \
    zh5605@zh5605.rsync.net ls -la

# Expected output (chrooted to /llama-shards):
# -rw-r--r-- 1 user user 4.3G Dec 26 llama-8b-q4_k_m-shard-0.gguf
# -rw-r--r-- 1 user user 4.3G Dec 26 llama-8b-q4_k_m-shard-1.gguf
# -rw-r--r-- 1 user user 4.3G Dec 26 llama-8b-q4_k_m-shard-2.gguf
# -rw-r--r-- 1 user user 4.3G Dec 26 llama-8b-q4_k_m-shard-3.gguf
```

### Download a Specific Shard

```bash
# Download shard 0
scp -i promethos_public_key -o StrictHostKeyChecking=no \
    zh5605@zh5605.rsync.net:llama-8b-q4_k_m-shard-0.gguf \
    ./models_cache/

# Download all shards
scp -i promethos_public_key -o StrictHostKeyChecking=no \
    "zh5605@zh5605.rsync.net:*.gguf" \
    ./models_cache/
```

### Using rsync (if daemon mode)

```bash
# List available modules
rsync rsync://shard-server.promethos.ai/

# List shards in a module
rsync rsync://shard-server.promethos.ai/llama-shards/

# Download specific shard
rsync -avz --progress \
    rsync://shard-server.promethos.ai/llama-shards/llama-8b-q4_k_m-shard-0.gguf \
    ./models_cache/

# Download all shards for a model
rsync -avz --progress \
    rsync://shard-server.promethos.ai/llama-shards/llama-8b-*.gguf \
    ./models_cache/
```

---

## Directory Structure

Recommended shard organization on the server:

```
/llama-shards/
├── README.txt                          # Usage instructions
├── CHECKSUMS.sha256                    # File integrity verification
├── llama-7b/
│   ├── q4_k_m/
│   │   ├── shard-0.gguf               # Layers 0-7, embeddings
│   │   ├── shard-1.gguf               # Layers 8-15
│   │   ├── shard-2.gguf               # Layers 16-23
│   │   └── shard-3.gguf               # Layers 24-31, output head
│   └── q8_0/
│       └── ...
├── llama-13b/
│   └── q4_k_m/
│       ├── shard-0.gguf
│       ├── shard-1.gguf
│       └── ...
├── llama-70b/
│   └── q4_k_m/
│       ├── shard-0.gguf               # ~8.75GB each
│       ├── shard-1.gguf
│       ├── shard-2.gguf
│       └── shard-3.gguf
└── metadata/
    ├── models.json                     # Available models list
    └── shards.json                     # Shard metadata
```

### metadata/models.json

```json
{
  "version": "1.0",
  "updated": "2024-12-26T00:00:00Z",
  "models": [
    {
      "name": "llama-7b",
      "params_billions": 7.0,
      "quantizations": ["q4_k_m", "q8_0"],
      "default_shards": 4,
      "total_layers": 32
    },
    {
      "name": "llama-13b",
      "params_billions": 13.0,
      "quantizations": ["q4_k_m"],
      "default_shards": 4,
      "total_layers": 40
    },
    {
      "name": "llama-70b",
      "params_billions": 70.0,
      "quantizations": ["q4_k_m"],
      "default_shards": 4,
      "total_layers": 80
    }
  ]
}
```

### metadata/shards.json

```json
{
  "version": "1.0",
  "shards": [
    {
      "model": "llama-7b",
      "quantization": "q4_k_m",
      "shard_id": 0,
      "filename": "llama-7b/q4_k_m/shard-0.gguf",
      "size_bytes": 4617089024,
      "sha256": "abc123...",
      "layer_start": 0,
      "layer_end": 8,
      "has_embeddings": true,
      "has_output": false
    }
  ]
}
```

---

## Integration with Promethos-AI

### Automatic Shard Discovery

```rust
use punch_simple::{LlamaModelManager, ScpConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Use anonymous configuration
    let config = anonymous_shard_config();
    let manager = LlamaModelManager::new(config);
    
    // List available shards
    let shards = manager.list_available_shards().await?;
    println!("Available shards: {:?}", shards);
    
    // Download what we need
    let shard_path = manager.download_shard("llama-7b/q4_k_m/shard-0.gguf").await?;
    println!("Downloaded to: {}", shard_path.display());
    
    Ok(())
}
```

### Environment Variable Configuration

```bash
# Point to anonymous shard server
export PROMETHOS_SHARD_SERVER="zh5605.rsync.net"
export PROMETHOS_SHARD_USER="zh5605"
export PROMETHOS_SHARD_KEY_PATH="/path/to/promethos_public_key"

# Or use built-in anonymous access
export PROMETHOS_ANONYMOUS_ACCESS=true
```

---

## Security Considerations

### What Anonymous Access CAN Do:
- ✅ List files in `/llama-shards` directory
- ✅ Download (read) files from `/llama-shards`
- ✅ Check file sizes and metadata

### What Anonymous Access CANNOT Do:
- ❌ Access files outside `/llama-shards`
- ❌ Write, modify, or delete any files
- ❌ Execute commands on the server
- ❌ Open interactive shell
- ❌ Forward ports or use as proxy
- ❌ Access SSH agent
- ❌ Access other users' data

### Rate Limiting (Optional)

If hosting your own server, consider adding rate limits:

```bash
# iptables rate limiting for SSH
iptables -A INPUT -p tcp --dport 22 -m state --state NEW \
    -m recent --set --name SSH
iptables -A INPUT -p tcp --dport 22 -m state --state NEW \
    -m recent --update --seconds 60 --hitcount 10 --name SSH \
    -j DROP
```

---

## Bandwidth Considerations

| Model | Quantization | Total Size | Per Shard (4) |
|-------|--------------|------------|---------------|
| Llama 7B | Q4_K_M | ~4 GB | ~1 GB |
| Llama 13B | Q4_K_M | ~7 GB | ~1.75 GB |
| Llama 70B | Q4_K_M | ~35 GB | ~8.75 GB |

### Recommended: Delta/Incremental Updates

For large shards, use rsync's delta algorithm:

```bash
# Only transfer changed portions (useful for updates)
rsync -avz --partial --progress \
    rsync://server/llama-shards/llama-70b/ \
    ./models_cache/llama-70b/
```

---

## Troubleshooting

### "Permission denied" Error

The anonymous key only works for specific paths:
```bash
# This works (within chroot)
scp -i key user@host:shard-0.gguf .

# This fails (outside chroot)  
scp -i key user@host:/etc/passwd .  # DENIED
```

### "Connection refused" Error

Check that:
1. SSH is running on port 22
2. The key is in `authorized_keys` on the server
3. Network/firewall allows connection

### Slow Downloads

- Use `rsync` instead of `scp` for resume capability
- Check your network bandwidth
- Consider a regional mirror

---

## Setting Up Your Own Shard Mirror

To run a community mirror:

1. **Request the model shards** from the main Promethos server
2. **Set up your server** with anonymous access (see Server Setup above)
3. **Register as a mirror** in the DHT network
4. **Announce availability** via the Promethos DHT

```rust
// Announce as a shard mirror
let announcement = ShardMirrorAnnouncement {
    server: "mirror.example.com".to_string(),
    username: "promethos".to_string(),
    anonymous_key_url: "https://mirror.example.com/promethos_key".to_string(),
    available_models: vec!["llama-7b", "llama-13b"],
    region: "us-west".to_string(),
    bandwidth_mbps: 1000,
};
```

---

## Public Key Distribution

The anonymous public key should be distributed via:

1. **GitHub Repository** - `keys/promethos_anonymous.key`
2. **HTTPS Download** - `https://promethos.ai/keys/anonymous`
3. **DHT Announcement** - Embedded in bootstrap node info
4. **Built into Client** - Compiled into the binary for zero-config

---

## Summary

Anonymous shard access enables:
- 🚀 **Instant node onboarding** - No registration required
- 🌐 **Global distribution** - Anyone can download models
- 🔒 **Secure by design** - Read-only, chrooted, restricted
- 📦 **Simple integration** - One environment variable or built-in

For questions or to request mirror status, contact the Promethos-AI team.







