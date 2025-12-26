# Complete Usage Guide: P2P JSON Messaging

This guide shows **exactly** what parameters to fill and how to wait for responses.

## Table of Contents

1. [Step-by-Step Example](#step-by-step-example)
2. [Parameter Reference](#parameter-reference)
3. [Waiting for Responses](#waiting-for-responses)
4. [Error Handling](#error-handling)
5. [Complete Working Example](#complete-working-example)

---

## Step-by-Step Example

### Step 1: Add Dependencies

Add to your `Cargo.toml`:

```toml
[dependencies]
libp2p = { version = "0.53", features = ["quic", "noise", "tcp", "dns", "macros", "rendezvous", "identify", "yamux", "tokio", "request-response"] }
tokio = { version = "1.35", features = ["full"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
async-trait = "0.1"
```

### Step 2: Include the Helper Module

```rust
// Option 1: If it's in your crate
mod client_helper;
use client_helper::P2PClient;

// Option 2: If it's a separate crate
use your_crate::client_helper::P2PClient;
```

### Step 3: Create the Client

```rust
use serde_json::json;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // PARAMETER 1: Server address
    // Format: "host:port" or just "host" (defaults to port 51820)
    // Examples:
    //   "127.0.0.1:51820"     - Local server on default port
    //   "192.168.1.100:8080"   - Remote server on custom port
    //   "example.com"          - Domain name (uses port 51820)
    let server = "127.0.0.1:51820";
    
    // PARAMETER 2: Namespace
    // This is like a "room name" - peers must use the SAME namespace to find each other
    // Examples:
    //   "my-app"              - Simple namespace
    //   "chat-room-1"         - Descriptive namespace
    //   "game-lobby-abc123"   - Unique namespace
    let namespace = "my-namespace";
    
    // Create client - this CONNECTS to the rendezvous server automatically
    // This will BLOCK until connection is established
    let mut client = P2PClient::new(server, namespace).await?;
    
    println!("✓ Connected to rendezvous server");
    Ok(())
}
```

**What happens:**
- Generates a unique peer identity
- Connects to the rendezvous server
- Sets up encrypted communication
- Returns a `P2PClient` instance

**Errors you might see:**
- `"Connection refused"` - Server is not running
- `"Invalid address"` - Wrong server format
- `"Network unreachable"` - Can't reach the server

### Step 4: Connect to a Peer

```rust
    // This will BLOCK until a peer is found and connected
    // Make sure another peer is running and registered in the same namespace!
    let peer_id = client.connect_to_peer().await?;
    
    println!("✓ Connected to peer: {}", peer_id);
```

**What happens:**
- Sends discovery request to rendezvous server
- Waits for peer registrations in your namespace
- Connects to the first peer found
- Returns the peer's `PeerId`

**Important:**
- This function **BLOCKS** until a peer is found
- If no peers are available, it waits **indefinitely**
- Make sure another peer is running in the same namespace

**Errors you might see:**
- `"Not connected to rendezvous server"` - Call `new()` first
- No error, but waits forever - No peers available in namespace

### Step 5: Send a Message and Wait for Response

```rust
    // PARAMETER 3: Create your JSON message
    // REQUIRED FIELDS:
    //   - "from": String - Your identifier/name
    //   - "message": String - Your message text
    // OPTIONAL FIELDS:
    //   - "timestamp": Number (u64) - Unix timestamp (auto-set if missing)
    let request = json!({
        "from": "my-client",                    // REQUIRED: Your name/ID
        "message": "Hello from my app!",        // REQUIRED: Your message
        "timestamp": std::time::SystemTime::now()  // OPTIONAL: Current time
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    });
    
    // PARAMETER 4: Send to peer
    // Use the peer_id from connect_to_peer()
    // This will BLOCK for up to 10 seconds waiting for a response
    let response = client.send_and_wait(peer_id, request).await?;
    
    // Response is now available!
    println!("Response: {}", serde_json::to_string_pretty(&response)?);
```

**What happens:**
1. Validates your JSON message (checks for "from" and "message" fields)
2. Sends the message to the peer
3. **WAITS** for response (up to 10 seconds)
4. Returns the response as JSON

**Response format:**
```json
{
  "from": "peer-name",
  "message": "response text",
  "timestamp": 1234567890
}
```

**How to access response fields:**
```rust
    let from = response["from"].as_str().unwrap();
    let message = response["message"].as_str().unwrap();
    let timestamp = response["timestamp"].as_u64().unwrap();
    
    println!("From: {}, Message: {}, Time: {}", from, message, timestamp);
```

---

## Parameter Reference

### `P2PClient::new(server, namespace)`

#### `server: &str`
- **Type:** String slice
- **Format:** `"host:port"` or `"host"` (port defaults to 51820)
- **Examples:**
  ```rust
  "127.0.0.1:51820"        // Local server
  "192.168.1.100:8080"     // Remote server, custom port
  "example.com"             // Domain name (port 51820)
  "localhost"               // Localhost (port 51820)
  ```
- **Required:** Yes
- **What it does:** Rendezvous server address for peer discovery

#### `namespace: &str`
- **Type:** String slice
- **Format:** Any string (peers must use the same string to find each other)
- **Examples:**
  ```rust
  "my-app"
  "chat-room-1"
  "game-lobby-abc123"
  "shared-namespace"
  ```
- **Required:** Yes
- **What it does:** Namespace for peer discovery (like a "room name")

#### Returns
- **Type:** `Result<P2PClient, Box<dyn Error>>`
- **Success:** Connected client instance
- **Error:** Connection failed (server unreachable, invalid address, etc.)

---

### `connect_to_peer()`

#### Parameters
- **None** - Uses the namespace from `new()`

#### Returns
- **Type:** `Result<PeerId, Box<dyn Error>>`
- **Success:** `PeerId` of the connected peer (use this for `send_and_wait()`)
- **Error:** No peers found, connection failed, etc.

#### Behavior
- **BLOCKS** until a peer is found and connected
- Waits **indefinitely** if no peers are available
- Connects to the **first peer** found in the namespace

---

### `send_and_wait(peer_id, json_message)`

#### `peer_id: PeerId`
- **Type:** `PeerId` (from `connect_to_peer()`)
- **Required:** Yes
- **How to get:** `let peer_id = client.connect_to_peer().await?;`
- **What it does:** Identifies which peer to send the message to

#### `json_message: serde_json::Value`
- **Type:** `serde_json::Value` (created with `json!()` macro)
- **Required fields:**
  - `"from"`: String - Your identifier
  - `"message"`: String - Message text
- **Optional fields:**
  - `"timestamp"`: Number (u64) - Unix timestamp (auto-set if missing)
- **Example:**
  ```rust
  let msg = json!({
      "from": "my-app",
      "message": "Hello!",
      "timestamp": 1234567890  // Optional
  });
  ```
- **What it does:** The JSON message to send

#### Returns
- **Type:** `Result<serde_json::Value, Box<dyn Error>>`
- **Success:** Response JSON with structure:
  ```json
  {
    "from": "peer-name",
    "message": "response text",
    "timestamp": 1234567890
  }
  ```
- **Error:** Timeout (10 seconds), peer not connected, invalid JSON, etc.

#### Timeout
- **Default:** 10 seconds
- **Behavior:** Function blocks until response received or timeout
- **On timeout:** Returns `Err("Timeout waiting for response (10 seconds elapsed)")`

---

## Waiting for Responses

### How It Works

The `send_and_wait()` function **automatically waits** for the response. You don't need to do anything special:

```rust
// This line BLOCKS until response is received (or 10 second timeout)
let response = client.send_and_wait(peer_id, message).await?;

// Response is now available - the function already waited for it!
println!("Got response: {}", response["message"]);
```

### What "Waiting" Means

1. **Function blocks:** The `.await` call doesn't return until:
   - Response is received, OR
   - 10 second timeout occurs

2. **No polling needed:** You don't need to check for responses manually
   - The function handles all event processing internally
   - It processes swarm events until the response arrives

3. **Automatic timeout:** If no response in 10 seconds:
   - Function returns `Err("Timeout waiting for response")`
   - You can handle this error appropriately

### Example: Waiting with Error Handling

```rust
match client.send_and_wait(peer_id, message).await {
    Ok(response) => {
        // Response received successfully
        println!("Response: {}", response["message"]);
    }
    Err(e) => {
        if e.to_string().contains("Timeout") {
            println!("Peer didn't respond in time");
        } else {
            println!("Error: {}", e);
        }
    }
}
```

### Example: Multiple Messages

```rust
// Send first message and wait
let response1 = client.send_and_wait(peer_id, message1).await?;

// Send second message and wait
let response2 = client.send_and_wait(peer_id, message2).await?;

// Each call waits for its own response
```

---

## Error Handling

### Common Errors

#### 1. Connection Errors

```rust
match P2PClient::new("127.0.0.1:51820", "ns").await {
    Ok(client) => { /* success */ }
    Err(e) => {
        if e.to_string().contains("Connection refused") {
            println!("Server is not running");
        } else if e.to_string().contains("Network unreachable") {
            println!("Can't reach the server");
        } else {
            println!("Connection error: {}", e);
        }
    }
}
```

#### 2. No Peers Found

```rust
// connect_to_peer() will wait forever if no peers are available
// You might want to add a timeout:

match tokio::time::timeout(
    Duration::from_secs(30),
    client.connect_to_peer()
).await {
    Ok(Ok(peer_id)) => {
        println!("Connected to peer: {}", peer_id);
    }
    Ok(Err(e)) => {
        println!("Connection error: {}", e);
    }
    Err(_) => {
        println!("Timeout: No peers found in 30 seconds");
    }
}
```

#### 3. Response Timeout

```rust
match client.send_and_wait(peer_id, message).await {
    Ok(response) => {
        println!("Got response: {}", response);
    }
    Err(e) => {
        if e.to_string().contains("Timeout") {
            println!("Peer didn't respond within 10 seconds");
            // Handle timeout: retry, notify user, etc.
        } else {
            println!("Error: {}", e);
        }
    }
}
```

#### 4. Invalid JSON Message

```rust
// Missing required field
let bad_message = json!({
    "from": "me"
    // Missing "message" field!
});

match client.send_and_wait(peer_id, bad_message).await {
    Ok(_) => { /* won't happen */ }
    Err(e) => {
        if e.to_string().contains("Missing 'message' field") {
            println!("Your JSON is missing required fields");
        }
    }
}
```

---

## Complete Working Example

Here's a complete, copy-paste ready example:

```rust
use client_helper::P2PClient;
use serde_json::json;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== P2P Client Example ===\n");
    
    // Step 1: Connect to rendezvous server
    println!("[1] Connecting to rendezvous server...");
    let server = "127.0.0.1:51820";        // PARAMETER: Server address
    let namespace = "my-namespace";         // PARAMETER: Namespace
    
    let mut client = match P2PClient::new(server, namespace).await {
        Ok(c) => {
            println!("    ✓ Connected!");
            println!("    My Peer ID: {}\n", c.local_peer_id());
            c
        }
        Err(e) => {
            eprintln!("    ✗ Failed to connect: {}", e);
            eprintln!("    Make sure the rendezvous server is running!");
            return Err(e);
        }
    };
    
    // Step 2: Find and connect to a peer
    println!("[2] Discovering peers in namespace '{}'...", namespace);
    println!("    (Make sure another peer is running in the same namespace)");
    
    let peer_id = match tokio::time::timeout(
        Duration::from_secs(30),
        client.connect_to_peer()
    ).await {
        Ok(Ok(pid)) => {
            println!("    ✓ Connected to peer: {}\n", pid);
            pid
        }
        Ok(Err(e)) => {
            eprintln!("    ✗ Connection error: {}", e);
            return Err(e);
        }
        Err(_) => {
            eprintln!("    ✗ Timeout: No peers found in 30 seconds");
            eprintln!("    Make sure another peer is running and registered!");
            return Err("No peers found".into());
        }
    };
    
    // Step 3: Send a message and wait for response
    println!("[3] Sending JSON message...");
    
    // PARAMETER: Create your JSON message
    let request = json!({
        "from": "example-client",           // REQUIRED: Your identifier
        "message": "Hello from my app!",    // REQUIRED: Your message
        "timestamp": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()                      // OPTIONAL: Timestamp
    });
    
    println!("    Request: {}", serde_json::to_string_pretty(&request)?);
    println!("    Waiting for response (10 second timeout)...");
    
    // PARAMETER: Send and wait for response
    // This BLOCKS until response is received or timeout
    match client.send_and_wait(peer_id, request).await {
        Ok(response) => {
            println!("    ✓ Response received!");
            println!("    Response: {}", serde_json::to_string_pretty(&response)?);
            
            // Access response fields
            let from = response["from"].as_str().unwrap();
            let message = response["message"].as_str().unwrap();
            println!("\n    From: {}", from);
            println!("    Message: {}", message);
        }
        Err(e) => {
            if e.to_string().contains("Timeout") {
                eprintln!("    ✗ Timeout: Peer didn't respond in 10 seconds");
            } else {
                eprintln!("    ✗ Error: {}", e);
            }
            return Err(e);
        }
    }
    
    println!("\n=== Example Complete ===");
    Ok(())
}
```

---

## Summary

### Required Parameters

1. **Server address:** `"host:port"` or `"host"` (default port 51820)
2. **Namespace:** Any string (peers must match to find each other)
3. **Peer ID:** From `connect_to_peer()` return value
4. **JSON message:** Must have `"from"` and `"message"` fields

### How to Wait for Response

**You don't need to do anything special!** Just use `.await`:

```rust
// This automatically waits for the response
let response = client.send_and_wait(peer_id, message).await?;
```

The function:
- Blocks until response is received
- Times out after 10 seconds
- Returns the response as JSON
- Handles all event processing internally

### Next Steps

1. Copy the code fragment into your project
2. Fill in the parameters (server, namespace, message)
3. Use `.await` to wait for responses
4. Handle errors appropriately

That's it! The function handles all the waiting and event processing for you.









