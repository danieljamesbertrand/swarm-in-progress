//! Upload Shard Files to Remote Server
//! 
//! Uploads shard files from local directory to eagleoneonline.ca server
//! and configures the rendezvous server to seed them.

use clap::Parser;
use std::path::PathBuf;
use std::process::Command;

#[derive(Parser)]
#[command(name = "upload_shards")]
#[command(about = "Upload shard files to remote rendezvous server")]
struct Args {
    /// Local directory containing shard files
    #[arg(long, default_value = "models_cache/shards")]
    source_dir: String,
    
    /// Remote server user
    #[arg(long, default_value = "dbertrand")]
    remote_user: String,
    
    /// Remote server host
    #[arg(long, default_value = "eagleoneonline.ca")]
    remote_host: String,
    
    /// Remote directory on server (where server will seed from)
    #[arg(long, default_value = "/home/dbertrand/punch-simple/shards")]
    remote_dir: String,
    
    /// Use SCP to upload files
    #[arg(long)]
    use_scp: bool,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    
    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║  UPLOAD SHARDS TO RENDEZVOUS SERVER                          ║");
    println!("╚══════════════════════════════════════════════════════════════╝\n");
    
    println!("Source directory: {}", args.source_dir);
    println!("Remote server: {}@{}", args.remote_user, args.remote_host);
    println!("Remote directory: {}\n", args.remote_dir);
    
    // Check source directory
    let source_path = PathBuf::from(&args.source_dir);
    if !source_path.exists() {
        return Err(format!("Source directory does not exist: {}", args.source_dir).into());
    }
    
    // Find shard files
    let shard_files: Vec<PathBuf> = std::fs::read_dir(&source_path)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.is_file() && 
            p.file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.starts_with("shard-") && n.ends_with(".gguf"))
                .unwrap_or(false)
        })
        .collect();
    
    if shard_files.is_empty() {
        return Err(format!("No shard files found in {}", args.source_dir).into());
    }
    
    println!("Found {} shard file(s):", shard_files.len());
    for file in &shard_files {
        let size_mb = std::fs::metadata(file)?.len() as f64 / 1_048_576.0;
        println!("  - {} ({:.2} MB)", file.file_name().unwrap().to_string_lossy(), size_mb);
    }
    println!();
    
    // Create remote directory
    println!("[1/3] Creating remote directory...");
    let ssh_create_cmd = format!(
        "ssh -F NUL {}@{} 'mkdir -p {}'",
        args.remote_user, args.remote_host, args.remote_dir
    );
    let output = Command::new("powershell")
        .arg("-Command")
        .arg(ssh_create_cmd)
        .output()?;
    
    if !output.status.success() {
        eprintln!("Warning: Failed to create remote directory (may already exist)");
    } else {
        println!("✓ Remote directory created");
    }
    
    // Upload files
    println!("\n[2/3] Uploading shard files...");
    let mut uploaded = 0;
    let mut failed = 0;
    
    for (idx, file) in shard_files.iter().enumerate() {
        let filename = file.file_name().unwrap().to_string_lossy();
        println!("  [{}/{}] Uploading {}...", idx + 1, shard_files.len(), filename);
        
        let remote_path = format!("{}/{}", args.remote_dir, filename);
        let scp_cmd = format!(
            "scp -F NUL {} {}@{}:{}",
            file.to_string_lossy(),
            args.remote_user,
            args.remote_host,
            remote_path
        );
        
        let output = Command::new("powershell")
            .arg("-Command")
            .arg(scp_cmd)
            .output()?;
        
        if output.status.success() {
            println!("    ✓ Uploaded successfully");
            uploaded += 1;
        } else {
            let error = String::from_utf8_lossy(&output.stderr);
            eprintln!("    ✗ Upload failed: {}", error);
            failed += 1;
        }
    }
    
    println!("\n[3/3] Verifying uploads...");
    let verify_cmd = format!(
        "ssh -F NUL {}@{} 'ls -lh {}/*.gguf 2>/dev/null | wc -l'",
        args.remote_user, args.remote_host, args.remote_dir
    );
    
    let output = Command::new("powershell")
        .arg("-Command")
        .arg(verify_cmd)
        .output()?;
    
    let count_str = String::from_utf8_lossy(&output.stdout);
    let remote_count: usize = count_str.trim().parse().unwrap_or(0);
    
    println!("\n╔══════════════════════════════════════════════════════════════╗");
    println!("║  UPLOAD SUMMARY                                                ║");
    println!("╚══════════════════════════════════════════════════════════════╝\n");
    println!("Uploaded: {}/{} files", uploaded, shard_files.len());
    if failed > 0 {
        println!("Failed: {} files", failed);
    }
    println!("Remote server has: {} .gguf file(s)", remote_count);
    println!("\nNext steps:");
    println!("  1. Restart the rendezvous server with --seed-dir {}", args.remote_dir);
    println!("  2. Shard nodes will download missing shards via torrent");
    println!("  3. Once 4 nodes have all shards, distributed inference can begin");
    
    Ok(())
}
