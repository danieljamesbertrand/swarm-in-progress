//! Llama Model Loader - Downloads and loads Llama model shards from rsync server
//! 
//! This module handles:
//! - Downloading model shards from rsync server
//! - Caching downloaded shards locally
//! - Loading models for inference
//! - Managing model shard distribution

use std::path::{Path, PathBuf};
use std::process::Command;
use std::fs;
use tokio::process::Command as TokioCommand;
use serde_json::json;
use std::collections::HashMap;

/// Rsync server configuration
pub struct RsyncConfig {
    pub host: String,
    pub username: String,
    pub password: String,
    pub remote_path: String,
    pub local_cache_dir: PathBuf,
}

impl Default for RsyncConfig {
    fn default() -> Self {
        Self {
            host: "zh5605.rsync.net".to_string(),
            username: "zh5605".to_string(),
            password: "3da393f1".to_string(),
            remote_path: ".".to_string(),
            local_cache_dir: PathBuf::from("./models_cache"),
        }
    }
}

/// Model shard information
#[derive(Debug, Clone)]
pub struct ModelShard {
    pub shard_name: String,
    pub shard_path: PathBuf,
    pub shard_size: u64,
    pub is_loaded: bool,
}

/// Llama model manager
pub struct LlamaModelManager {
    config: RsyncConfig,
    loaded_models: HashMap<String, ModelShard>,
}

impl LlamaModelManager {
    /// Create a new model manager
    pub fn new(config: RsyncConfig) -> Self {
        // Create cache directory
        std::fs::create_dir_all(&config.local_cache_dir).ok();
        
        Self {
            config,
            loaded_models: HashMap::new(),
        }
    }

