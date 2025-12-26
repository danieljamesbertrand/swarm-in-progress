#!/bin/bash
# Bash test script for Kademlia P2P testbed
# Usage: ./test.sh

set -e

echo "=== Starting Kademlia P2P Testbed ==="
echo ""

# Get script directory
SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
cd "$SCRIPT_DIR"

# Check if binaries exist
if [ ! -f "target/release/server" ]; then
    echo "Binaries not found. Building..."
    cargo build --release
fi

# Function to cleanup on exit
cleanup() {
    echo ""
    echo "=== Stopping testbed ==="
    kill $SERVER_PID $LISTENER1_PID $DIALER_PID $LISTENER2_PID 2>/dev/null || true
    exit
}

trap cleanup EXIT INT TERM

echo "[1/4] Starting bootstrap node..."
cargo run --release --bin server -- --listen-addr 0.0.0.0 --port 51820 > /tmp/bootstrap.log 2>&1 &
SERVER_PID=$!
sleep 3

echo "[2/4] Starting listener (Peer A)..."
cargo run --release --bin listener -- --bootstrap /ip4/127.0.0.1/tcp/51820 --namespace test-room > /tmp/listener1.log 2>&1 &
LISTENER1_PID=$!
sleep 5

echo "[3/4] Starting dialer (Peer B)..."
cargo run --release --bin dialer -- --bootstrap /ip4/127.0.0.1/tcp/51820 --namespace test-room > /tmp/dialer.log 2>&1 &
DIALER_PID=$!
sleep 2

echo "[4/4] Starting additional listener (Peer C)..."
cargo run --release --bin listener -- --bootstrap /ip4/127.0.0.1/tcp/51820 --namespace test-room > /tmp/listener2.log 2>&1 &
LISTENER2_PID=$!
sleep 2

echo ""
echo "=== Testbed Started Successfully ==="
echo ""
echo "Process IDs:"
echo "  Bootstrap: $SERVER_PID"
echo "  Listener A: $LISTENER1_PID"
echo "  Dialer: $DIALER_PID"
echo "  Listener C: $LISTENER2_PID"
echo ""
echo "Logs:"
echo "  Bootstrap: /tmp/bootstrap.log"
echo "  Listener A: /tmp/listener1.log"
echo "  Dialer: /tmp/dialer.log"
echo "  Listener C: /tmp/listener2.log"
echo ""
echo "Expected behavior:"
echo "  - Dialer should discover and connect to both listeners"
echo "  - Messages should be exchanged between peers"
echo ""
echo "Press Ctrl+C to stop all processes"
echo ""

# Wait for interrupt
wait







