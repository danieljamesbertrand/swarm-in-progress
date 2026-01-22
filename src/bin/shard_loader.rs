//! Shard Loader Binary - Command-line utility for loading and mapping shard files

use clap::{Parser, Subcommand};
use punch_simple::shard_loader::{ShardLoader, ShardStatus};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "shard_loader")]
#[command(about = "Load and map shard files from metadata configuration")]
struct Args {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Map safetensors files to GGUF naming convention
    Map {
        /// Metadata directory (contains shard_metadata.json)
        #[arg(long, default_value = "E:\\rust\\llamaModels\\shards")]
        metadata_dir: String,
        
        /// Safetensors directory (if different from metadata_dir)
        #[arg(long)]
        safetensors_dir: Option<String>,
        
        /// Target directory for mapped files
        #[arg(long, default_value = "models_cache/shards")]
        target_dir: String,
        
        /// Use symbolic links instead of copying (faster, requires admin/Developer Mode on Windows)
        #[arg(long)]
        symlink: bool,
    },
    
    /// Validate shard files exist and are correct
    Validate {
        /// Target directory to check
        #[arg(long, default_value = "models_cache/shards")]
        target_dir: String,
        
        /// Expected number of shards
        #[arg(long, default_value = "4")]
        expected_shards: u32,
    },
    
    /// Show shard metadata information
    Info {
        /// Metadata directory
        #[arg(long, default_value = "E:\\rust\\llamaModels\\shards")]
        metadata_dir: String,
        
        /// Also show safetensors metadata if available
        #[arg(long)]
        safetensors_dir: Option<String>,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    
    match args.command {
        Commands::Map { metadata_dir, safetensors_dir, target_dir, symlink } => {
            let mut loader = ShardLoader::new(&metadata_dir, &target_dir);
            if let Some(ref safetensors) = safetensors_dir {
                loader = loader.with_safetensors_dir(safetensors);
            }
            
            println!("╔══════════════════════════════════════════════════════════════╗");
            println!("║  SHARD LOADER - Mapping Safetensors to GGUF Naming         ║");
            println!("╚══════════════════════════════════════════════════════════════╝\n");
            println!("Metadata directory: {}", metadata_dir);
            if let Some(ref sd) = safetensors_dir {
                println!("Safetensors directory: {}", sd);
            } else {
                println!("Safetensors directory: {}", metadata_dir);
            }
            println!("Target directory: {}", target_dir);
            println!("Method: {}\n", if symlink { "Symbolic Links" } else { "File Copy" });
            
            let mapped = loader.map_safetensors_to_gguf_names(symlink)?;
            println!("\n✅ Successfully mapped {} file(s) to target directory", mapped.len());
            
            // Validate after mapping
            println!("\nValidating mapped files...");
            loader.print_status(4)?;
        }
        
        Commands::Validate { target_dir, expected_shards } => {
            let loader = ShardLoader::new("", &target_dir);
            loader.print_status(expected_shards)?;
        }
        
        Commands::Info { metadata_dir, safetensors_dir } => {
            let loader = ShardLoader::new(&metadata_dir, "");
            
            println!("╔══════════════════════════════════════════════════════════════╗");
            println!("║  SHARD METADATA INFORMATION                                 ║");
            println!("╚══════════════════════════════════════════════════════════════╝\n");
            
            // Try to load standard metadata
            match loader.load_metadata() {
                Ok(metadata) => {
                    println!("[METADATA] Standard Shard Metadata:");
                    println!("  Original Model: {}", metadata.original_model);
                    println!("  Model Name: {}", metadata.model_info.model_name);
                    println!("  Total Layers: {}", metadata.model_info.n_layers);
                    println!("  Embedding Dim: {}", metadata.model_info.n_embd);
                    println!("  Total Shards: {}", metadata.total_shards);
                    println!("\n  Shard Plan:");
                    for plan in &metadata.sharding_plan {
                        println!("    Shard {}: Layers {}-{} ({} layers, {:.2} MB)", 
                            plan.shard_id,
                            plan.layer_range[0],
                            plan.layer_range[1] - 1,
                            plan.num_layers,
                            plan.estimated_size_mb);
                        println!("      Embeddings: {}, Output: {}", 
                            plan.includes_embeddings, plan.includes_output);
                    }
                }
                Err(e) => {
                    eprintln!("[METADATA] Could not load standard metadata: {}", e);
                }
            }
            
            // Try to load safetensors metadata
            if let Some(ref sd) = safetensors_dir {
                let loader_safe = loader.with_safetensors_dir(sd);
                match loader_safe.load_safetensors_metadata() {
                    Ok(safe_meta) => {
                        println!("\n[METADATA] Safetensors Metadata:");
                        println!("  Original Model: {}", safe_meta.original_model);
                        println!("  Architecture: {:?}", safe_meta.architecture);
                        println!("  Total Shards: {}", safe_meta.total_shards);
                        println!("\n  Safetensors Shards:");
                        for shard in &safe_meta.shards {
                            let size_mb = shard.file_size as f64 / 1_048_576.0;
                            println!("    Shard {}: {} (Layers {}-{}, {:.2} MB)", 
                                shard.shard_id,
                                shard.file_name,
                                shard.layer_start,
                                shard.layer_end - 1,
                                size_mb);
                            println!("      Embeddings: {}, Output: {}, Tensors: {}", 
                                shard.has_embeddings, 
                                shard.has_output,
                                shard.tensors.len());
                        }
                    }
                    Err(e) => {
                        eprintln!("[METADATA] Could not load safetensors metadata: {}", e);
                    }
                }
            }
            
            println!("\n╚══════════════════════════════════════════════════════════════╝\n");
        }
    }
    
    Ok(())
}
