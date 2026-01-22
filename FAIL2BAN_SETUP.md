# fail2ban Configuration for Punch Rendezvous Server

## Overview

fail2ban is configured to monitor and block suspicious connection attempts on port 51820 (QUIC/TCP) for the Punch Rendezvous Server.

## Protection Details

- **Max Retries**: 5 failed connection attempts
- **Time Window**: 5 minutes (300 seconds)
- **Ban Duration**: 1 hour (3600 seconds)
- **Protocols**: UDP (QUIC) and TCP
- **Port**: 51820

## How It Works

1. **Server Logging**: The server logs connection errors in a fail2ban-friendly format:
   - `[FAIL2BAN] Connection attempt failed from IP: <IP>`
   - `[SECURITY] Incoming connection error from <IP>: <error details>`

2. **fail2ban Monitoring**: fail2ban monitors `/home/dbertrand/punch-simple/server.log` for these patterns.

3. **Automatic Blocking**: After 5 failed attempts within 5 minutes, fail2ban automatically:
   - Adds a firewall rule to block the IP
   - Logs the ban action
   - Maintains the ban for 1 hour

## Configuration Files

- **Filter**: `/etc/fail2ban/filter.d/punch-rendezvous.conf`
  - Defines regex patterns to match connection errors
  
- **Jail**: `/etc/fail2ban/jail.d/punch-rendezvous.conf`
  - Defines ban parameters (maxretry, findtime, bantime)
  - Points to the log file and filter

## Monitoring

### Check Jail Status
```bash
ssh dbertrand@eagleoneonline.ca 'sudo fail2ban-client status punch-rendezvous'
```

### View Banned IPs
```bash
ssh dbertrand@eagleoneonline.ca 'sudo fail2ban-client status punch-rendezvous | grep "Banned IP"'
```

### Unban an IP (if needed)
```bash
ssh dbertrand@eagleoneonline.ca 'sudo fail2ban-client set punch-rendezvous unbanip <IP_ADDRESS>'
```

### View Recent Logs
```bash
ssh dbertrand@eagleoneonline.ca 'tail -50 /home/dbertrand/punch-simple/server.log | grep -E "\[FAIL2BAN\]|\[SECURITY\]"'
```

## Server Code Changes

The server code (`src/server.rs`) has been enhanced to:
1. Extract IP addresses from `IncomingConnectionError` events using `send_back_addr`
2. Log connection errors in fail2ban-friendly format
3. Include both `[FAIL2BAN]` and `[SECURITY]` log prefixes for different error types

## Testing

To test the configuration (after server is running and logging):

1. **Generate test log entry** (manually or via connection attempt):
   ```bash
   echo "[FAIL2BAN] Connection attempt failed from IP: 192.168.1.100" >> /home/dbertrand/punch-simple/server.log
   ```

2. **Check if fail2ban detects it**:
   ```bash
   sudo fail2ban-client status punch-rendezvous
   ```

3. **Verify regex matching**:
   ```bash
   echo "[FAIL2BAN] Connection attempt failed from IP: 192.168.1.100" | sudo fail2ban-regex /dev/stdin /etc/fail2ban/filter.d/punch-rendezvous.conf
   ```

## What Gets Detected

The filter detects:
- **Persistent connection failures**: Multiple failed connection attempts from the same IP
- **Connection errors**: Any incoming connection error logged by the server
- **Suspicious patterns**: Rapid repeated connection attempts (5+ within 5 minutes)

## What Doesn't Get Blocked

- Legitimate connection attempts that succeed
- Occasional connection failures (less than 5 in 5 minutes)
- Connections from whitelisted IPs (if configured)

## Notes

- The server must be rebuilt and restarted for the enhanced logging to take effect
- fail2ban reads from the log file, so ensure the log file path is correct
- Systemd service configuration ensures logs are written to `/home/dbertrand/punch-simple/server.log`
- Ban actions use the default fail2ban action (typically `iptables` or `ufw`)

## Troubleshooting

### Jail not active
```bash
sudo fail2ban-client reload
sudo fail2ban-client status punch-rendezvous
```

### Filter not matching
```bash
# Test the filter manually
echo "[FAIL2BAN] Connection attempt failed from IP: 192.168.1.100" | sudo fail2ban-regex -v /dev/stdin /etc/fail2ban/filter.d/punch-rendezvous.conf
```

### Check fail2ban logs
```bash
sudo tail -50 /var/log/fail2ban.log | grep punch-rendezvous
```
