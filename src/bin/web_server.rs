//! Promethos-AI Web Server
//! 
//! WebSocket server that connects the web console to the Llama inference engine.
//! 
//! Run with: cargo run --bin web_server
//! Then open: http://localhost:8080

use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::net::{TcpListener, TcpStream};
use tokio::time::{sleep, Duration, Instant};
use tokio_tungstenite::{accept_async, tungstenite::Message};
use futures_util::{StreamExt, SinkExt};
use serde::{Deserialize, Serialize};
use punch_simple::pipeline_coordinator::{PipelineCoordinator, InferenceRequest, PipelineStrategy, NodeSpawner};
use punch_simple::kademlia_shard_discovery::KademliaShardDiscovery;
use punch_simple::llama_model_loader::LlamaModelManager;
use punch_simple::message::{JsonMessage, JsonCodec};
use punch_simple::command_protocol::{Command, CommandResponse, commands};
use libp2p::{
    identity,
    tcp,
    noise,
    yamux,
    kad,
    relay,
    request_response::{self, ProtocolSupport},
    swarm::{NetworkBehaviour, Swarm, SwarmEvent},
    core::transport::Transport,
    PeerId, Multiaddr, StreamProtocol,
};
use libp2p::swarm::Config as SwarmConfig;
use libp2p::futures::StreamExt as Libp2pStreamExt;

/// Query request from web client
#[derive(Deserialize)]
struct QueryRequest {
    query: String,
    #[serde(default)]
    request_id: Option<String>,
}

/// Response to web client
#[derive(Serialize)]
struct QueryResponse {
    response: String,
    tokens: usize,
    latency_ms: u64,
    shards_used: Vec<ShardInfo>,
    success: bool,
    request_id: Option<String>,
}

/// Pipeline status update
#[derive(Serialize)]
struct PipelineUpdate {
    stage: String,
    status: String, // "waiting", "processing", "complete", "error"
    shard_id: Option<u32>,
    latency_ms: Option<u64>,
}

/// Shard info for response
#[derive(Serialize, Clone)]
struct ShardInfo {
    shard_id: u32,
    layer_start: u32,
    layer_end: u32,
    latency_ms: u64,
}

/// Simulated shard node
struct ShardNode {
    shard_id: u32,
    layer_start: u32,
    layer_end: u32,
    has_embeddings: bool,
    has_output: bool,
}

impl ShardNode {
    fn new(shard_id: u32, total_shards: u32, total_layers: u32) -> Self {
        let layers_per_shard = total_layers / total_shards;
        let layer_start = shard_id * layers_per_shard;
        let layer_end = if shard_id == total_shards - 1 {
            total_layers
        } else {
            (shard_id + 1) * layers_per_shard
        };
        
        Self {
            shard_id,
            layer_start,
            layer_end,
            has_embeddings: shard_id == 0,
            has_output: shard_id == total_shards - 1,
        }
    }
}

/// The inference engine - uses real distributed pipeline
struct InferenceEngine {
    coordinator: Arc<PipelineCoordinator>,
    peer_id: PeerId,
}

impl InferenceEngine {
    async fn new(bootstrap: &str) -> Result<Self, Box<dyn std::error::Error>> {
        // Generate peer identity
        let key = identity::Keypair::generate_ed25519();
        let peer_id = PeerId::from(key.public());
        
        // Create discovery
        let discovery = KademliaShardDiscovery::with_expected_shards("llama-cluster", 4);
        
        // Create node spawner for on-demand node creation
        let spawner = NodeSpawner::new(
            bootstrap.to_string(),
            "llama-cluster".to_string(),
            4,  // total_shards
            32, // total_layers
            "llama-8b".to_string(),
            "models_cache/shards".to_string(),
        );

        // Create pipeline coordinator with spawner and strategy
        let mut coordinator = PipelineCoordinator::new(discovery)
            .with_node_spawner(spawner);
        coordinator.set_strategy(PipelineStrategy::Adaptive {
            wait_timeout_secs: 30,
            min_memory_for_shard_mb: 4096,
            min_memory_for_full_mb: 16384,
        });
        let coordinator = Arc::new(coordinator);
        
        // Note: DHT discovery will happen when shard listeners connect
        // The coordinator will discover shards as they announce themselves
        
        Ok(Self {
            coordinator,
            peer_id,
        })
    }

