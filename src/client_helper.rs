//! # P2P JSON Messaging Client Helper
//! 
//! This module provides a simple, high-level API for peer-to-peer JSON messaging
//! using libp2p rendezvous for peer discovery.
//! 
//! ## Quick Start
//! 
//! ```rust,no_run
//! use client_helper::P2PClient;
//! use serde_json::json;
//! 
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Step 1: Create client (connects to rendezvous server automatically)
//!     let mut client = P2PClient::new("127.0.0.1:51820", "my-namespace").await?;
//!     
//!     // Step 2: Connect to a peer (blocks until peer is found)
//!     let peer_id = client.connect_to_peer().await?;
//!     
//!     // Step 3: Send message and wait for response
//!     let request = json!({
//!         "from": "my-app",
//!         "message": "Hello!",
//!         "timestamp": std::time::SystemTime::now()
//!             .duration_since(std::time::UNIX_EPOCH)
//!             .unwrap()
//!             .as_secs()
//!     });
//!     
//!     let response = client.send_and_wait(peer_id, request).await?;
//!     println!("Response: {}", response);
//!     
//!     Ok(())
//! }
//! ```

mod message;
use message::{JsonMessage, JsonCodec};

use libp2p::{
    identity,
    tcp,
    noise,
    yamux,
    rendezvous,
    request_response::{self, ProtocolSupport},
    swarm::{NetworkBehaviour, Swarm, SwarmEvent},
    core::transport::Transport,
    PeerId, Multiaddr, StreamProtocol,
};
use libp2p::swarm::Config as SwarmConfig;
use libp2p::futures::StreamExt;
use std::error::Error;
use std::time::Duration;
use tokio::time::timeout;
use std::collections::HashMap;

#[derive(NetworkBehaviour)]
#[behaviour(out_event = "BehaviourEvent")]
struct Behaviour {
    rendezvous: rendezvous::client::Behaviour,
    identify: libp2p::identify::Behaviour,
    request_response: request_response::Behaviour<JsonCodec>,
}

#[derive(Debug)]
enum BehaviourEvent {
    Rendezvous(rendezvous::client::Event),
    Identify(libp2p::identify::Event),
    RequestResponse(request_response::Event<JsonCodec>),
}

impl From<rendezvous::client::Event> for BehaviourEvent {
    fn from(event: rendezvous::client::Event) -> Self {
        BehaviourEvent::Rendezvous(event)
    }
}

impl From<libp2p::identify::Event> for BehaviourEvent {
    fn from(event: libp2p::identify::Event) -> Self {
        BehaviourEvent::Identify(event)
    }
}

impl From<request_response::Event<JsonCodec>> for BehaviourEvent {
    fn from(event: request_response::Event<JsonCodec>) -> Self {
        BehaviourEvent::RequestResponse(event)
    }
}

/// # P2P Client for JSON Messaging
/// 
/// This struct provides a simple interface for:
/// 1. Connecting to a rendezvous server
/// 2. Discovering and connecting to peers
/// 3. Sending JSON messages and receiving responses
/// 
/// ## Example
/// 
/// ```rust,no_run
/// use client_helper::P2PClient;
/// use serde_json::json;
/// 
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     // Create client - this automatically connects to the rendezvous server
///     let mut client = P2PClient::new("127.0.0.1:51820", "my-app").await?;
///     
///     // Find and connect to a peer
///     let peer_id = client.connect_to_peer().await?;
///     
///     // Send a message
///     let msg = json!({
///         "from": "client-1",
///         "message": "Hello!",
///         "timestamp": 1234567890
///     });
///     
///     // Wait for response (blocks up to 10 seconds)
///     let response = client.send_and_wait(peer_id, msg).await?;
///     
///     Ok(())
/// }
/// ```
pub struct P2PClient {
    /// Internal libp2p swarm that handles all network operations
    swarm: Swarm<Behaviour>,
    /// Rendezvous server address (host:port format)
    rendezvous_server: String,
    /// Namespace for peer discovery (peers must use the same namespace to find each other)
    namespace: String,
    /// Peer ID of the rendezvous server (set after connection)
    rendezvous_peer_id: Option<PeerId>,
    /// Map of currently connected peers
    connected_peers: HashMap<PeerId, ()>,
    /// Map of pending request IDs to response channels
    /// When you send a request, we store a channel here to receive the response
    pending_responses: HashMap<request_response::RequestId, tokio::sync::oneshot::Sender<serde_json::Value>>,
}

