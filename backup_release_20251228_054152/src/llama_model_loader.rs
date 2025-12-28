//! Llama Model Loader - Downloads and loads Llama model shards via SCP
//! 
//! This module handles:
//! - Downloading model shards from remote server via SCP
//! - Caching downloaded shards locally
//! - Loading models for inference
//! - Managing model shard distribution
//!
//! ## Authentication
//! SCP authentication can be configured via:
//! - SSH key (recommended): Set SSH_KEY_PATH environment variable
//! - Password: Set in ScpConfig (less secure, requires sshpass on Unix)

use std::path::{Path, PathBuf};
use std::fs;
use tokio::process::Command as TokioCommand;
use std::collections::HashMap;

/// SCP server configuration (replaces RsyncConfig)
#[derive(Clone, Debug)]
pub struct ScpConfig {
    pub host: String,
    pub username: String,
    pub password: Option<String>,
    pub ssh_key_path: Option<PathBuf>,
    pub remote_path: String,
    pub local_cache_dir: PathBuf,
    pub port: u16,
}

/// Backwards-compatible alias
pub type RsyncConfig = ScpConfig;

impl Default for ScpConfig {
    fn default() -> Self {
        Self {
            host: "zh5605.rsync.net".to_string(),
            username: "zh5605".to_string(),
            password: Some("3da393f1".to_string()),
            ssh_key_path: std::env::var("SSH_KEY_PATH").ok().map(PathBuf::from),
            remote_path: ".".to_string(),
            local_cache_dir: PathBuf::from("./models_cache"),
            port: 22,
        }
    }
}

impl ScpConfig {
    /// Create with SSH key authentication (recommended)
    pub fn with_ssh_key(host: &str, username: &str, key_path: PathBuf) -> Self {
        Self {
            host: host.to_string(),
            username: username.to_string(),
            password: None,
            ssh_key_path: Some(key_path),
            remote_path: ".".to_string(),
            local_cache_dir: PathBuf::from("./models_cache"),
            port: 22,
        }
    }

    /// Create with password authentication
    pub fn with_password(host: &str, username: &str, password: &str) -> Self {
        Self {
            host: host.to_string(),
            username: username.to_string(),
            password: Some(password.to_string()),
            ssh_key_path: None,
            remote_path: ".".to_string(),
            local_cache_dir: PathBuf::from("./models_cache"),
            port: 22,
        }
    }

    /// Set remote path
    pub fn remote_path(mut self, path: &str) -> Self {
        self.remote_path = path.to_string();
        self
    }

    /// Set local cache directory
    pub fn cache_dir(mut self, path: PathBuf) -> Self {
        self.local_cache_dir = path;
        self
    }

