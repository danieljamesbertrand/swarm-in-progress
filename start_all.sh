#!/bin/bash
# Bash script to start the complete Kademlia P2P network with monitoring
# Usage: ./start_all.sh

set -e

echo "=== Starting Complete Kademlia P2P Network ==="
echo ""

SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
cd "$SCRIPT_DIR"

# Check if binaries exist
if [ ! -f "target/release/monitor" ]; then
    echo "Binaries not found. Building..."
    cargo build --release
fi

# Function to cleanup on exit
cleanup() {
    echo ""
    echo "=== Stopping network ==="
    kill $MONITOR_PID $LISTENER1_PID $LISTENER2_PID $DIALER1_PID $DIALER2_PID 2>/dev/null || true
    exit
}

trap cleanup EXIT INT TERM

echo "[1/5] Starting Network Monitor (Bootstrap + Web Dashboard)..."
cargo run --release --bin monitor > /tmp/monitor.log 2>&1 &
MONITOR_PID=$!
sleep 5

echo "[2/5] Starting Listener 1 (Peer A)..."
cargo run --release --bin listener -- --bootstrap /ip4/127.0.0.1/tcp/51820 --namespace test-room > /tmp/listener1.log 2>&1 &
LISTENER1_PID=$!
sleep 3

echo "[3/5] Starting Listener 2 (Peer B)..."
cargo run --release --bin listener -- --bootstrap /ip4/127.0.0.1/tcp/51820 --namespace test-room > /tmp/listener2.log 2>&1 &
LISTENER2_PID=$!
sleep 3

echo "[4/5] Starting Dialer 1 (Peer C)..."
cargo run --release --bin dialer -- --bootstrap /ip4/127.0.0.1/tcp/51820 --namespace test-room > /tmp/dialer1.log 2>&1 &
DIALER1_PID=$!
sleep 2

echo "[5/5] Starting Dialer 2 (Peer D)..."
cargo run --release --bin dialer -- --bootstrap /ip4/127.0.0.1/tcp/51820 --namespace test-room > /tmp/dialer2.log 2>&1 &
DIALER2_PID=$!
sleep 2

echo ""
echo "=== Network Started Successfully! ==="
echo ""
echo "Process IDs:"
echo "  Monitor: $MONITOR_PID"
echo "  Listener 1: $LISTENER1_PID"
echo "  Listener 2: $LISTENER2_PID"
echo "  Dialer 1: $DIALER1_PID"
echo "  Dialer 2: $DIALER2_PID"
echo ""
echo "🌐 Web Dashboard: http://localhost:8080"
echo ""
echo "Logs:"
echo "  Monitor: /tmp/monitor.log"
echo "  Listener 1: /tmp/listener1.log"
echo "  Listener 2: /tmp/listener2.log"
echo "  Dialer 1: /tmp/dialer1.log"
echo "  Dialer 2: /tmp/dialer2.log"
echo ""
echo "Expected behavior:"
echo "  - Monitor shows bootstrap node running"
echo "  - Listeners register in DHT and wait for connections"
echo "  - Dialers discover and connect to listeners"
echo "  - Dashboard shows all nodes and connections in real-time"
echo ""
echo "Opening dashboard in browser..."
sleep 2

# Try to open browser (works on most Linux/Mac)
if command -v xdg-open &> /dev/null; then
    xdg-open "http://localhost:8080"
elif command -v open &> /dev/null; then
    open "http://localhost:8080"
fi

echo ""
echo "Press Ctrl+C to stop all processes"
echo ""

# Wait for interrupt
wait

