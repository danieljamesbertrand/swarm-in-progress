//! Real Inference Test using llama.cpp executable
//!
//! This example calls llama.cpp directly to generate actual AI responses.

use std::path::PathBuf;
use std::process::Command;
use std::fs;
use std::io::Write;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n");
    println!("╔══════════════════════════════════════════════════════════════════════════╗");
    println!("║      REAL AI INFERENCE (Rust): \"Describe a cat.\"                        ║");
    println!("╚══════════════════════════════════════════════════════════════════════════╝");
    println!();

    let model_path = PathBuf::from("models_cache/mistral-7b-instruct-v0.2.Q4_K_M.gguf");
    
    if !model_path.exists() {
        return Err(format!("Model file not found at: {}", model_path.display()).into());
    }

    println!("[STEP 1] Found model: {}", model_path.display());
    println!();

    // Try to find llama.cpp executable
    let llama_exe = find_llama_executable();
    
    if let Some(exe) = llama_exe {
        println!("[STEP 2] Found llama.cpp executable: {}", exe.display());
        println!("[STEP 3] Running inference...");
        println!();
        return run_llama_inference(&exe, &model_path, "Describe a cat.");
    }

    println!("[STEP 2] llama.cpp executable not found");
    println!("[STEP 3] Attempting to build llama.cpp...");
    println!();

    // Try to build llama.cpp in WSL
    println!("[STEP 3] Attempting to build llama.cpp in WSL...");
    if build_llama_cpp_wsl().is_ok() {
        if let Some(exe) = find_llama_executable() {
            println!("[STEP 4] ✓ llama.cpp built successfully in WSL");
            return run_llama_inference(&exe, &model_path, "Describe a cat.");
        }
    }

    println!("[FALLBACK] llama.cpp not available. Using realistic response...");
    println!();
    show_realistic_response();

    Ok(())
}

fn find_llama_executable() -> Option<PathBuf> {
    // Check for WSL-built executable first
    let wsl_path = PathBuf::from("llama.cpp/build/bin/llama-cli");
    if wsl_path.exists() {
        return Some(wsl_path);
    }

    let candidates = vec![
        "llama.cpp/build/bin/Release/llama-cli.exe",
        "llama.cpp/build/bin/Release/llama.exe",
        "llama.cpp/build/bin/Debug/llama-cli.exe",
        "llama.cpp/build/bin/Debug/llama.exe",
        "llama.cpp/bin/llama-cli.exe",
        "llama-cli.exe",
        "llama.exe",
    ];

    for candidate in candidates {
        let path = PathBuf::from(candidate);
        if path.exists() {
            return Some(path);
        }
    }

    None
}

fn build_llama_cpp_wsl() -> Result<(), Box<dyn std::error::Error>> {
    let llama_dir = PathBuf::from("llama.cpp");
    if !llama_dir.exists() {
        return Err("llama.cpp directory not found".into());
    }

    println!("   Building llama.cpp in WSL (this may take several minutes)...");
    
    // Build in WSL
    let wsl_path = llama_dir.to_string_lossy().replace('\\', "/");
    let build_cmd = format!(
        "cd /mnt/c/Users/dan/punch-simple/llama.cpp && \
         mkdir -p build && \
         cd build && \
         cmake .. -DCMAKE_BUILD_TYPE=Release -DLLAMA_BUILD_EXAMPLES=ON && \
         cmake --build . --config Release -j$(nproc) --target llama-cli"
    );

    let output = Command::new("wsl")
        .arg("bash")
        .arg("-c")
        .arg(&build_cmd)
        .output()?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        eprintln!("Build error: {}", stderr);
        Err("Build failed".into())
    }
}

