//! Shard Loader - Loads shard files from metadata configuration
//! 
//! This module provides utilities to load shard files based on metadata
//! from E:\rust\llamaModels\shards directory structure.
//! 
//! Supports:
//! - Reading shard metadata from JSON files
//! - Mapping safetensors files to expected shard naming
//! - Creating symbolic links or copies with correct names
//! - Validating shard files exist and match metadata

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::fs;
use std::io;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShardMetadata {
    pub original_model: String,
    pub model_info: ModelInfo,
    pub sharding_plan: Vec<ShardPlan>,
    pub total_shards: u32,
    #[serde(default)]
    pub avg_shard_size_mb: f64,
    #[serde(default)]
    pub total_size_mb: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub n_layers: u32,
    pub n_embd: u32,
    pub n_head: u32,
    pub n_vocab: u32,
    #[serde(default)]
    pub file_size_gb: f64,
    #[serde(default)]
    pub file_size_mb: f64,
    #[serde(default)]
    pub model_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShardPlan {
    pub shard_id: u32,
    pub layer_range: Vec<u32>, // [start, end]
    pub num_layers: u32,
    #[serde(default)]
    pub estimated_size_mb: f64,
    pub includes_embeddings: bool,
    pub includes_output: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafetensorsShardMetadata {
    pub original_model: String,
    pub architecture: Option<String>,
    pub model_info: ModelInfo,
    pub shards: Vec<SafetensorsShard>,
    pub total_shards: u32,
    #[serde(default)]
    pub total_size_mb: f64,
    pub created_at: Option<u64>,
    pub config: Option<ShardConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafetensorsShard {
    pub shard_id: u32,
    pub layer_start: u32,
    pub layer_end: u32,
    pub num_layers: u32,
    pub file_name: String,
    pub file_size: u64,
    #[serde(default)]
    pub checksum: String,
    pub has_embeddings: bool,
    pub has_output: bool,
    pub tensors: Vec<String>,
    pub dtype: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShardConfig {
    pub num_shards: u32,
    pub layers_per_shard: u32,
    pub output_dtype: String,
    pub output_dir: String,
    #[serde(default)]
    pub keep_quantized: bool,
    #[serde(default)]
    pub compute_checksums: bool,
}

/// Shard loader that can work with both metadata formats
pub struct ShardLoader {
    metadata_dir: PathBuf,
    safetensors_dir: Option<PathBuf>,
    target_dir: PathBuf,
}

impl ShardLoader {
    /// Create a new shard loader
    pub fn new(metadata_dir: impl AsRef<Path>, target_dir: impl AsRef<Path>) -> Self {
        Self {
            metadata_dir: metadata_dir.as_ref().to_path_buf(),
            safetensors_dir: None,
            target_dir: target_dir.as_ref().to_path_buf(),
        }
    }

    /// Set the safetensors directory (if different from metadata dir)
    pub fn with_safetensors_dir(mut self, dir: impl AsRef<Path>) -> Self {
        self.safetensors_dir = Some(dir.as_ref().to_path_buf());
        self
    }

    /// Load shard metadata from JSON file
    pub fn load_metadata(&self) -> Result<ShardMetadata, Box<dyn std::error::Error>> {
        let metadata_path = self.metadata_dir.join("shard_metadata.json");
        let content = fs::read_to_string(&metadata_path)?;
        let metadata: ShardMetadata = serde_json::from_str(&content)?;
        Ok(metadata)
    }

    /// Load safetensors metadata
    pub fn load_safetensors_metadata(&self) -> Result<SafetensorsShardMetadata, Box<dyn std::error::Error>> {
        let safetensors_dir = self.safetensors_dir.as_ref().unwrap_or(&self.metadata_dir);
        let metadata_path = safetensors_dir.join("shard_metadata.json");
        let content = fs::read_to_string(&metadata_path)?;
        let metadata: SafetensorsShardMetadata = serde_json::from_str(&content)?;
        Ok(metadata)
    }

    /// Map safetensors files to expected GGUF naming and create links/copies
    /// 
    /// This creates symbolic links (or copies) from safetensors files to the expected
    /// naming convention: shard-0.gguf, shard-1.gguf, etc.
    pub fn map_safetensors_to_gguf_names(&self, use_symlinks: bool) -> Result<Vec<PathBuf>, Box<dyn std::error::Error>> {
        let safetensors_meta = self.load_safetensors_metadata()?;
        let safetensors_dir = self.safetensors_dir.as_ref().unwrap_or(&self.metadata_dir);
        
        // Ensure target directory exists
        fs::create_dir_all(&self.target_dir)?;
        
        let mut mapped_files = Vec::new();
        
        for shard in &safetensors_meta.shards {
            let source_file = safetensors_dir.join(&shard.file_name);
            let target_file = self.target_dir.join(format!("shard-{}.gguf", shard.shard_id));
            
            // Check if source exists and is not empty
            if !source_file.exists() {
                eprintln!("[SHARD_LOADER] ⚠️  Source file not found: {}", source_file.display());
                continue;
            }
            
            let file_size = fs::metadata(&source_file)?.len();
            if file_size == 0 {
                eprintln!("[SHARD_LOADER] ⚠️  Source file is empty: {}", source_file.display());
                continue;
            }
            
            // Remove existing target if it exists
            if target_file.exists() {
                fs::remove_file(&target_file)?;
            }
            
            if use_symlinks {
                // Create symbolic link (Windows requires admin or Developer Mode)
                #[cfg(windows)]
                {
                    use std::os::windows::fs::symlink_file;
                    symlink_file(&source_file, &target_file)
                        .map_err(|e| format!("Failed to create symlink (may need admin/Developer Mode): {}", e))?;
                }
                #[cfg(not(windows))]
                {
                    std::os::unix::fs::symlink(&source_file, &target_file)?;
                }
                println!("[SHARD_LOADER] ✓ Created symlink: {} -> {}", 
                    target_file.display(), source_file.display());
            } else {
                // Copy file (slower but works everywhere)
                fs::copy(&source_file, &target_file)?;
                println!("[SHARD_LOADER] ✓ Copied: {} -> {} ({:.2} MB)", 
                    source_file.display(), target_file.display(), file_size as f64 / 1_048_576.0);
            }
            
            mapped_files.push(target_file);
        }
        
        println!("[SHARD_LOADER] ✓ Mapped {} shard file(s) to target directory", mapped_files.len());
        Ok(mapped_files)
    }

    /// Validate that all required shard files exist
    pub fn validate_shards(&self, expected_shards: u32) -> Result<HashMap<u32, ShardStatus>, Box<dyn std::error::Error>> {
        let mut status = HashMap::new();
        
        for shard_id in 0..expected_shards {
            let shard_file = self.target_dir.join(format!("shard-{}.gguf", shard_id));
            let status_entry = if shard_file.exists() {
                let metadata = fs::metadata(&shard_file)?;
                if metadata.len() > 0 {
                    ShardStatus::Found {
                        path: shard_file,
                        size_mb: metadata.len() as f64 / 1_048_576.0,
                    }
                } else {
                    ShardStatus::Empty { path: shard_file }
                }
            } else {
                ShardStatus::Missing { expected: shard_file }
            };
            status.insert(shard_id, status_entry);
        }
        
        Ok(status)
    }

    /// Get shard configuration for a specific shard ID
    pub fn get_shard_config(&self, shard_id: u32) -> Result<Option<ShardPlan>, Box<dyn std::error::Error>> {
        let metadata = self.load_metadata()?;
        Ok(metadata.sharding_plan.iter()
            .find(|s| s.shard_id == shard_id)
            .cloned())
    }

    /// Print shard status report
    pub fn print_status(&self, expected_shards: u32) -> Result<(), Box<dyn std::error::Error>> {
        let status = self.validate_shards(expected_shards)?;
        
        println!("\n[SHARD_LOADER] ═══════════════════════════════════════════════════════════════════════════");
        println!("[SHARD_LOADER] Shard Status Report");
        println!("[SHARD_LOADER] Target Directory: {}", self.target_dir.display());
        println!("[SHARD_LOADER] ═══════════════════════════════════════════════════════════════════════════\n");
        
        for shard_id in 0..expected_shards {
            match status.get(&shard_id) {
                Some(ShardStatus::Found { path, size_mb }) => {
                    println!("[SHARD_LOADER] ✓ Shard {}: {} ({:.2} MB)", shard_id, path.display(), size_mb);
                }
                Some(ShardStatus::Empty { path }) => {
                    println!("[SHARD_LOADER] ⚠️  Shard {}: {} (EMPTY)", shard_id, path.display());
                }
                Some(ShardStatus::Missing { expected }) => {
                    println!("[SHARD_LOADER] ✗ Shard {}: MISSING (expected: {})", shard_id, expected.display());
                }
                None => {
                    println!("[SHARD_LOADER] ✗ Shard {}: UNKNOWN", shard_id);
                }
            }
        }
        
        let found_count = status.values()
            .filter(|s| matches!(s, ShardStatus::Found { .. }))
            .count();
        
        println!("\n[SHARD_LOADER] Summary: {}/{} shards found", found_count, expected_shards);
        println!("[SHARD_LOADER] ═══════════════════════════════════════════════════════════════════════════\n");
        
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub enum ShardStatus {
    Found { path: PathBuf, size_mb: f64 },
    Empty { path: PathBuf },
    Missing { expected: PathBuf },
}

/// Command-line utility for shard loading
#[cfg(feature = "bin")]
pub fn main() -> Result<(), Box<dyn std::error::Error>> {
    use clap::{Parser, Subcommand};
    
    #[derive(Parser)]
    #[command(name = "shard_loader")]
    #[command(about = "Load and map shard files from metadata")]
    struct Args {
        #[command(subcommand)]
        command: Commands,
    }
    
    #[derive(Subcommand)]
    enum Commands {
        /// Map safetensors files to GGUF naming
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
            
            /// Use symbolic links instead of copying
            #[arg(long)]
            symlink: bool,
        },
        
        /// Validate shard files
        Validate {
            /// Target directory to check
            #[arg(long, default_value = "models_cache/shards")]
            target_dir: String,
            
            /// Expected number of shards
            #[arg(long, default_value = "4")]
            expected_shards: u32,
        },
        
        /// Show shard metadata
        Info {
            /// Metadata directory
            #[arg(long, default_value = "E:\\rust\\llamaModels\\shards")]
            metadata_dir: String,
        },
    }
    
    let args = Args::parse();
    
    match args.command {
        Commands::Map { metadata_dir, safetensors_dir, target_dir, symlink } => {
            let mut loader = ShardLoader::new(&metadata_dir, &target_dir);
            if let Some(safetensors) = safetensors_dir {
                loader = loader.with_safetensors_dir(&safetensors);
            }
            
            println!("[SHARD_LOADER] Mapping safetensors files to GGUF naming...");
            println!("[SHARD_LOADER]   Metadata dir: {}", metadata_dir);
            if let Some(ref sd) = safetensors_dir {
                println!("[SHARD_LOADER]   Safetensors dir: {}", sd);
            }
            println!("[SHARD_LOADER]   Target dir: {}", target_dir);
            println!("[SHARD_LOADER]   Method: {}\n", if symlink { "Symlink" } else { "Copy" });
            
            let mapped = loader.map_safetensors_to_gguf_names(!symlink)?;
            println!("\n[SHARD_LOADER] ✓ Successfully mapped {} file(s)", mapped.len());
        }
        
        Commands::Validate { target_dir, expected_shards } => {
            let loader = ShardLoader::new("", &target_dir);
            loader.print_status(expected_shards)?;
        }
        
        Commands::Info { metadata_dir } => {
            let loader = ShardLoader::new(&metadata_dir, "");
            let metadata = loader.load_metadata()?;
            
            println!("\n[SHARD_LOADER] ═══════════════════════════════════════════════════════════════════════════");
            println!("[SHARD_LOADER] Shard Metadata Information");
            println!("[SHARD_LOADER] ═══════════════════════════════════════════════════════════════════════════\n");
            println!("Original Model: {}", metadata.original_model);
            println!("Model Name: {}", metadata.model_info.model_name);
            println!("Total Layers: {}", metadata.model_info.n_layers);
            println!("Total Shards: {}", metadata.total_shards);
            println!("\nShard Plan:");
            for plan in &metadata.sharding_plan {
                println!("  Shard {}: Layers {}-{} ({} layers, {:.2} MB)", 
                    plan.shard_id,
                    plan.layer_range[0],
                    plan.layer_range[1] - 1,
                    plan.num_layers,
                    plan.estimated_size_mb);
                println!("    Embeddings: {}, Output: {}", 
                    plan.includes_embeddings, plan.includes_output);
            }
            println!("\n[SHARD_LOADER] ═══════════════════════════════════════════════════════════════════════════\n");
        }
    }
    
    Ok(())
}
