#!/bin/bash
# Bash script to start an intensive 16-node Kademlia P2P network test
# Usage: ./start_intensive_16.sh

echo "=== Starting Intensive 16-Node Kademlia P2P Network Test ==="
echo ""

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

# Check if binaries exist
if [ ! -f "target/release/monitor" ]; then
    echo "Binaries not found. Building..."
    cargo build --release
    if [ $? -ne 0 ]; then
        echo "Build failed!"
        exit 1
    fi
fi

NAMESPACE="intensive-test"
BOOTSTRAP="/ip4/127.0.0.1/tcp/51820"

# Start monitor
echo "[1/17] Starting Network Monitor (Bootstrap + Web Dashboard)..."
gnome-terminal --title="Network Monitor" -- bash -c "cd '$SCRIPT_DIR'; echo '=== NETWORK MONITOR ==='; echo 'Dashboard: http://localhost:8080'; echo ''; cargo run --release --bin monitor; exec bash" 2>/dev/null || \
xterm -T "Network Monitor" -e "cd '$SCRIPT_DIR'; echo '=== NETWORK MONITOR ==='; echo 'Dashboard: http://localhost:8080'; echo ''; cargo run --release --bin monitor; exec bash" 2>/dev/null || \
osascript -e "tell app \"Terminal\" to do script \"cd '$SCRIPT_DIR' && echo '=== NETWORK MONITOR ===' && echo 'Dashboard: http://localhost:8080' && echo '' && cargo run --release --bin monitor\"" 2>/dev/null
sleep 5

# Start 8 listeners
for i in {1..8}; do
    echo "[$((i+1))/17] Starting Listener $i..."
    gnome-terminal --title="Listener $i" -- bash -c "cd '$SCRIPT_DIR'; echo '=== LISTENER $i ==='; echo 'Namespace: $NAMESPACE'; echo ''; cargo run --release --bin listener -- --bootstrap $BOOTSTRAP --namespace $NAMESPACE; exec bash" 2>/dev/null || \
    xterm -T "Listener $i" -e "cd '$SCRIPT_DIR'; echo '=== LISTENER $i ==='; echo 'Namespace: $NAMESPACE'; echo ''; cargo run --release --bin listener -- --bootstrap $BOOTSTRAP --namespace $NAMESPACE; exec bash" 2>/dev/null || \
    osascript -e "tell app \"Terminal\" to do script \"cd '$SCRIPT_DIR' && echo '=== LISTENER $i ===' && echo 'Namespace: $NAMESPACE' && echo '' && cargo run --release --bin listener -- --bootstrap $BOOTSTRAP --namespace $NAMESPACE\"" 2>/dev/null
    sleep 0.5
done

sleep 2

# Start 8 dialers
for i in {1..8}; do
    echo "[$((i+9))/17] Starting Dialer $i..."
    gnome-terminal --title="Dialer $i" -- bash -c "cd '$SCRIPT_DIR'; echo '=== DIALER $i ==='; echo 'Namespace: $NAMESPACE'; echo ''; cargo run --release --bin dialer -- --bootstrap $BOOTSTRAP --namespace $NAMESPACE; exec bash" 2>/dev/null || \
    xterm -T "Dialer $i" -e "cd '$SCRIPT_DIR'; echo '=== DIALER $i ==='; echo 'Namespace: $NAMESPACE'; echo ''; cargo run --release --bin dialer -- --bootstrap $BOOTSTRAP --namespace $NAMESPACE; exec bash" 2>/dev/null || \
    osascript -e "tell app \"Terminal\" to do script \"cd '$SCRIPT_DIR' && echo '=== DIALER $i ===' && echo 'Namespace: $NAMESPACE' && echo '' && cargo run --release --bin dialer -- --bootstrap $BOOTSTRAP --namespace $NAMESPACE\"" 2>/dev/null
    sleep 0.5
done

sleep 2

echo ""
echo "=== Intensive 16-Node Network Started! ==="
echo ""
echo "Network Configuration:"
echo "  - 1 Monitor (Bootstrap + Dashboard)"
echo "  - 8 Listeners (Register in DHT)"
echo "  - 8 Dialers (Discover and Connect)"
echo "  - Total: 16 P2P Nodes + 1 Monitor"
echo "  - Namespace: $NAMESPACE"
echo ""
echo "🌐 Web Dashboard: http://localhost:8080"
echo ""
echo "Expected Behavior:"
echo "  - All 16 nodes connect to bootstrap (monitor)"
echo "  - Listeners register their peer info in DHT"
echo "  - Dialers query DHT and discover all listeners"
echo "  - Dialers connect to discovered listeners"
echo "  - Listeners also discover each other via DHT"
echo "  - Network forms a mesh of connections"
echo "  - Dashboard shows all nodes and connections"
echo ""
echo "Discovery Timeline:"
echo "  0-5s:   Monitor starts, nodes begin connecting"
echo "  5-15s:  All nodes bootstrap to DHT"
echo "  15-30s: Listeners register in DHT"
echo "  30-60s: Dialers discover and connect to listeners"
echo "  60s+:   Full mesh network established"
echo ""

# Try to open browser
if command -v xdg-open > /dev/null; then
    xdg-open "http://localhost:8080" 2>/dev/null &
elif command -v open > /dev/null; then
    open "http://localhost:8080" 2>/dev/null &
fi

echo "Press Enter to exit (terminals will remain open)..."
read