    async fn process_query(&self, query: &str, update_sender: Option<&tokio::sync::mpsc::Sender<PipelineUpdate>>) -> QueryResponse {
        let start = Instant::now();

        // Send initial status
        if let Some(sender) = update_sender {
            let _ = sender.send(PipelineUpdate {
                stage: "input".to_string(),
                status: "processing".to_string(),
                shard_id: None,
                latency_ms: None,
            }).await;
        }

        // Create inference request
        let inference_request = InferenceRequest::new(query)
            .with_max_tokens(256)
            .with_temperature(0.7);

        if let Some(sender) = update_sender {
            let _ = sender.send(PipelineUpdate {
                stage: "discovery".to_string(),
                status: "processing".to_string(),
                shard_id: None,
                latency_ms: None,
            }).await;
        }

        // Submit to pipeline coordinator
        let result = self.coordinator.submit_inference(inference_request).await;

        if let Some(sender) = update_sender {
            let _ = sender.send(PipelineUpdate {
                stage: "discovery".to_string(),
                status: "complete".to_string(),
                shard_id: None,
                latency_ms: Some(100),
            }).await;
        }

        match result {
            Ok(response) => {
                // Send shard processing updates
                for (_idx, shard_latency) in response.shard_latencies.iter().enumerate() {
                    if let Some(sender) = update_sender {
                        let _ = sender.send(PipelineUpdate {
                            stage: format!("shard{}", shard_latency.shard_id),
                            status: "processing".to_string(),
                            shard_id: Some(shard_latency.shard_id),
                            latency_ms: None,
                        }).await;
                        
                        tokio::time::sleep(Duration::from_millis(10)).await;
                        
                        let _ = sender.send(PipelineUpdate {
                            stage: format!("shard{}", shard_latency.shard_id),
                            status: "complete".to_string(),
                            shard_id: Some(shard_latency.shard_id),
                            latency_ms: Some(shard_latency.latency_ms as u64),
                        }).await;
                    }
                }

                if let Some(sender) = update_sender {
                    let _ = sender.send(PipelineUpdate {
                        stage: "output".to_string(),
                        status: "processing".to_string(),
                        shard_id: None,
                        latency_ms: None,
                    }).await;
                }

                let shard_infos: Vec<ShardInfo> = response.shard_latencies.iter().map(|sl| {
                    ShardInfo {
                        shard_id: sl.shard_id,
                        layer_start: 0, // Will be filled from shard announcement
                        layer_end: 0,
                        latency_ms: sl.latency_ms as u64,
                    }
                }).collect();

                if let Some(sender) = update_sender {
                    let _ = sender.send(PipelineUpdate {
                        stage: "output".to_string(),
                        status: "complete".to_string(),
                        shard_id: None,
                        latency_ms: Some(50),
                    }).await;
                }

                QueryResponse {
                    response: response.text,
                    tokens: response.tokens_generated as usize,
                    latency_ms: response.total_latency_ms as u64,
                    shards_used: shard_infos,
                    success: response.success,
                    request_id: Some(response.request_id),
                }
            }
            Err(e) => {
                let error_msg = format!("Pipeline error: {}", e);
                eprintln!("[INFERENCE] {}", error_msg);
                
                QueryResponse {
                    response: error_msg,
                    tokens: 0,
                    latency_ms: start.elapsed().as_millis() as u64,
                    shards_used: vec![],
                    success: false,
                    request_id: None,
                }
            }
        }
    }
}

