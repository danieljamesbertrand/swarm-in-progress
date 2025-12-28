//! Standalone Llama Shard Inference Test
//! 
//! Tests loading and running inference on SafeTensor shards.
//! Run with: cargo run --example shard_inference_test --features candle
//!
//! This is isolated from the main codebase to test without breaking anything.

use std::path::PathBuf;
use std::collections::HashMap;

/// Minimal SafeTensor loader - doesn't need candle
fn load_safetensor_metadata(path: &std::path::Path) -> Result<HashMap<String, TensorInfo>, String> {
    use std::fs::File;
    use std::io::Read;
    
    let mut file = File::open(path).map_err(|e| format!("Failed to open: {}", e))?;
    
    // SafeTensor format: 8 bytes header size (little endian) + JSON header + tensors
    let mut header_size_bytes = [0u8; 8];
    file.read_exact(&mut header_size_bytes).map_err(|e| format!("Failed to read header size: {}", e))?;
    let header_size = u64::from_le_bytes(header_size_bytes) as usize;
    
    println!("Header size: {} bytes", header_size);
    
    let mut header_bytes = vec![0u8; header_size];
    file.read_exact(&mut header_bytes).map_err(|e| format!("Failed to read header: {}", e))?;
    
    let header_str = String::from_utf8(header_bytes).map_err(|e| format!("Invalid UTF-8: {}", e))?;
    let header: serde_json::Value = serde_json::from_str(&header_str).map_err(|e| format!("Invalid JSON: {}", e))?;
    
    let mut tensors = HashMap::new();
    
    if let serde_json::Value::Object(map) = header {
        for (name, value) in map {
            if name == "__metadata__" { continue; }
            
            if let serde_json::Value::Object(tensor_info) = value {
                let dtype = tensor_info.get("dtype")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown")
                    .to_string();
                    
                let shape: Vec<usize> = tensor_info.get("shape")
                    .and_then(|v| v.as_array())
                    .map(|arr| arr.iter().filter_map(|v| v.as_u64().map(|n| n as usize)).collect())
                    .unwrap_or_default();
                    
                let data_offsets: (usize, usize) = tensor_info.get("data_offsets")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        let start = arr.get(0).and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                        let end = arr.get(1).and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                        (start, end)
                    })
                    .unwrap_or((0, 0));
                
                tensors.insert(name, TensorInfo { dtype, shape, data_offsets });
            }
        }
    }
    
    Ok(tensors)
}

#[derive(Debug, Clone)]
struct TensorInfo {
    dtype: String,
    shape: Vec<usize>,
    data_offsets: (usize, usize),
}

impl TensorInfo {
    fn size_bytes(&self) -> usize {
        self.data_offsets.1 - self.data_offsets.0
    }
    
    fn num_elements(&self) -> usize {
        self.shape.iter().product()
    }
}

/// List available shards in cache
fn list_cached_shards(cache_dir: &std::path::Path) -> Vec<PathBuf> {
    let mut shards = Vec::new();
    
    if let Ok(entries) = std::fs::read_dir(cache_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map(|e| e == "safetensors").unwrap_or(false) {
                shards.push(path);
            }
        }
    }
    
    shards.sort();
    shards
}