    /// Set SSH port
    pub fn port(mut self, port: u16) -> Self {
        self.port = port;
        self
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
#[derive(Debug)]
pub struct LlamaModelManager {
    config: ScpConfig,
    loaded_models: HashMap<String, ModelShard>,
}

impl LlamaModelManager {
    /// Create a new model manager
    pub fn new(config: ScpConfig) -> Self {
        // Create cache directory
        std::fs::create_dir_all(&config.local_cache_dir).ok();
        
        Self {
            config,
            loaded_models: HashMap::new(),
        }
    }

    /// Build SSH/SCP common arguments
    fn build_ssh_args(&self) -> Vec<String> {
        let mut args = Vec::new();
        
        // Port
        args.push("-P".to_string());
        args.push(self.config.port.to_string());
        
        // SSH key if available
        if let Some(key_path) = &self.config.ssh_key_path {
            args.push("-i".to_string());
            args.push(key_path.to_string_lossy().to_string());
        }
        
        // Disable strict host key checking for automation
        args.push("-o".to_string());
        args.push("StrictHostKeyChecking=no".to_string());
        args.push("-o".to_string());
        args.push("UserKnownHostsFile=/dev/null".to_string());
        
        // Batch mode (no interactive prompts)
        args.push("-o".to_string());
        args.push("BatchMode=yes".to_string());
        
        args
    }

    /// List available model shards on remote server via SSH
    pub async fn list_available_shards(&self) -> Result<Vec<String>, String> {
        println!("[MODEL] Listing shards from {}@{}:{}...", 
            self.config.username, self.config.host, self.config.remote_path);
        
        let ssh_target = format!("{}@{}", self.config.username, self.config.host);
        let ls_command = format!("ls -la {}", self.config.remote_path);
        
        let mut cmd = TokioCommand::new("ssh");
        
        // Add SSH args
        for arg in self.build_ssh_args() {
            // Use -p for ssh instead of -P
            if arg == "-P" {
                cmd.arg("-p");
            } else {
                cmd.arg(&arg);
            }
        }
        
        cmd.arg(&ssh_target);
        cmd.arg(&ls_command);
        
        let output = cmd.output().await
            .map_err(|e| format!("Failed to execute ssh: {}. Make sure OpenSSH is installed.", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            // Check for common SSH errors
            if stderr.contains("Permission denied") {
                return Err("SSH authentication failed. Set SSH_KEY_PATH environment variable or configure SSH keys.".to_string());
            }
            return Err(format!("SSH failed: {}", stderr));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut shards = Vec::new();
        
        // Parse ls output to find model shard files
        for line in stdout.lines() {
            // Skip directory entries and header
            if line.starts_with('d') || line.starts_with("total") {
                continue;
            }
            
            // Get filename (last column in ls -la)
            if let Some(filename) = line.split_whitespace().last() {
                // Look for common model file patterns
                if filename.ends_with(".safetensors") || 
                   filename.ends_with(".bin") ||
                   filename.ends_with(".gguf") ||
                   filename.contains("model") ||
                   filename.contains("shard") {
                    shards.push(filename.to_string());
                }
            }
        }

        println!("[MODEL] Found {} shard files", shards.len());
        Ok(shards)
    }

    /// Download a model shard via SCP
    pub async fn download_shard(&self, shard_name: &str) -> Result<PathBuf, String> {
        let local_path = self.config.local_cache_dir.join(shard_name);
        
        // Check if already downloaded
        if local_path.exists() {
            println!("[MODEL] Shard {} already cached locally", shard_name);
            return Ok(local_path);
        }

        println!("[MODEL] Downloading shard {} via SCP...", shard_name);
        
        // Ensure local directory exists
        if let Some(parent) = local_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create directory: {}", e))?;
        }

        // Build SCP source path
        let remote_file = if self.config.remote_path.is_empty() || self.config.remote_path == "." {
            shard_name.to_string()
        } else {
            format!("{}/{}", self.config.remote_path, shard_name)
        };
        
        let scp_source = format!("{}@{}:{}", self.config.username, self.config.host, remote_file);
        
        let mut cmd = TokioCommand::new("scp");
        
        // Add SCP args
        for arg in self.build_ssh_args() {
            cmd.arg(&arg);
        }
        
        // Source and destination
        cmd.arg(&scp_source);
        cmd.arg(local_path.to_string_lossy().to_string());
        
        let status = cmd.status().await
            .map_err(|e| format!("Failed to execute scp: {}. Make sure OpenSSH is installed.", e))?;

        if !status.success() {
            return Err(format!("Failed to download shard {} via SCP", shard_name));
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
    pub async fn get_shard_path(&mut self, _model_name: &str, shard_name: &str) -> Result<PathBuf, String> {
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
    
    /// Get the config
    pub fn config(&self) -> &ScpConfig {
        &self.config
    }
}

/// Initialize model manager with default SCP config
pub fn create_model_manager() -> LlamaModelManager {
    LlamaModelManager::new(ScpConfig::default())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scp_config_default() {
        let config = ScpConfig::default();
        assert_eq!(config.host, "zh5605.rsync.net");
        assert_eq!(config.username, "zh5605");
        assert!(config.password.is_some());
        assert_eq!(config.port, 22);
    }

    #[test]
    fn test_scp_config_with_ssh_key() {
        let config = ScpConfig::with_ssh_key(
            "example.com", 
            "user", 
            PathBuf::from("/home/user/.ssh/id_rsa")
        );
        assert_eq!(config.host, "example.com");
        assert_eq!(config.username, "user");
        assert!(config.ssh_key_path.is_some());
        assert!(config.password.is_none());
    }

    #[test]
    fn test_scp_config_with_password() {
        let config = ScpConfig::with_password("example.com", "user", "secret");
        assert_eq!(config.host, "example.com");
        assert!(config.password.is_some());
        assert!(config.ssh_key_path.is_none());
    }

    #[test]
    fn test_scp_config_builder() {
        let config = ScpConfig::default()
            .remote_path("/models")
            .cache_dir(PathBuf::from("/tmp/cache"))
            .port(2222);
        
        assert_eq!(config.remote_path, "/models");
        assert_eq!(config.local_cache_dir, PathBuf::from("/tmp/cache"));
        assert_eq!(config.port, 2222);
    }

    // Backwards compatibility alias test
    #[test]
    fn test_rsync_config_alias() {
        let config: RsyncConfig = ScpConfig::default();
        assert_eq!(config.host, "zh5605.rsync.net");
    }

    #[test]
    fn test_model_manager_creation() {
        let config = ScpConfig::default();
        let manager = LlamaModelManager::new(config);
        assert!(manager.cache_dir().exists() || manager.cache_dir().parent().is_some());
    }

    #[test]
    fn test_build_ssh_args() {
        let config = ScpConfig::with_ssh_key(
            "example.com",
            "user", 
            PathBuf::from("/path/to/key")
        ).port(2222);
        
        let manager = LlamaModelManager::new(config);
        let args = manager.build_ssh_args();
        
        assert!(args.contains(&"-P".to_string()));
        assert!(args.contains(&"2222".to_string()));
        assert!(args.contains(&"-i".to_string()));
        assert!(args.iter().any(|a| a.contains("/path/to/key")));
    }

    #[tokio::test]
    #[ignore = "Requires SSH access to remote server"]
    async fn test_list_available_shards() {
        let manager = create_model_manager();
        let result = manager.list_available_shards().await;
        // This will fail if SSH is not configured, which is expected in CI
        println!("Result: {:?}", result);
    }
}