/// Generate contextual responses (DEPRECATED - now using real inference)
#[allow(dead_code)]
fn generate_response(query: &str) -> String {
    let q = query.to_lowercase();
    
    // Music questions
    if q.contains("pinball wizard") {
        return "**Pete Townshend** wrote \"Pinball Wizard\" for The Who's 1969 rock opera \"Tommy\". The song tells the story of a \"deaf, dumb and blind kid\" who becomes a pinball champion. It reached #4 in the UK and #19 in the US. Elton John later covered it for the 1975 film.".to_string();
    }
    
    if q.contains("wonderwall") {
        return "**Noel Gallagher** wrote \"Wonderwall\" for **Oasis** in 1995. It appeared on \"(What's the Story) Morning Glory?\" and reached #2 in the UK. Noel said it's about \"an imaginary friend who's gonna save you from yourself.\" It's one of the most-covered songs ever.".to_string();
    }
    
    if q.contains("bohemian rhapsody") {
        return "**Freddie Mercury** wrote \"Bohemian Rhapsody\" for **Queen** in 1975. The 6-minute epic features an intro, ballad, operatic section, hard rock segment, and outro. Despite being \"too long for radio,\" it became one of the best-selling singles of all time.".to_string();
    }

    if q.contains("twist and shout") || q.contains("twist & shout") {
        return "\"Twist and Shout\" was written by **Phil Medley** and **Bert Berns** in 1961. The Beatles' 1963 version is most famous - recorded in one take at the end of a 10-hour session when John Lennon's voice was nearly gone, giving it that raw, powerful sound.".to_string();
    }

    if q.contains("imagine") && !q.contains("dragon") {
        return "**John Lennon** wrote \"Imagine\" in 1971. It envisions a world without borders, religion, or possessions. Yoko Ono was credited as co-writer in 2017. It's been voted the best song of the 20th century and remains an anthem for peace movements worldwide.".to_string();
    }

    if q.contains("stairway to heaven") {
        return "**Jimmy Page** (music) and **Robert Plant** (lyrics) wrote \"Stairway to Heaven\" for Led Zeppelin in 1971. At 8 minutes, it builds from acoustic to thundering rock. Never released as a single, yet became the most-requested song in radio history.".to_string();
    }

    if q.contains("hotel california") {
        return "**Don Felder** wrote the music, **Don Henley** and **Glenn Frey** wrote the lyrics to \"Hotel California\" for the Eagles in 1977. Often interpreted as a metaphor for excess in the music industry. The guitar outro with Felder and Joe Walsh is iconic.".to_string();
    }

    if q.contains("smells like teen spirit") {
        return "**Kurt Cobain** wrote \"Smells Like Teen Spirit\" for Nirvana in 1991. The title came from graffiti by Kathleen Hanna (referencing a deodorant brand). It knocked Michael Jackson off #1 and defined the grunge movement. Cobain grew to hate it due to its popularity.".to_string();
    }

    if q.contains("yesterday") && q.contains("beatles") || (q.contains("yesterday") && q.contains("wrote")) {
        return "**Paul McCartney** wrote \"Yesterday\" for the Beatles in 1965. It's the most-covered song in history with 2,200+ versions. McCartney woke up with the melody and initially used \"Scrambled eggs\" as placeholder lyrics. It was the first Beatles song featuring just one member.".to_string();
    }

    if q.contains("like a rolling stone") {
        return "**Bob Dylan** wrote \"Like a Rolling Stone\" in 1965. Rolling Stone magazine ranked it #1 greatest song of all time. At 6 minutes, it broke radio conventions. The opening snare hit by Bobby Gregg is one of rock's most famous drum sounds.".to_string();
    }

    if q.contains("sweet home alabama") {
        return "Lynyrd Skynyrd's **Ronnie Van Zant**, **Ed King**, and **Gary Rossington** wrote \"Sweet Home Alabama\" in 1974. It was a response to Neil Young's \"Southern Man.\" Despite the lyrical rivalry, Van Zant was a huge Neil Young fan and wore his t-shirt on stage.".to_string();
    }

    // Capital cities
    if q.contains("capital") && q.contains("france") {
        return "The capital of **France** is **Paris**. Located on the Seine River, it's known as the \"City of Light.\" Key landmarks include the Eiffel Tower (1889), Louvre Museum, Notre-Dame Cathedral, and Arc de Triomphe. Population: 2.1 million (12 million metro).".to_string();
    }
    
    if q.contains("capital") && q.contains("japan") {
        return "The capital of **Japan** is **Tokyo**. With 37 million people, it's the world's most populous metro area. Famous districts include Shibuya, Shinjuku, and Akihabara. It blends ancient temples like Senso-ji with ultramodern architecture and technology.".to_string();
    }
    
    if q.contains("capital") && q.contains("germany") {
        return "The capital of **Germany** is **Berlin**. Population: 3.7 million. It's been the capital of reunified Germany since 1990. Key sites include Brandenburg Gate, the Reichstag, Berlin Wall remnants, and Museum Island (UNESCO World Heritage).".to_string();
    }

    if q.contains("capital") && q.contains("italy") {
        return "The capital of **Italy** is **Rome**. Founded in 753 BC, it was the center of the Roman Empire. Home to the Vatican City, Colosseum, Pantheon, and Trevi Fountain. Population: 2.8 million. It's called \"The Eternal City.\"".to_string();
    }

    if q.contains("capital") && q.contains("spain") {
        return "The capital of **Spain** is **Madrid**. Located in the center of the Iberian Peninsula, it's Spain's largest city with 3.3 million people. Famous for the Prado Museum, Royal Palace, and vibrant nightlife. It became the capital in 1561.".to_string();
    }

    // Promethos/AI
    if q.contains("promethos") || q.contains("what are you") || q.contains("who are you") {
        return "I am **Promethos-AI**, a distributed AI running on a decentralized swarm network. Your queries are processed across 4 neural network shards via Kademlia DHT. The name references Prometheus, who brought fire to humanity - we're bringing AI to everyone through distributed computing.".to_string();
    }

    // Code
    if q.contains("rust") || q.contains("code") || q.contains("program") {
        return "Here's a Rust async example:\n\n```rust\n#[tokio::main]\nasync fn main() {\n    let result = fetch_data().await;\n    println!(\"Got: {}\", result);\n}\n\nasync fn fetch_data() -> String {\n    tokio::time::sleep(Duration::from_secs(1)).await;\n    \"Hello from async Rust!\".to_string()\n}\n```\n\nThis shows Rust's async/await with Tokio runtime.".to_string();
    }

    // Greetings
    if q.contains("hello") || q.contains("hi ") || q.starts_with("hi") || q.contains("hey") {
        return "**Hello!** 👋 I'm Promethos-AI, running on a distributed swarm network. Try asking me about:\n\n• 🎵 Music: \"Who wrote Bohemian Rhapsody?\"\n• 🌍 Geography: \"What is the capital of Japan?\"\n• 💻 Code: \"Show me some Rust code\"\n• 🤖 About me: \"What is Promethos?\"".to_string();
    }

    // Math
    if q.contains("2+2") || q.contains("2 + 2") {
        return "2 + 2 = **4**\n\nFun fact: This simple equation is processed through the same distributed pipeline as complex queries - tokenized, embedded into vectors, processed through transformer layers, and decoded into this response!".to_string();
    }

    // Weather
    if q.contains("weather") {
        return "I don't have real-time data access, but I can explain weather! It's determined by atmospheric pressure, humidity, temperature, and wind patterns. For current conditions, try weather.gov (US) or your phone's weather app.".to_string();
    }

    // Default - still informative
    format!("I processed your query \"{}\" through the distributed Promethos-AI pipeline.\n\nWhile I don't have specific information about that topic in my current knowledge base, I can help with:\n\n• 🎵 **Music**: Song writers and history\n• 🌍 **Geography**: World capitals and facts\n• 💻 **Code**: Rust programming examples\n• 🤖 **AI**: How this system works\n\nTry asking something like \"Who wrote Hotel California?\" or \"What is the capital of France?\"", query)
}