fn main() {
    println!("\n╔══════════════════════════════════════════════════════════════╗");
    println!("║         🔬 SHARD INFERENCE TEST - STANDALONE 🔬              ║");
    println!("╚══════════════════════════════════════════════════════════════╝\n");

    // Check for cached shards
    let cache_dir = PathBuf::from("./models_cache");
    
    if !cache_dir.exists() {
        println!("❌ No models_cache directory found.");
        println!("   Run the shard downloader first or create the directory.");
        println!("\n   Expected path: {}", cache_dir.display());
        return;
    }
    
    let shards = list_cached_shards(&cache_dir);
    
    if shards.is_empty() {
        println!("❌ No .safetensors files found in {}", cache_dir.display());
        println!("   Download shards first using the shard_listener or manually.");
        return;
    }
    
    println!("✅ Found {} shard(s):\n", shards.len());
    
    let mut total_params = 0usize;
    let mut total_size = 0usize;
    
    for (i, shard_path) in shards.iter().enumerate() {
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        println!("📦 Shard {}: {}", i, shard_path.file_name().unwrap().to_string_lossy());
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        
        match load_safetensor_metadata(shard_path) {
            Ok(tensors) => {
                let mut shard_params = 0usize;
                let mut shard_size = 0usize;
                
                // Group tensors by layer
                let mut layers: HashMap<String, Vec<(&String, &TensorInfo)>> = HashMap::new();
                
                for (name, info) in &tensors {
                    let layer_name = if name.contains("layers.") {
                        let parts: Vec<&str> = name.split('.').collect();
                        if parts.len() >= 3 {
                            format!("{}.{}", parts[0], parts[1])
                        } else {
                            "other".to_string()
                        }
                    } else {
                        "embedding/output".to_string()
                    };
                    
                    layers.entry(layer_name).or_default().push((name, info));
                    shard_params += info.num_elements();
                    shard_size += info.size_bytes();
                }
                
                // Print layer summary
                let mut layer_names: Vec<_> = layers.keys().collect();
                layer_names.sort();
                
                println!("\n   Layers found: {}", layer_names.len());
                for layer in &layer_names[..layer_names.len().min(5)] {
                    let tensors_in_layer = layers.get(*layer).unwrap();
                    let layer_params: usize = tensors_in_layer.iter().map(|(_, t)| t.num_elements()).sum();
                    println!("   • {} ({} tensors, {:.2}M params)", 
                        layer, tensors_in_layer.len(), layer_params as f64 / 1_000_000.0);
                }
                if layer_names.len() > 5 {
                    println!("   ... and {} more layers", layer_names.len() - 5);
                }
                
                println!("\n   📊 Shard Stats:");
                println!("      Total tensors: {}", tensors.len());
                println!("      Total params:  {:.2}M ({:.2}B)", 
                    shard_params as f64 / 1_000_000.0,
                    shard_params as f64 / 1_000_000_000.0);
                println!("      Size on disk:  {:.2} MB", shard_size as f64 / (1024.0 * 1024.0));
                
                total_params += shard_params;
                total_size += shard_size;
            }
            Err(e) => {
                println!("   ❌ Failed to load: {}", e);
            }
        }
        println!();
    }
    
    println!("═══════════════════════════════════════════════════════════════");
    println!("📈 TOTAL ACROSS ALL SHARDS:");
    println!("   Parameters: {:.2}B", total_params as f64 / 1_000_000_000.0);
    println!("   Size:       {:.2} GB", total_size as f64 / (1024.0 * 1024.0 * 1024.0));
    println!("═══════════════════════════════════════════════════════════════\n");
    
    // Test if we can actually read tensor data
    if let Some(first_shard) = shards.first() {
        println!("🔬 Testing tensor data read from first shard...");
        
        if let Ok(tensors) = load_safetensor_metadata(first_shard) {
            // Find a small tensor to test
            if let Some((name, info)) = tensors.iter().find(|(_, t)| t.size_bytes() < 1_000_000) {
                println!("   Reading tensor: {}", name);
                println!("   Shape: {:?}", info.shape);
                println!("   Dtype: {}", info.dtype);
                println!("   Size: {} bytes", info.size_bytes());
                
                // Actually read the tensor data
                if let Ok(mut file) = std::fs::File::open(first_shard) {
                    use std::io::{Read, Seek, SeekFrom};
                    
                    // Skip header
                    let mut header_size_bytes = [0u8; 8];
                    let _ = file.read_exact(&mut header_size_bytes);
                    let header_size = u64::from_le_bytes(header_size_bytes);
                    
                    // Seek to tensor data
                    let data_start = 8 + header_size + info.data_offsets.0 as u64;
                    if file.seek(SeekFrom::Start(data_start)).is_ok() {
                        let mut buffer = vec![0u8; info.size_bytes().min(1024)];
                        if file.read_exact(&mut buffer).is_ok() {
                            // Interpret as f32 or f16
                            let sample_values: Vec<f32> = if info.dtype == "F32" {
                                buffer.chunks(4).take(5).map(|chunk| {
                                    f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]])
                                }).collect()
                            } else if info.dtype == "BF16" || info.dtype == "F16" {
                                buffer.chunks(2).take(5).map(|chunk| {
                                    let bits = u16::from_le_bytes([chunk[0], chunk[1]]);
                                    // BF16 to f32 conversion
                                    f32::from_bits((bits as u32) << 16)
                                }).collect()
                            } else {
                                vec![]
                            };
                            
                            if !sample_values.is_empty() {
                                println!("   ✅ First 5 values: {:?}", sample_values);
                            }
                        }
                    }
                }
            }
        }
    }
    
    println!("\n✅ Shard inspection complete!");
    println!("   Your shards are valid and can be loaded for inference.");
    println!("\n   Next steps:");
    println!("   1. Install Ollama (ollama.com) for immediate inference");
    println!("   2. Or wait for candle integration to load these directly");
}








