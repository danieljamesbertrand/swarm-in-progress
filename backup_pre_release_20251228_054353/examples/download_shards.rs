//! Download safetensors shards from GitHub, rsync.net, or local directory to cache
//! 
//! Run with: cargo run --example download_shards
//! 
//! Options:
//!   - From GitHub: GITHUB_REPO=username/repo cargo run --example download_shards
//!   - From rsync.net: RSYNC_DOWNLOAD=1 cargo run --example download_shards (downloads files)
//!   - List rsync.net: RSYNC_LIST=1 cargo run --example download_shards (lists files)
//!   - From local: Just run without env vars (uses C:\Users\dan\Documents\Mistral)

use std::process::Stdio;
use tokio::process::Command;
use tokio::io::AsyncWriteExt;
use reqwest;

/// Download a file from URL with progress
async fn download_file(client: &reqwest::Client, url: &str, dest_path: &std::path::Path) -> Result<f64, Box<dyn std::error::Error>> {
    let response = client.get(url).send().await?;
    
    if !response.status().is_success() {
        return Err(format!("HTTP {}: {}", response.status(), url).into());
    }
    
    let total_size = response.content_length().unwrap_or(0);
    let mut file = tokio::fs::File::create(dest_path).await?;
    let mut stream = response.bytes_stream();
    let mut downloaded: u64 = 0;
    
    use futures_util::StreamExt;
    while let Some(item) = stream.next().await {
        let chunk = item?;
        file.write_all(&chunk).await?;
        downloaded += chunk.len() as u64;
        
        if total_size > 0 {
            let percent = (downloaded * 100) / total_size;
            print!("\r   Progress: {}% ({:.2} MB / {:.2} MB)", 
                percent,
                downloaded as f64 / (1024.0 * 1024.0),
                total_size as f64 / (1024.0 * 1024.0)
            );
        }
    }
    println!();
    
    Ok(downloaded as f64 / (1024.0 * 1024.0))
}

