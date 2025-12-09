# Punch Simple

A simple libp2p rendezvous client and server implementation for peer discovery and connection.

## Overview

This project provides three binaries:
- **server** - Rendezvous server that accepts peer registrations and serves discovery requests
- **listener** - Client that registers with the rendezvous server and waits for connections
- **dialer** - Client that discovers peers via the rendezvous server and connects to them

## Features

- libp2p 0.53 compatible
- TCP transport with Noise encryption and Yamux multiplexing
- Automatic retry logic with exponential backoff
- Command-line configuration (server, port, namespace)
- Cross-platform (Windows, Linux, macOS)

## Building

```bash
cargo build --release
```

This will build all three binaries:
- `target/release/server`
- `target/release/listener`
- `target/release/dialer`

## Usage

### Server

Start the rendezvous server:

```bash
# Default (0.0.0.0:51820)
./target/release/server

# Custom address and port
./target/release/server --listen-addr 0.0.0.0 --port 8080
```

### Listener

Register with the rendezvous server:

```bash
# Default (connects to 162.221.207.169:51820)
./target/release/listener

# Custom server
./target/release/listener --server 192.168.1.100 --port 8080

# Custom namespace
./target/release/listener --namespace my-namespace
```

### Dialer

Discover and connect to peers:

```bash
# Default (connects to 162.221.207.169:51820)
./target/release/dialer

# Custom server
./target/release/dialer --server 192.168.1.100 --port 8080

# Custom namespace
./target/release/dialer --namespace my-namespace
```

## Command-Line Arguments

All binaries support:
- `--server <HOST>` - Rendezvous server hostname or IP (default: 162.221.207.169)
- `--port <PORT>` - Rendezvous server port (default: 51820)
- `--namespace <NAMESPACE>` - Namespace for peer discovery (default: simple-chat)

Server additionally supports:
- `--listen-addr <ADDR>` - Address to listen on (default: 0.0.0.0)

## Example Workflow

1. **Start the server:**
   ```bash
   ./target/release/server --port 8080
   ```

2. **Start a listener (on machine A):**
   ```bash
   ./target/release/listener --server 192.168.1.100 --port 8080
   ```

3. **Start a dialer (on machine B):**
   ```bash
   ./target/release/dialer --server 192.168.1.100 --port 8080
   ```

The dialer will discover the listener and establish a peer-to-peer connection.

## Dependencies

- libp2p 0.53
- tokio 1.35
- clap 4.4

## License

This project is provided as-is for demonstration purposes.

