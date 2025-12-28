//! Split GGUF file into shards respecting tensor boundaries
//! 
//! Run with: cargo run --example split_gguf_rust -- <input.gguf> <num_shards>

use std::fs::File;
use std::io::{Read, Write, Seek, SeekFrom};
use std::path::PathBuf;
use std::env;

// GGUF format constants
const GGUF_MAGIC: &[u8] = b"GGUF";
const GGUF_VERSION: u32 = 3;

struct TensorInfo {
    name: String,
    dims: Vec<u64>,
    tensor_type: u32,
    offset: u64,
    size: u64,
}

fn read_string<R: Read>(reader: &mut R) -> std::io::Result<String> {
    let mut len_bytes = [0u8; 8];
    reader.read_exact(&mut len_bytes)?;
    let len = u64::from_le_bytes(len_bytes);
    
    let mut buf = vec![0u8; len as usize];
    reader.read_exact(&mut buf)?;
    String::from_utf8(buf).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
}

fn read_metadata<R: Read>(reader: &mut R) -> std::io::Result<()> {
    let mut num_kv_bytes = [0u8; 8];
    reader.read_exact(&mut num_kv_bytes)?;
    let num_kv = u64::from_le_bytes(num_kv_bytes);
    
    for _ in 0..num_kv {
        let _key = read_string(reader)?;
        let mut type_bytes = [0u8; 4];
        reader.read_exact(&mut type_bytes)?;
        let value_type = u32::from_le_bytes(type_bytes);
        
        // Skip value based on type
        match value_type {
            8 => { let _ = read_string(reader)?; } // STRING
            0 | 1 => { let mut buf = [0u8; 1]; reader.read_exact(&mut buf)?; } // UINT8/INT8
            2 | 3 => { let mut buf = [0u8; 2]; reader.read_exact(&mut buf)?; } // UINT16/INT16
            4 | 5 | 6 => { let mut buf = [0u8; 4]; reader.read_exact(&mut buf)?; } // UINT32/INT32/FLOAT32
            7 => { let mut buf = [0u8; 1]; reader.read_exact(&mut buf)?; } // BOOL
            9 => {
                // ARRAY
                let mut array_type_bytes = [0u8; 4];
                reader.read_exact(&mut array_type_bytes)?;
                let mut array_len_bytes = [0u8; 8];
                reader.read_exact(&mut array_len_bytes)?;
                let array_len = u64::from_le_bytes(array_len_bytes);
                
                // Skip array elements
                for _ in 0..array_len {
                    match u32::from_le_bytes(array_type_bytes) {
                        8 => { let _ = read_string(reader)?; }
                        4 | 5 => { let mut buf = [0u8; 4]; reader.read_exact(&mut buf)?; }
                        6 => { let mut buf = [0u8; 4]; reader.read_exact(&mut buf)?; }
                        _ => { let mut buf = [0u8; 4]; reader.read_exact(&mut buf)?; }
                    }
                }
            }
            _ => {
                // Unknown type, try to skip
                let mut buf = [0u8; 4];
                let _ = reader.read_exact(&mut buf);
            }
        }
    }
    
    Ok(())
}

fn read_tensor_info<R: Read>(reader: &mut R) -> std::io::Result<TensorInfo> {
    let name = read_string(reader)?;
    
    let mut n_dims_bytes = [0u8; 4];
    reader.read_exact(&mut n_dims_bytes)?;
    let n_dims = u32::from_le_bytes(n_dims_bytes);
    
    let mut dims = Vec::new();
    for _ in 0..n_dims {
        let mut dim_bytes = [0u8; 8];
        reader.read_exact(&mut dim_bytes)?;
        dims.push(u64::from_le_bytes(dim_bytes));
    }
    
    let mut tensor_type_bytes = [0u8; 4];
    reader.read_exact(&mut tensor_type_bytes)?;
    let tensor_type = u32::from_le_bytes(tensor_type_bytes);
    
    let mut offset_bytes = [0u8; 8];
    reader.read_exact(&mut offset_bytes)?;
    let offset = u64::from_le_bytes(offset_bytes);
    
    // Calculate tensor size
    let element_size = match tensor_type {
        0 => 4,   // F32
        1 => 2,   // F16
        2..=14 => 4, // Quantized types (simplified)
        _ => 4,
    };
    
    let total_elements: u64 = dims.iter().product();
    let size = total_elements * element_size;
    
    Ok(TensorInfo {
        name,
        dims,
        tensor_type,
        offset,
        size,
    })
}