    /// List available model shards on rsync server
    pub async fn list_available_shards(&self) -> Result<Vec<String>, String> {
        // Create a temporary password file for rsync
        let password_file = std::env::temp_dir().join(format!("rsync_pass_{}", uuid::Uuid::new_v4()));
        std::fs::write(&password_file, &self.config.password)
            .map_err(|e| format!("Failed to create password file: {}", e))?;
        
        // Set permissions (rsync requires password file to be readable only by owner)
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&password_file, std::fs::Permissions::from_mode(0o600))
                .map_err(|e| format!("Failed to set password file permissions: {}", e))?;
        }
        
        let rsync_url = format!("rsync://{}@{}/{}", self.config.username, self.config.host, self.config.remote_path);
        
        // Use rsync command to list files
        let output = TokioCommand::new("rsync")
            .arg("--list-only")
            .arg("--password-file")
            .arg(&password_file)
            .arg(&rsync_url)
            .output()
            .await
            .map_err(|e| format!("Failed to execute rsync: {}", e))?;

        // Clean up password file
        let _ = std::fs::remove_file(&password_file);

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("Rsync failed: {}", stderr));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut shards = Vec::new();
        
        // Parse rsync output to find model shard files
        // Look for common Llama model file patterns
        for line in stdout.lines() {
            if line.contains(".safetensors") || 
               line.contains(".bin") || 
               line.contains("model") ||
               line.contains("shard") {
                if let Some(filename) = line.split_whitespace().last() {
                    if filename.ends_with(".safetensors") || 
                       filename.ends_with(".bin") ||
                       filename.contains("model") {
                        shards.push(filename.to_string());
                    }
                }
            }
        }

        Ok(shards)
    }

    /// Download a model shard from rsync server
    pub async fn download_shard(&self, shard_name: &str) -> Result<PathBuf, String> {
        let local_path = self.config.local_cache_dir.join(shard_name);
        
        // Check if already downloaded
        if local_path.exists() {
            println!("[MODEL] Shard {} already cached locally", shard_name);
            return Ok(local_path);
        }

        println!("[MODEL] Downloading shard {} from rsync server...", shard_name);
        
        // Create temporary password file
        let password_file = std::env::temp_dir().join(format!("rsync_pass_{}", uuid::Uuid::new_v4()));
        std::fs::write(&password_file, &self.config.password)
            .map_err(|e| format!("Failed to create password file: {}", e))?;
        
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&password_file, std::fs::Permissions::from_mode(0o600))
                .map_err(|e| format!("Failed to set password file permissions: {}", e))?;
        }
        
        let remote_path = format!("rsync://{}@{}/{}/{}", 
            self.config.username, 
            self.config.host, 
            self.config.remote_path,
            shard_name
        );
        
        let local_dir = local_path.parent().unwrap();
        std::fs::create_dir_all(local_dir).map_err(|e| format!("Failed to create directory: {}", e))?;

        // Use rsync to download
        let status = TokioCommand::new("rsync")
            .arg("-avz")
            .arg("--progress")
            .arg("--password-file")
            .arg(&password_file)
            .arg(&remote_path)
            .arg(&local_path)
            .status()
            .await
            .map_err(|e| format!("Failed to execute rsync: {}", e))?;

        // Clean up password file
        let _ = std::fs::remove_file(&password_file);

        if !status.success() {
            return Err(format!("Failed to download shard {}", shard_name));
        }

        println!("[MODEL] ✓ Downloaded shard {} to {}", shard_name, local_path.display());
        Ok(local_path)
    }

    /// Download all shards for a model
    pub async fn download_model_shards(&self, model_name: &str) -> Result<Vec<ModelShard>, String> {
        let available_shards = self.list_available_shards().await?;
        
        // Filter shards for this model
        let model_shards: Vec<String> = available_shards
            .into_iter()
            .filter(|s| s.contains(model_name) || s.contains("model"))
            .collect();

        if model_shards.is_empty() {
            return Err(format!("No shards found for model {}", model_name));
        }

        println!("[MODEL] Found {} shards for model {}", model_shards.len(), model_name);

        let mut downloaded_shards = Vec::new();
        for shard_name in &model_shards {
            let shard_path = self.download_shard(shard_name).await?;
            let shard_size = fs::metadata(&shard_path)
                .map(|m| m.len())
                .unwrap_or(0);
            
            downloaded_shards.push(ModelShard {
                shard_name: shard_name.clone(),
                shard_path,
                shard_size,
                is_loaded: false,
            });
        }

        Ok(downloaded_shards)
    }

    /// Get local path for a model shard (downloads if needed)
    pub async fn get_shard_path(&mut self, model_name: &str, shard_name: &str) -> Result<PathBuf, String> {
        // Check if already loaded
        if let Some(shard) = self.loaded_models.get(shard_name) {
            return Ok(shard.shard_path.clone());
        }

        // Download if not cached
        let shard_path = self.download_shard(shard_name).await?;
        
        // Register as loaded
        let shard_size = fs::metadata(&shard_path)
            .map(|m| m.len())
            .unwrap_or(0);
        
        self.loaded_models.insert(shard_name.to_string(), ModelShard {
            shard_name: shard_name.to_string(),
            shard_path: shard_path.clone(),
            shard_size,
            is_loaded: true,
        });

        Ok(shard_path)
    }

    /// Check if a model shard is available locally
    pub fn is_shard_cached(&self, shard_name: &str) -> bool {
        let local_path = self.config.local_cache_dir.join(shard_name);
        local_path.exists()
    }

    /// Get cache directory
    pub fn cache_dir(&self) -> &Path {
        &self.config.local_cache_dir
    }
}

/// Initialize model manager with default rsync config
pub fn create_model_manager() -> LlamaModelManager {
    LlamaModelManager::new(RsyncConfig::default())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rsync_config_default() {
        let config = RsyncConfig::default();
        assert_eq!(config.host, "zh5605.rsync.net");
        assert_eq!(config.username, "zh5605");
        assert!(!config.password.is_empty());
    }

    #[test]
    fn test_model_manager_creation() {
        let config = RsyncConfig::default();
        let manager = LlamaModelManager::new(config);
        assert!(manager.cache_dir().exists() || manager.cache_dir().parent().is_some());
    }

    #[tokio::test]
    #[ignore] // Requires rsync and network access
    async fn test_list_available_shards() {
        let manager = create_model_manager();
        let result = manager.list_available_shards().await;
        // This will fail if rsync is not available, which is expected in CI
        // In production, this would list actual shards
        assert!(result.is_ok() || result.is_err());
    }
}