/// List safetensors files on rsync.net server using SSH
async fn list_rsync_files() -> Result<(), Box<dyn std::error::Error>> {
    let host = "zh5605.rsync.net";
    let user = "zh5605";
    
    // Try multiple find commands as per rsync.net documentation
    let commands = vec![
        "find . -name '*.safetensors' -type f",
        "gfind . -name '*.safetensors' -type f -ls",
        "ls -lh *.safetensors",
        "ls -lh | grep safetensors",
    ];
    
    for cmd in commands {
        println!("Trying: {}", cmd);
        
        let child = Command::new("ssh")
            .args([
                "-o", "StrictHostKeyChecking=no",
                "-o", "ConnectTimeout=10",
                &format!("{}@{}", user, host),
                cmd,
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        
        let output = child.wait_with_output().await?;
        
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if !stdout.trim().is_empty() {
                println!("✅ Found files:\n{}", stdout);
                return Ok(());
            }
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if !stderr.contains("Connection closed") && !stderr.contains("Permission denied") {
                println!("⚠️  Command failed: {}", stderr);
            }
        }
    }
    
    println!("\n❌ Could not list files. SSH may require password authentication.");
    println!("💡 Try setting up SSH keys:");
    println!("   1. ssh-keygen -t ed25519");
    println!("   2. ssh-copy-id {}@{}", user, host);
    println!("\n   Or use the download_shards example to download files directly via SCP.");
    
    Ok(())
}

/// Download safetensors files from rsync.net server using SCP
async fn download_from_rsync(local_cache: &str) -> Result<(), Box<dyn std::error::Error>> {
    let host = "zh5605.rsync.net";
    let user = "zh5605";
    
    println!("🌐 Downloading safetensors files from rsync.net server...\n");
    
    // Try common filename patterns (skip listing to avoid password prompt)
    let files_to_download = vec![
        "model-00001-of-00004.safetensors".to_string(),
        "model-00002-of-00004.safetensors".to_string(),
        "model-00003-of-00004.safetensors".to_string(),
        "model-00004-of-00004.safetensors".to_string(),
        "model-00001-of-00003.safetensors".to_string(),
        "model-00002-of-00003.safetensors".to_string(),
        "model-00003-of-00003.safetensors".to_string(),
        "shard-0.safetensors".to_string(),
        "shard-1.safetensors".to_string(),
        "shard-2.safetensors".to_string(),
        "shard-3.safetensors".to_string(),
    ];
    
    println!("📋 Will try to download {} common shard filename patterns\n", files_to_download.len());
    
    if files_to_download.is_empty() {
        println!("❌ No safetensors files found on server");
        println!("💡 Try listing files first: $env:RSYNC_LIST='1'; cargo run --example download_shards");
        return Ok(());
    }
    
    println!("✅ Found {} file(s) to download\n", files_to_download.len());
    
    // Download each file using SCP
    for (i, filename) in files_to_download.iter().enumerate() {
        let dest_path = std::path::Path::new(local_cache).join(filename);
        
        // Skip if already exists
        if dest_path.exists() {
            let size_mb = std::fs::metadata(&dest_path)?.len() as f64 / (1024.0 * 1024.0);
            println!("⏭️  Skipping {} (already exists, {:.2} MB)", filename, size_mb);
            continue;
        }
        
        println!("📥 Downloading {}/{}...", i + 1, files_to_download.len());
        println!("   File: {}", filename);
        
        // Use SCP to download
        let scp_source = format!("{}@{}:{}", user, host, filename);
        let scp_dest = dest_path.to_string_lossy().to_string();
        
        let scp_cmd = Command::new("scp")
            .args([
                "-o", "StrictHostKeyChecking=no",
                "-o", "ConnectTimeout=30",
                &scp_source,
                &scp_dest,
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        
        let scp_output = scp_cmd.wait_with_output().await?;
        
        if scp_output.status.success() {
            let size_mb = std::fs::metadata(&dest_path)?.len() as f64 / (1024.0 * 1024.0);
            println!("   ✅ Complete! ({:.2} MB)", size_mb);
        } else {
            let stderr = String::from_utf8_lossy(&scp_output.stderr);
            if stderr.contains("No such file") {
                println!("   ⚠️  File not found on server");
            } else if stderr.contains("Permission denied") || stderr.contains("password") {
                println!("   ⚠️  Authentication required. You may need to:");
                println!("      - Set up SSH keys");
                println!("      - Or enter password when prompted");
            } else {
                println!("   ❌ Failed: {}", stderr);
            }
        }
    }
    
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n╔══════════════════════════════════════════════════════════════╗");
    println!("║         📥 SHARD DOWNLOADER WITH PROGRESS 📥                 ║");
    println!("╚══════════════════════════════════════════════════════════════╝\n");

    let local_cache = "models_cache";
    std::fs::create_dir_all(local_cache)?;

    // Check if we should list files on rsync.net
    if std::env::var("RSYNC_LIST").is_ok() {
        println!("🔍 Listing safetensors files on rsync.net server...\n");
        list_rsync_files().await?;
        return Ok(());
    }
    
    // Check if we should download from rsync.net
    if std::env::var("RSYNC_DOWNLOAD").is_ok() {
        download_from_rsync(local_cache).await?;
        
        // Show what we downloaded
        println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        println!("📁 Contents of {}:", local_cache);
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        
        for entry in std::fs::read_dir(local_cache)? {
            let entry = entry?;
            let metadata = entry.metadata()?;
            let size_mb = metadata.len() as f64 / (1024.0 * 1024.0);
            println!("   {:40} {:>10.2} MB", 
                entry.file_name().to_string_lossy(),
                size_mb
            );
        }
        
        return Ok(());
    }

    // Check if GitHub repo is specified
    if let Ok(github_repo) = std::env::var("GITHUB_REPO") {
        let branch = std::env::var("GITHUB_BRANCH").unwrap_or_else(|_| "main".to_string());
        let path = std::env::var("GITHUB_PATH").unwrap_or_else(|_| "".to_string());
        
        println!("🌐 Downloading from GitHub: {}/{}\n", github_repo, branch);
        
        // Common shard file patterns to try
        let shard_patterns = vec![
            "model-00001-of-00004.safetensors",
            "model-00002-of-00004.safetensors",
            "model-00003-of-00004.safetensors",
            "model-00004-of-00004.safetensors",
            "model-00001-of-00003.safetensors",
            "model-00002-of-00003.safetensors",
            "model-00003-of-00003.safetensors",
            "shard-0.safetensors",
            "shard-1.safetensors",
            "shard-2.safetensors",
            "shard-3.safetensors",
        ];
        
        let base_url = if path.is_empty() {
            format!("https://raw.githubusercontent.com/{}/{}/", github_repo, branch)
        } else {
            format!("https://raw.githubusercontent.com/{}/{}/{}/", github_repo, branch, path.trim_start_matches('/'))
        };
        
        let client = reqwest::Client::new();
        let mut downloaded = 0;
        
        for (i, filename) in shard_patterns.iter().enumerate() {
            let url = format!("{}{}", base_url, filename);
            let dest_path = std::path::Path::new(local_cache).join(filename);
            
            // Skip if already exists
            if dest_path.exists() {
                println!("⏭️  Skipping {} (already exists)", filename);
                continue;
            }
            
            println!("📥 Downloading {}/{}...", i + 1, shard_patterns.len());
            println!("   URL: {}", url);
            println!("   File: {}", filename);
            
            match download_file(&client, &url, &dest_path).await {
                Ok(size_mb) => {
                    println!("   ✅ Complete! ({:.2} MB)", size_mb);
                    downloaded += 1;
                }
                Err(e) => {
                    // File might not exist, that's okay
                    if !e.to_string().contains("404") {
                        println!("   ⚠️  Failed: {}", e);
                    }
                }
            }
        }
        
        if downloaded == 0 {
            println!("\n⚠️  No files downloaded. Try specifying GITHUB_PATH if files are in a subdirectory.");
            println!("   Example: $env:GITHUB_PATH='models'; cargo run --example download_shards");
        }
    } else {
        // Fallback to local directory
        let source_dir = r"C:\Users\dan\Documents\Mistral";
        println!("📋 Looking for safetensors files in {}...\n", source_dir);
        
        if !std::path::Path::new(source_dir).exists() {
            println!("❌ Source directory not found: {}", source_dir);
            println!("\n💡 To download from GitHub, set GITHUB_REPO environment variable:");
            println!("   $env:GITHUB_REPO='username/repo'; cargo run --example download_shards");
            return Ok(());
        }

        // Find all .safetensors files in the source directory
        let mut safetensors_files = Vec::new();
        for entry in std::fs::read_dir(source_dir)? {
            let entry = entry?;
            let path = entry.path();
            if let Some(ext) = path.extension() {
                if ext == "safetensors" {
                    if let Some(file_name) = path.file_name() {
                        safetensors_files.push((path.clone(), file_name.to_string_lossy().to_string()));
                    }
                }
            }
        }

        if safetensors_files.is_empty() {
            println!("⚠️ No .safetensors files found in {}", source_dir);
            return Ok(());
        }

        safetensors_files.sort_by(|a, b| a.1.cmp(&b.1));
        println!("✅ Found {} safetensors file(s)\n", safetensors_files.len());

        for (i, (source_path, file_name)) in safetensors_files.iter().enumerate() {
            println!("📥 Copying shard {}/{}...", i + 1, safetensors_files.len());
            println!("   File: {}", file_name);
            
            let dest_path = std::path::Path::new(local_cache).join(file_name);
            let metadata = std::fs::metadata(source_path)?;
            let size_mb = metadata.len() as f64 / (1024.0 * 1024.0);
            
            std::fs::copy(source_path, &dest_path)?;
            println!("   ✅ Complete! ({:.2} MB)", size_mb);
        }
    }

    // Check what we downloaded
    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("📁 Contents of {}:", local_cache);
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    
    for entry in std::fs::read_dir(local_cache)? {
        let entry = entry?;
        let metadata = entry.metadata()?;
        let size_mb = metadata.len() as f64 / (1024.0 * 1024.0);
        println!("   {:40} {:>10.2} MB", 
            entry.file_name().to_string_lossy(),
            size_mb
        );
    }
    
    Ok(())
}

