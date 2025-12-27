# Load Test Features

## What's Implemented

### 1. Random Message Load
- **Listeners and Dialers** now send random messages to random peers
- **Random intervals**: 100ms - 2000ms between message batches
- **Random batch size**: 1-5 messages per batch
- **Random peer selection**: Sends to 1-3 random connected peers
- **Random message content**: Various test message types

### 2. Latency Tracking
- **Message timestamps**: Each message includes `send_time_ms` for latency calculation
- **Per-message latency**: Calculated when messages are received
- **Latency display**: Shows latency in console output for each message

### 3. Enhanced Metrics (Structure Added)
The monitor now tracks:
- **Latency metrics**: min, max, avg, p50, p95, p99
- **Throughput**: messages per second
- **Message counts**: sent, received
- **Error tracking**: message errors, timeout errors
- **Data transfer**: bytes sent/received

## Current Status

✅ **Implemented:**
- Random message sending (listeners & dialers)
- Latency calculation in message handlers
- Metrics structure in monitor
- Latency display in console

⚠️ **Needs Implementation:**
- Monitor doesn't see peer-to-peer messages (only connections to monitor)
- Metrics aggregation from peers
- Actual latency tracking in monitor (structure exists but not populated)

## How It Works

### Message Flow
1. **Random Trigger**: Every 100-2000ms, a random number of messages (1-5) are queued
2. **Random Peer Selection**: Messages sent to 1-3 random connected peers
3. **Latency Tracking**: Each message includes `send_time_ms` timestamp
4. **Response**: Receiving peer calculates latency and includes it in response

### Latency Calculation
```rust
let latency_ms = if let Some(send_time) = request.send_time_ms {
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
    if now_ms > send_time {
        (now_ms - send_time) as f64
    } else {
        0.0
    }
} else {
    0.0
};
```

## Next Steps

To fully implement metrics tracking:

1. **Peer Metrics Reporting**: Have peers send metrics to monitor periodically
2. **Monitor Aggregation**: Monitor collects and aggregates metrics from all peers
3. **Real-time Updates**: Dashboard shows live latency and throughput metrics

## Current Console Output

You'll see in listener/dialer windows:
```
[📨 RECEIVED JSON MESSAGE] (latency: 12.34ms)
  From: dialer-12D3KooW
  Message: Load test message #42
  Timestamp: 1234567890

[📤 SENT RANDOM MESSAGE] to peer 12D3KooW... (#42)
```

## Metrics API

The monitor exposes metrics at:
- `http://localhost:8080/api/metrics` - Current metrics
- `http://localhost:8080/api/state` - Full network state including metrics

Metrics include:
- `latency_min_ms`, `latency_max_ms`, `latency_avg_ms`
- `latency_p50_ms`, `latency_p95_ms`, `latency_p99_ms`
- `messages_per_second`
- `messages_sent`, `messages_received`
- `bytes_sent`, `bytes_received`
- `message_errors`, `timeout_errors`