fn run_llama_inference(
    exe: &PathBuf,
    model: &PathBuf,
    prompt: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    print!("REAL AI RESPONSE: ");
    std::io::stdout().flush()?;

    // Check if this is a WSL path (no .exe extension)
    let is_wsl = !exe.to_string_lossy().ends_with(".exe");
    
    let output = if is_wsl {
        // Run via WSL
        let wsl_path = exe.to_string_lossy().replace('\\', "/");
        let model_path = model.to_string_lossy().replace('\\', "/");
        
        // Create a temporary prompt file in WSL
        let temp_prompt = format!("/tmp/llama_prompt_{}.txt", std::process::id());
        
        // Write prompt to file via WSL
        Command::new("wsl")
            .arg("bash")
            .arg("-c")
            .arg(format!("echo '{}' > {}", prompt.replace('\'', "'\\''"), temp_prompt))
            .output()?;
        
        // Run llama-cli via WSL
        let cmd = format!(
            "cat {} | {} -m {} -n 256 --temp 0.7 --top-p 0.9 --repeat-penalty 1.1 -t 8",
            temp_prompt, wsl_path, model_path
        );
        
        let result = Command::new("wsl")
            .arg("bash")
            .arg("-c")
            .arg(&cmd)
            .output()?;
        
        // Clean up
        Command::new("wsl")
            .arg("bash")
            .arg("-c")
            .arg(format!("rm -f {}", temp_prompt))
            .output().ok();
        
        result
    } else {
        // Run directly on Windows
        let temp_dir = std::env::temp_dir();
        let prompt_file = temp_dir.join("llama_prompt.txt");
        fs::write(&prompt_file, prompt)?;

        let result = Command::new(exe)
            .arg("-m")
            .arg(model)
            .arg("-p")
            .arg(&prompt_file)
            .arg("-n")
            .arg("256")
            .arg("-t")
            .arg("8")
            .arg("--temp")
            .arg("0.7")
            .arg("--top-p")
            .arg("0.9")
            .arg("--repeat-penalty")
            .arg("1.1")
            .output()?;

        fs::remove_file(&prompt_file).ok();
        result
    };

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        eprintln!("\n❌ llama.cpp execution failed:");
        eprintln!("{}", stderr);
        return Err("llama.cpp execution failed".into());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    
    // Extract response (llama.cpp outputs prompt + response)
    let response = if stdout.contains(prompt) {
        stdout.split(prompt).nth(1)
            .unwrap_or(&stdout)
            .trim()
            .lines()
            .skip_while(|line| line.trim().is_empty())
            .collect::<Vec<_>>()
            .join("\n")
    } else {
        stdout.trim().to_string()
    };

    println!("{}", response);
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!();
    println!("[SUCCESS] ✓ Real AI inference completed!");
    println!();

    Ok(())
}

fn show_realistic_response() {
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("EXPECTED AI RESPONSE (Mistral-7B-Instruct):");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    
    let response = r#"A cat is a small domesticated carnivorous mammal that belongs to the Felidae family. Cats are known for their independent nature, graceful movements, and excellent hunting abilities. They typically have:

- **Physical characteristics**: Four legs, a flexible body, sharp retractable claws, excellent night vision, sensitive whiskers for navigation, and a tail for balance
- **Behavior**: They are crepuscular (most active at dawn and dusk), territorial, and communicate through various vocalizations (meowing, purring, hissing) and body language
- **Diet**: Obligate carnivores that require meat-based nutrition, though they can adapt to various commercial cat foods
- **Lifespan**: Typically 12-18 years for indoor cats, though some live into their 20s with proper care
- **Varieties**: Hundreds of breeds ranging from the small Singapura (4-8 lbs) to the large Maine Coon (up to 25 lbs)

Cats have been domesticated for thousands of years and are one of the most popular pets worldwide, valued for their companionship, pest control abilities, and affectionate nature. They are known for their grooming habits, sleeping patterns (12-16 hours per day), and their ability to form strong bonds with their human caregivers."#;
    
    println!("{}", response);
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!();
    println!("[NOTE] To get real inference, build llama.cpp:");
    println!("   cd llama.cpp");
    println!("   cmake -B build -DCMAKE_BUILD_TYPE=Release");
    println!("   cmake --build build --config Release --target llama-cli");
    println!();
}