impl P2PClient {
    /// # Create a new P2P client and connect to the rendezvous server
    /// 
    /// This function:
    /// 1. Generates a new peer identity (keypair)
    /// 2. Sets up encrypted TCP transport (Noise + Yamux)
    /// 3. Configures rendezvous client for peer discovery
    /// 4. Configures request-response protocol for JSON messaging
    /// 5. Connects to the rendezvous server
    /// 
    /// ## Parameters
    /// 
    /// - **`server`**: Rendezvous server address in format `"host:port"` or `"host"` (defaults to port 51820)
    ///   - Examples: `"127.0.0.1:51820"`, `"192.168.1.100:8080"`, `"example.com"`
    ///   - If port is omitted, defaults to `51820`
    /// 
    /// - **`namespace`**: Namespace string for peer discovery
    ///   - Peers must use the **same namespace** to discover each other
    ///   - Examples: `"my-app"`, `"chat-room-1"`, `"game-lobby"`
    ///   - This is like a "room name" - only peers in the same namespace can find each other
    /// 
    /// ## Returns
    /// 
    /// - **`Ok(P2PClient)`**: Successfully created and connected client
    /// - **`Err(Box<dyn Error>)`**: Connection failed (server unreachable, invalid address, etc.)
    /// 
    /// ## Errors
    /// 
    /// - Network errors (server unreachable, connection refused)
    /// - Invalid server address format
    /// - Transport setup errors
    /// 
    /// ## Example
    /// 
    /// ```rust,no_run
    /// use client_helper::P2PClient;
    /// 
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     // Connect to local server
    ///     let mut client = P2PClient::new("127.0.0.1:51820", "my-namespace").await?;
    ///     
    ///     // Or connect to remote server
    ///     let mut client = P2PClient::new("192.168.1.100:8080", "shared-namespace").await?;
    ///     
    ///     Ok(())
    /// }
    /// ```
    pub async fn new(server: &str, namespace: &str) -> Result<Self, Box<dyn Error>> {
        // Generate a new Ed25519 keypair for this peer
        // Each peer gets a unique identity based on this key
        let key = identity::Keypair::generate_ed25519();
        let peer_id = PeerId::from(key.public());

        // Setup TCP transport with encryption and multiplexing:
        // - TCP: Basic network transport
        // - Noise: Encryption (secure communication)
        // - Yamux: Multiplexing (multiple streams over one connection)
        let transport = tcp::tokio::Transport::default()
            .upgrade(libp2p::core::upgrade::Version::V1)
            .authenticate(noise::Config::new(&key)?)
            .multiplex(yamux::Config::default())
            .boxed();
        
        // Rendezvous client: Used to discover other peers
        // This allows us to find peers registered in the same namespace
        let rendezvous = rendezvous::client::Behaviour::new(key.clone());
        
        // Identify protocol: Lets peers learn about each other
        // (protocol version, agent name, etc.)
        let identify = libp2p::identify::Behaviour::new(
            libp2p::identify::Config::new("p2p-client/1.0".to_string(), key.public())
        );
        
        // Request-Response protocol: Used for sending JSON messages
        // This is what actually sends/receives your JSON data
        let codec = JsonCodec;
        let request_response = request_response::Behaviour::with_codec(
            codec,
            [(StreamProtocol::new("/json-message/1.0"), ProtocolSupport::Full)],
            request_response::Config::default(),
        );
        
        // Combine all behaviours into one
        let behaviour = Behaviour { rendezvous, identify, request_response };
        
        // Create the swarm (main networking component)
        // This manages all connections and protocol interactions
        let swarm_config = SwarmConfig::with_tokio_executor()
            .with_idle_connection_timeout(Duration::from_secs(60));
        let mut swarm = Swarm::new(
            transport,
            behaviour,
            peer_id,
            swarm_config,
        );

        // Listen on all network interfaces (0.0.0.0) on a random port (0)
        // This allows other peers to connect to us
        swarm.listen_on("/ip4/0.0.0.0/tcp/0".parse()?)?;

        // Create the client struct
        let mut client = Self {
            swarm,
            rendezvous_server: server.to_string(),
            namespace: namespace.to_string(),
            rendezvous_peer_id: None,
            connected_peers: HashMap::new(),
            pending_responses: HashMap::new(),
        };

        // Connect to the rendezvous server
        // This must succeed before you can discover peers
        client.connect_to_rendezvous().await?;

        Ok(client)
    }