/// Handle a WebSocket connection
async fn handle_connection(stream: TcpStream, addr: SocketAddr, engine: Arc<InferenceEngine>) {
    println!("[WS] New connection from: {}", addr);
    
    let ws_stream = match accept_async(stream).await {
        Ok(ws) => ws,
        Err(e) => {
            eprintln!("[WS] Failed to accept connection: {}", e);
            return;
        }
    };

    let (mut write, mut read) = ws_stream.split();
    
    // Create channel for pipeline updates
    let (update_tx, _update_rx) = tokio::sync::mpsc::channel::<PipelineUpdate>(32);
    
    // Spawn task to send updates
    let mut write_clone = write;
    
    while let Some(msg) = read.next().await {
        match msg {
            Ok(Message::Text(text)) => {
                println!("[WS] Received: {}", text);
                
                // Parse request
                let request: QueryRequest = match serde_json::from_str(&text) {
                    Ok(r) => r,
                    Err(_) => QueryRequest { query: text, request_id: None },
                };
                
                // Process query
                let mut response = engine.process_query(&request.query, Some(&update_tx)).await;
                response.request_id = request.request_id;
                
                // Send final response
                let response_json = serde_json::to_string(&response).unwrap();
                if let Err(e) = write_clone.send(Message::Text(response_json)).await {
                    eprintln!("[WS] Failed to send response: {}", e);
                    break;
                }
            }
            Ok(Message::Close(_)) => {
                println!("[WS] Client {} disconnected", addr);
                break;
            }
            Err(e) => {
                eprintln!("[WS] Error: {}", e);
                break;
            }
            _ => {}
        }
    }
}

