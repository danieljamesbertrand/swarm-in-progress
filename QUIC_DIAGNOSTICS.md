# QUIC Protocol Diagnostics Tool

## Overview

A comprehensive protocol analyzer for QUIC connections on the Punch Rendezvous Server. This tool provides real-time monitoring, connection analysis, and diagnostic capabilities to help troubleshoot QUIC connection issues.

## Features

- **Real-time Connection Tracking**: Monitors all QUIC connection attempts, handshake stages, and connection lifecycle
- **Handshake Stage Analysis**: Tracks Initial, Handshake, 1-RTT, and Completed stages
- **Error Logging**: Captures and categorizes connection errors, timeouts, and failures
- **Performance Metrics**: Tracks bytes sent/received, packet counts, handshake durations
- **Web Interface**: Beautiful, real-time web dashboard for monitoring
- **REST API**: JSON endpoints for programmatic access

## Accessing Diagnostics

### Web Interface

Once the server is running, access the diagnostic dashboard at:

```
http://<server-ip>:<port+1>/
```

For example, if the server is running on port 51820:
```
http://eagleoneonline.ca:51821/
```

The web interface provides:
- Real-time statistics dashboard
- Recent connection events table
- Active connections overview
- Error log viewer
- Auto-refresh capability (5-second intervals)

### REST API Endpoints

All endpoints return JSON data:

#### Get Full Diagnostics
```
GET /diagnostics
```

Returns complete diagnostic snapshot including:
- Total/active/failed connections
- Handshake timeout counts
- Byte and packet statistics
- All connection records
- Recent events
- Error log

#### Get Recent Events
```
GET /diagnostics/events?limit=100
```

Returns recent connection events (default: 100, max: 1000)

#### Get Error Log
```
GET /diagnostics/errors?limit=100
```

Returns recent errors (default: 100, max: 500)

#### Get Connection Stats
```
GET /diagnostics/connection/:peer_id/:addr
```

Returns detailed statistics for a specific connection

#### Health Check
```
GET /diagnostics/health
```

Returns service health status

## What Gets Tracked

### Connection Events

- **ConnectionAttempt**: Initial connection attempt detected
- **InitialSent/Received**: QUIC Initial packet sent/received
- **HandshakeSent/Received**: QUIC Handshake packet sent/received
- **Established**: Connection successfully established
- **Closed**: Connection closed
- **Error**: Connection error occurred
- **HandshakeTimeout**: Handshake timed out
- **Migration**: Connection migration event
- **StreamOpened/Closed**: Stream lifecycle events

### Handshake Stages

- **Initial**: Initial packet stage
- **Handshake**: Handshake packet stage
- **OneRtt**: 1-RTT packet stage (connection ready)
- **Completed**: Handshake completed
- **Failed**: Handshake failed

### Statistics Tracked

- Total connections attempted
- Active connections
- Failed connections
- Handshake timeouts
- Total bytes sent/received
- Total packets sent/received
- Average handshake duration
- Per-connection statistics

## Integration

The diagnostics module is automatically integrated into the rendezvous server. No additional configuration is needed.

### Server Code Integration

The diagnostics manager is initialized in `src/server.rs`:

```rust
let diagnostics = Arc::new(QuicDiagnosticsManager::new());
```

Events are automatically captured from the libp2p swarm:
- `SwarmEvent::ConnectionEstablished`
- `SwarmEvent::ConnectionClosed`
- `SwarmEvent::IncomingConnectionError`

### Manual Event Recording

You can also manually record events:

```rust
// Record connection attempt
diagnostics.record_connection_attempt(peer_id, remote_addr, local_addr).await;

// Record handshake stage
diagnostics.record_handshake_stage(peer_id, remote_addr, QuicHandshakeStage::Handshake).await;

// Record error
diagnostics.record_connection_error(peer_id, remote_addr, "Error message", Some(stage)).await;
```

## Troubleshooting Common Issues

### HandshakeTimeout Errors

If you see frequent `HandshakeTimeout` errors:

1. **Check Firewall**: Ensure UDP port 51820 is open
2. **Check Server Status**: Verify server is running and listening
3. **Check Network**: Verify network connectivity and NAT traversal
4. **Review Error Log**: Check the error log for specific error messages

### Connection Failures

1. **Review Connection Events**: Check the events table for connection lifecycle
2. **Check Handshake Stages**: See if connections are failing at specific stages
3. **Review Error Details**: Check error messages in the error log
4. **Check Active Connections**: See if connections are being established but then closing

### Performance Issues

1. **Check Handshake Duration**: High average handshake duration may indicate network issues
2. **Check Byte Counts**: Monitor bytes sent/received for unusual patterns
3. **Check Error Rates**: High error rates may indicate configuration issues

## Example Usage

### Using curl

```bash
# Get full diagnostics
curl http://eagleoneonline.ca:51821/diagnostics

# Get recent events
curl http://eagleoneonline.ca:51821/diagnostics/events?limit=50

# Get error log
curl http://eagleoneonline.ca:51821/diagnostics/errors?limit=20

# Get specific connection stats
curl http://eagleoneonline.ca:51821/diagnostics/connection/12D3KooW.../ip4/1.2.3.4
```

### Using JavaScript

```javascript
// Fetch diagnostics
const response = await fetch('http://eagleoneonline.ca:51821/diagnostics');
const data = await response.json();

console.log('Total connections:', data.total_connections);
console.log('Active connections:', data.active_connections);
console.log('Failed connections:', data.failed_connections);
console.log('Recent events:', data.recent_events);
```

## Architecture

### Module Structure

- **`src/quic_diagnostics.rs`**: Core diagnostics module
  - `QuicDiagnosticsManager`: Main manager class
  - `QuicConnectionStats`: Per-connection statistics
  - `QuicConnectionEvent`: Event records
  - `QuicHandshakeStage`: Handshake stage enum

- **`src/server.rs`**: Server integration
  - Event loop integration
  - HTTP server for diagnostics
  - REST API endpoints

- **`diagnostics.html`**: Web interface
  - Real-time dashboard
  - Auto-refresh capability
  - Event and error viewers

### Data Flow

1. **libp2p Swarm Events** → Server event loop
2. **Event Processing** → Diagnostics manager
3. **State Storage** → In-memory data structures
4. **HTTP Requests** → JSON responses
5. **Web Interface** → Real-time updates

## Performance Considerations

- **Memory Usage**: Events are stored in memory with configurable limits (1000 events, 500 errors)
- **CPU Usage**: Minimal overhead, events are processed asynchronously
- **Network**: HTTP server runs on separate port (port + 1) to avoid interference

## Future Enhancements

Potential improvements:
- Persistent storage (database backend)
- Export diagnostics to files
- Alert system for critical errors
- Historical trend analysis
- Packet-level capture integration
- Integration with external monitoring tools

## Notes

- The HTTP diagnostics server runs on port `server_port + 1`
- Diagnostics are stored in memory and reset on server restart
- The web interface requires modern browser with JavaScript enabled
- All timestamps are in Unix epoch seconds