fn split_gguf(input_path: &str, num_shards: usize, output_dir: &str) -> std::io::Result<()> {
    println!("\nGGUF Proper Splitter (Respects Tensor Boundaries)");
    println!("Input: {}", input_path);
    println!("Shards: {}", num_shards);
    println!("Output: {}\n", output_dir);
    
    std::fs::create_dir_all(output_dir)?;
    
    let mut file = File::open(input_path)?;
    
    // Read magic
    let mut magic = [0u8; 4];
    file.read_exact(&mut magic)?;
    if &magic != GGUF_MAGIC {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "Not a valid GGUF file"
        ));
    }
    
    // Read version
    let mut version_bytes = [0u8; 4];
    file.read_exact(&mut version_bytes)?;
    let version = u32::from_le_bytes(version_bytes);
    println!("GGUF version: {}", version);
    
    // Read metadata
    println!("Reading metadata...");
    read_metadata(&mut file)?;
    
    // Read tensor count
    let mut num_tensors_bytes = [0u8; 8];
    file.read_exact(&mut num_tensors_bytes)?;
    let num_tensors = u64::from_le_bytes(num_tensors_bytes);
    println!("Found {} tensors", num_tensors);
    
    // Read all tensor info
    let mut tensors = Vec::new();
    for i in 0..num_tensors {
        let tensor = read_tensor_info(&mut file)?;
        if i < 5 {
            println!("  Tensor {}: {} at offset {} ({} bytes)", 
                i, tensor.name, tensor.offset, tensor.size);
        }
        tensors.push(tensor);
    }
    
    if num_tensors > 5 {
        println!("  ... and {} more tensors", num_tensors - 5);
    }
    
    // Get file size
    let file_size = file.metadata()?.len();
    let data_start = file.stream_position()?;
    
    println!("\nTensor data starts at offset: {}", data_start);
    println!("Total file size: {} bytes ({:.2} GB)", 
        file_size, file_size as f64 / 1_000_000_000.0);
    
    // Split tensors across shards
    let num_shards_u64 = num_shards as u64;
    let tensors_per_shard = num_tensors / num_shards_u64;
    let remainder = (num_tensors % num_shards_u64) as usize;
    
    println!("\nSplitting {} tensors across {} shards...", num_tensors, num_shards);
    println!("Tensors per shard: ~{} (with {} extra tensors)\n", tensors_per_shard, remainder);
    
    // Create shard files
    let mut tensor_idx = 0;
    for shard_num in 0..num_shards {
        let shard_path = format!("{}/shard-{}.gguf", output_dir, shard_num);
        println!("Creating shard {}/{}: {}", shard_num + 1, num_shards, shard_path);
        
        let mut shard_file = File::create(&shard_path)?;
        
        // Write GGUF header
        shard_file.write_all(GGUF_MAGIC)?;
        shard_file.write_all(&version.to_le_bytes())?;
        
        // Write minimal metadata (0 key-value pairs for now)
        shard_file.write_all(&0u64.to_le_bytes())?;
        
        // Calculate how many tensors this shard gets
        let shard_tensor_count = tensors_per_shard + if shard_num < remainder { 1 } else { 0 };
        
        // Write tensor count
        shard_file.write_all(&(shard_tensor_count as u64).to_le_bytes())?;
        
        // Write tensor info and copy tensor data
        let mut shard_data_offset = data_start;
        for _ in 0..shard_tensor_count {
            if tensor_idx >= tensors.len() {
                break;
            }
            
            let tensor = &tensors[tensor_idx];
            
            // Write tensor info
            let name_bytes = tensor.name.as_bytes();
            shard_file.write_all(&(name_bytes.len() as u64).to_le_bytes())?;
            shard_file.write_all(name_bytes)?;
            shard_file.write_all(&(tensor.dims.len() as u32).to_le_bytes())?;
            for dim in &tensor.dims {
                shard_file.write_all(&dim.to_le_bytes())?;
            }
            shard_file.write_all(&tensor.tensor_type.to_le_bytes())?;
            shard_file.write_all(&shard_data_offset.to_le_bytes())?;
            
            // Copy tensor data
            file.seek(SeekFrom::Start(tensor.offset))?;
            let mut tensor_data = vec![0u8; tensor.size as usize];
            file.read_exact(&mut tensor_data)?;
            shard_file.write_all(&tensor_data)?;
            shard_data_offset += tensor.size;
            
            tensor_idx += 1;
        }
        
        let shard_size = shard_file.metadata()?.len();
        println!("  Complete! ({:.2} MB)\n", shard_size as f64 / 1_000_000.0);
    }
    
    println!("Shard splitting complete!");
    Ok(())
}

fn main() {
    let args: Vec<String> = env::args().collect();
    
    if args.len() < 3 {
        eprintln!("Usage: cargo run --example split_gguf_rust -- <input.gguf> <num_shards> [output_dir]");
        eprintln!("Example: cargo run --example split_gguf_rust -- model.gguf 8 models_cache/shards");
        std::process::exit(1);
    }
    
    let input_file = &args[1];
    let num_shards: usize = args[2].parse().expect("Invalid number of shards");
    let output_dir = args.get(3).map(|s| s.as_str()).unwrap_or("models_cache/shards");
    
    if !PathBuf::from(input_file).exists() {
        eprintln!("Error: File not found: {}", input_file);
        std::process::exit(1);
    }
    
    if let Err(e) = split_gguf(input_file, num_shards, output_dir) {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}