/// Serve static files
async fn serve_static(path: &str) -> Option<(String, Vec<u8>)> {
    let file_path = if path == "/" || path.is_empty() {
        "web/ai-console.html"
    } else {
        path.trim_start_matches('/')
    };

    let full_path = std::path::Path::new(file_path);
    
    match tokio::fs::read(full_path).await {
        Ok(content) => {
            let content_type = match full_path.extension().and_then(|e| e.to_str()) {
                Some("html") => "text/html",
                Some("css") => "text/css",
                Some("js") => "application/javascript",
                Some("json") => "application/json",
                Some("png") => "image/png",
                Some("jpg") | Some("jpeg") => "image/jpeg",
                Some("svg") => "image/svg+xml",
                _ => "application/octet-stream",
            };
            Some((content_type.to_string(), content))
        }
        Err(_) => None,
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n");
    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║          🔥 PROMETHOS-AI WEB SERVER 🔥                       ║");
    println!("╠══════════════════════════════════════════════════════════════╣");
    println!("║  Web Console: http://localhost:8080                          ║");
    println!("║  WebSocket:   ws://localhost:8081                            ║");
    println!("╚══════════════════════════════════════════════════════════════╝\n");

    // Initialize real inference engine with DHT discovery
    let bootstrap = std::env::var("BOOTSTRAP").unwrap_or_else(|_| "/ip4/127.0.0.1/tcp/51820".to_string());
    println!("[SERVER] Connecting to DHT bootstrap: {}", bootstrap);
    
    let engine = Arc::new(InferenceEngine::new(&bootstrap).await?);
    println!("[SERVER] Inference engine initialized with real distributed pipeline");

    // Start WebSocket server
    let ws_listener = TcpListener::bind("127.0.0.1:8081").await?;
    println!("[SERVER] WebSocket listening on ws://127.0.0.1:8081");

    // Start HTTP server for static files
    let http_listener = TcpListener::bind("127.0.0.1:8080").await?;
    println!("[SERVER] HTTP listening on http://127.0.0.1:8080");
    println!("\n[SERVER] Open http://localhost:8080 in your browser!\n");

    // Spawn HTTP server
    tokio::spawn(async move {
        loop {
            if let Ok((mut stream, _)) = http_listener.accept().await {
                tokio::spawn(async move {
                    use tokio::io::{AsyncReadExt, AsyncWriteExt};
                    
                    let mut buf = [0u8; 4096];
                    if let Ok(n) = stream.read(&mut buf).await {
                        let request = String::from_utf8_lossy(&buf[..n]);
                        let path = request.lines().next()
                            .and_then(|line| line.split_whitespace().nth(1))
                            .unwrap_or("/");
                        
                        let response = if let Some((content_type, body)) = serve_static(path).await {
                            let header = format!(
                                "HTTP/1.1 200 OK\r\nContent-Type: {}\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\n\r\n",
                                content_type,
                                body.len()
                            );
                            [header.into_bytes(), body].concat()
                        } else {
                            b"HTTP/1.1 404 Not Found\r\nContent-Length: 9\r\n\r\nNot Found".to_vec()
                        };
                        
                        let _ = stream.write_all(&response).await;
                    }
                });
            }
        }
    });

    // Accept WebSocket connections
    loop {
        let (stream, addr) = ws_listener.accept().await?;
        let engine_clone = Arc::clone(&engine);
        tokio::spawn(handle_connection(stream, addr, engine_clone));
    }
}