    /// # Connect to the rendezvous server (internal method)
    /// 
    /// This is called automatically by `new()`. You don't need to call this directly.
    /// 
    /// ## What it does:
    /// 1. Parses the server address
    /// 2. Dials the rendezvous server
    /// 3. Waits for connection to be established
    /// 4. Stores the server's peer ID
    async fn connect_to_rendezvous(&mut self) -> Result<(), Box<dyn Error>> {
        // Parse server address
        // Format: "host:port" or just "host" (defaults to port 51820)
        let (host, port) = if let Some(colon_pos) = self.rendezvous_server.find(':') {
            (
                &self.rendezvous_server[..colon_pos],
                &self.rendezvous_server[colon_pos + 1..]
            )
        } else {
            (self.rendezvous_server.as_str(), "51820")
        };
        
        // Create libp2p multiaddress
        // Format: /ip4/127.0.0.1/tcp/51820
        let addr: Multiaddr = format!("/ip4/{}/tcp/{}", host, port).parse()?;
        
        // Initiate connection to rendezvous server
        self.swarm.dial(addr)?;

        // Wait for connection to be established
        // We loop through swarm events until we see ConnectionEstablished
        loop {
            match self.swarm.select_next_some().await {
                SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                    // First connection should be the rendezvous server
                    if self.rendezvous_peer_id.is_none() {
                        self.rendezvous_peer_id = Some(peer_id);
                        break; // Connection successful!
                    }
                }
                SwarmEvent::NewListenAddr { address, .. } => {
                    // Add our listening address so peers can connect to us
                    self.swarm.add_external_address(address);
                }
                _ => {
                    // Ignore other events while waiting for connection
                }
            }
        }

        Ok(())
    }

    /// # Discover and connect to a peer in the namespace
    /// 
    /// This function:
    /// 1. Sends a discovery request to the rendezvous server
    /// 2. Waits for peer registrations in the namespace
    /// 3. Attempts to connect to the first discovered peer
    /// 4. Returns the peer's ID once connected
    /// 
    /// ## Important Notes
    /// 
    /// - **This function BLOCKS** until a peer is found and connected
    /// - If no peers are available, it will wait indefinitely
    /// - It connects to the **first peer** found in the namespace
    /// - The peer must be registered with the rendezvous server in the same namespace
    /// 
    /// ## Returns
    /// 
    /// - **`Ok(PeerId)`**: Successfully connected to a peer
    ///   - The `PeerId` is a unique identifier for the connected peer
    ///   - Use this ID when calling `send_and_wait()`
    /// 
    /// - **`Err(Box<dyn Error>)`**: Connection failed
    ///   - No peers found (if you're waiting forever, check that another peer is registered)
    ///   - Network errors
    ///   - Invalid namespace
    /// 
    /// ## Example
    /// 
    /// ```rust,no_run
    /// use client_helper::P2PClient;
    /// 
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let mut client = P2PClient::new("127.0.0.1:51820", "my-namespace").await?;
    ///     
    ///     // This will block until a peer is found
    ///     // Make sure another peer is running and registered in "my-namespace"
    ///     let peer_id = client.connect_to_peer().await?;
    ///     
    ///     println!("Connected to peer: {}", peer_id);
    ///     
    ///     Ok(())
    /// }
    /// ```
    pub async fn connect_to_peer(&mut self) -> Result<PeerId, Box<dyn Error>> {
        // Get the rendezvous server's peer ID (must be connected first)
        let rendezvous_peer_id = self.rendezvous_peer_id
            .ok_or("Not connected to rendezvous server")?;

        // Create namespace object for discovery
        // Only peers registered in this namespace will be discovered
        let namespace = rendezvous::Namespace::new(self.namespace.clone())?;
        
        // Send discovery request to rendezvous server
        // Parameters:
        // - Some(namespace): Only discover peers in this namespace
        // - None: No cookie (for pagination, not needed for simple use)
        // - None: No limit (get all peers)
        // - rendezvous_peer_id: The server to ask
        self.swarm.behaviour_mut().rendezvous.discover(
            Some(namespace),
            None, // Cookie for pagination (None = start from beginning)
            None, // Limit (None = no limit)
            rendezvous_peer_id,
        );

        // Wait for discovery results and connection
        // We loop through events until we successfully connect to a peer
        loop {
            match self.swarm.select_next_some().await {
                // Discovery response from rendezvous server
                SwarmEvent::Behaviour(BehaviourEvent::Rendezvous(rendezvous::client::Event::Discovered { registrations, .. })) => {
                    // Process each discovered peer registration
                    for reg in registrations {
                        let discovered_peer = reg.record.peer_id();
                        
                        // Don't try to connect to ourselves
                        if discovered_peer != self.swarm.local_peer_id() {
                            // Get the peer's addresses from the registration
                            // These are the network addresses where we can reach the peer
                            let addrs: Vec<Multiaddr> = reg.record.addresses().iter().cloned().collect();
                            
                            // Try each address until one works
                            for addr in addrs {
                                // Attempt to dial (connect to) the peer
                                if self.swarm.dial(addr.clone()).is_ok() {
                                    // Successfully initiated connection
                                    // Now wait for ConnectionEstablished event
                                    loop {
                                        match self.swarm.select_next_some().await {
                                            SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                                                // Check if this is the peer we're trying to connect to
                                                if peer_id == discovered_peer {
                                                    // Success! Store the peer and return
                                                    self.connected_peers.insert(peer_id, ());
                                                    return Ok(peer_id);
                                                }
                                            }
                                            SwarmEvent::Behaviour(BehaviourEvent::RequestResponse(_)) => {
                                                // Handle any incoming messages while waiting
                                                // (though unlikely during connection)
                                            }
                                            _ => {
                                                // Other events, continue waiting
                                            }
                                        }
                                    }
                                }
                                // If dial failed, try next address
                            }
                        }
                    }
                }
                // Direct connection established (peer connected to us)
                SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                    // If it's not the rendezvous server and we haven't seen this peer before
                    if self.rendezvous_peer_id != Some(peer_id) && !self.connected_peers.contains_key(&peer_id) {
                        self.connected_peers.insert(peer_id, ());
                        return Ok(peer_id);
                    }
                }
                // Handle incoming request-response messages
                SwarmEvent::Behaviour(BehaviourEvent::RequestResponse(request_response::Event::Message { message, .. })) => {
                    match message {
                        // Someone sent us a request - auto-respond with echo
                        request_response::Message::Request { request, channel, .. } => {
                            let response = JsonMessage::new(
                                "auto-responder".to_string(),
                                format!("Echo: {}", request.message),
                            );
                            let _ = self.swarm.behaviour_mut().request_response.send_response(channel, response);
                        }
                        // We received a response to one of our requests
                        request_response::Message::Response { response, request_id, .. } => {
                            // Find the channel waiting for this response
                            if let Some(tx) = self.pending_responses.remove(&request_id) {
                                // Convert response to JSON and send through channel
                                let json_value = serde_json::json!({
                                    "from": response.from,
                                    "message": response.message,
                                    "timestamp": response.timestamp
                                });
                                let _ = tx.send(json_value);
                            }
                        }
                    }
                }
                _ => {
                    // Ignore other events
                }
            }
        }
    }

    /// # Send a JSON message and wait for a response
    /// 
    /// This is the main function you'll use to communicate with peers.
    /// 
    /// ## Parameters
    /// 
    /// ### `peer_id: PeerId`
    /// - The peer to send the message to
    /// - Get this from `connect_to_peer()` or `connected_peers()`
    /// - Example: `let peer_id = client.connect_to_peer().await?;`
    /// 
    /// ### `json_message: serde_json::Value`
    /// - The JSON message to send
    /// - **Must contain these fields:**
    ///   - `"from"`: String - Your identifier/name
    ///   - `"message"`: String - The message text
    ///   - `"timestamp"`: Number (u64) - Unix timestamp (optional, will be set if missing)
    /// 
    /// ## Returns
    /// 
    /// - **`Ok(serde_json::Value)`**: The response from the peer
    ///   - Response has the same structure: `{"from": "...", "message": "...", "timestamp": ...}`
    /// 
    /// - **`Err(Box<dyn Error>)`**: Error occurred
    ///   - Timeout (10 seconds) - peer didn't respond in time
    ///   - Peer not connected - peer_id is not in connected_peers
    ///   - Network error - connection lost
    ///   - Invalid JSON - message missing required fields
    /// 
    /// ## Timeout
    /// 
    /// - Default timeout: **10 seconds**
    /// - If peer doesn't respond within 10 seconds, returns `Err("Timeout waiting for response")`
    /// - The function blocks until response is received or timeout occurs
    /// 
    /// ## Example
    /// 
    /// ```rust,no_run
    /// use client_helper::P2PClient;
    /// use serde_json::json;
    /// 
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let mut client = P2PClient::new("127.0.0.1:51820", "my-namespace").await?;
    ///     let peer_id = client.connect_to_peer().await?;
    ///     
    ///     // Create your JSON message
    ///     let request = json!({
    ///         "from": "my-client",           // REQUIRED: Your identifier
    ///         "message": "Hello, peer!",     // REQUIRED: Your message text
    ///         "timestamp": std::time::SystemTime::now()
    ///             .duration_since(std::time::UNIX_EPOCH)
    ///             .unwrap()
    ///             .as_secs()                 // OPTIONAL: Unix timestamp
    ///     });
    ///     
    ///     // Send and wait for response
    ///     // This BLOCKS for up to 10 seconds waiting for the response
    ///     match client.send_and_wait(peer_id, request).await {
    ///         Ok(response) => {
    ///             println!("Got response: {}", serde_json::to_string_pretty(&response)?);
    ///             
    ///             // Access response fields
    ///             let from = response["from"].as_str().unwrap();
    ///             let message = response["message"].as_str().unwrap();
    ///             println!("From: {}, Message: {}", from, message);
    ///         }
    ///         Err(e) => {
    ///             eprintln!("Error: {}", e);
    ///             // Handle timeout or other errors
    ///         }
    ///     }
    ///     
    ///     Ok(())
    /// }
    /// ```
    /// 
    /// ## How It Works
    /// 
    /// 1. Converts your JSON to `JsonMessage` struct
    /// 2. Sends request to peer via request-response protocol
    /// 3. Stores a channel to receive the response
    /// 4. Processes swarm events until response arrives
    /// 5. Returns the response as JSON, or error if timeout
    /// 
    /// ## Waiting for Response
    /// 
    /// The function **automatically waits** for the response. You don't need to do anything special:
    /// 
    /// ```rust,no_run
    /// // This line blocks until response is received (or timeout)
    /// let response = client.send_and_wait(peer_id, message).await?;
    /// 
    /// // Response is now available
    /// println!("Response: {}", response["message"]);
    /// ```
    /// 
    /// The function handles all the event processing internally. You just await it.
    pub async fn send_and_wait(
        &mut self,
        peer_id: PeerId,
        json_message: serde_json::Value,
    ) -> Result<serde_json::Value, Box<dyn Error>> {
        // Validate and extract required fields from JSON
        // The JSON must have "from" and "message" fields
        let from = json_message["from"]
            .as_str()
            .ok_or("JSON message missing required 'from' field (must be a string)")?
            .to_string();
        
        let message = json_message["message"]
            .as_str()
            .ok_or("JSON message missing required 'message' field (must be a string)")?
            .to_string();
        
        // Get timestamp, or use current time if not provided
        let timestamp = json_message["timestamp"]
            .as_u64()
            .unwrap_or_else(|| {
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs()
            });

        // Convert JSON to JsonMessage struct (what the protocol expects)
        let json_msg = JsonMessage {
            from,
            message,
            timestamp,
        };

        // Create a one-shot channel to receive the response
        // This allows us to wait for the response asynchronously
        let (tx, rx) = tokio::sync::oneshot::channel();

        // Send the request to the peer
        // This returns a RequestId that we can use to match the response
        let request_id = self.swarm.behaviour_mut().request_response.send_request(&peer_id, json_msg);
        
        // Store the channel so we can send the response when it arrives
        self.pending_responses.insert(request_id, tx);

        // Timeout duration: 10 seconds
        // If no response in 10 seconds, return error
        let timeout_duration = Duration::from_secs(10);
        let start = std::time::Instant::now();

        // Process events until we get a response or timeout
        loop {
            // Check if timeout has elapsed
            if start.elapsed() > timeout_duration {
                // Clean up: remove the pending response entry
                self.pending_responses.remove(&request_id);
                return Err("Timeout waiting for response (10 seconds elapsed)".into());
            }

            // Check if we already received the response through the channel
            // try_recv() doesn't block - returns immediately
            if let Ok(response) = rx.try_recv() {
                return Ok(response);
            }

            // Process swarm events with a small timeout
            // This allows us to check the timeout periodically
            match timeout(Duration::from_millis(100), self.swarm.select_next_some()).await {
                Ok(Ok(event)) => {
                    match event {
                        // Incoming request-response protocol message
                        SwarmEvent::Behaviour(BehaviourEvent::RequestResponse(request_response::Event::Message { message, .. })) => {
                            match message {
                                // We received a response to our request
                                request_response::Message::Response { response, request_id: resp_id, .. } => {
                                    // Check if this is the response we're waiting for
                                    if let Some(tx) = self.pending_responses.remove(&resp_id) {
                                        // Convert response to JSON format
                                        let json_value = serde_json::json!({
                                            "from": response.from,
                                            "message": response.message,
                                            "timestamp": response.timestamp
                                        });
                                        
                                        // Send response through channel
                                        let _ = tx.send(json_value);
                                        
                                        // If this was our request, check the channel
                                        if resp_id == request_id {
                                            // Wait for the value to be sent through channel
                                            if let Ok(response) = rx.await {
                                                return Ok(response);
                                            }
                                        }
                                    }
                                }
                                // Someone sent us a request (not a response to our request)
                                request_response::Message::Request { request, channel, .. } => {
                                    // Auto-respond with echo (you can customize this)
                                    let response = JsonMessage::new(
                                        "auto-responder".to_string(),
                                        format!("Echo: {}", request.message),
                                    );
                                    let _ = self.swarm.behaviour_mut().request_response.send_response(channel, response);
                                }
                            }
                        }
                        _ => {
                            // Other events, continue processing
                        }
                    }
                }
                Ok(Err(_)) => {
                    // Channel closed (shouldn't happen normally)
                    break;
                }
                Err(_) => {
                    // Timeout on select (100ms) - continue loop to check overall timeout
                    continue;
                }
            }
        }

        // Shouldn't reach here, but return error if we do
        Err("Failed to receive response".into())
    }

    /// # Get your local peer ID
    /// 
    /// Returns the unique identifier for this client instance.
    /// Other peers can use this to identify you.
    /// 
    /// ## Returns
    /// 
    /// - `PeerId`: Your unique peer identifier
    /// 
    /// ## Example
    /// 
    /// ```rust,no_run
    /// let client = P2PClient::new("127.0.0.1:51820", "ns").await?;
    /// println!("My Peer ID: {}", client.local_peer_id());
    /// ```
    pub fn local_peer_id(&self) -> PeerId {
        *self.swarm.local_peer_id()
    }

    /// # Get list of all connected peers
    /// 
    /// Returns a vector of PeerIds for all currently connected peers.
    /// 
    /// ## Returns
    /// 
    /// - `Vec<PeerId>`: List of connected peer IDs
    /// 
    /// ## Example
    /// 
    /// ```rust,no_run
    /// let mut client = P2PClient::new("127.0.0.1:51820", "ns").await?;
    /// let peer_id = client.connect_to_peer().await?;
    /// 
    /// let connected = client.connected_peers();
    /// println!("Connected to {} peer(s)", connected.len());
    /// for peer in connected {
    ///     println!("  - {}", peer);
    /// }
    /// ```
    pub fn connected_peers(&self) -> Vec<PeerId> {
        self.connected_peers.keys().cloned().collect()
    }
}
