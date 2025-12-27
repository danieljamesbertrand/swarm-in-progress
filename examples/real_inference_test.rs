//! Real Inference Test - Uses llama.cpp to get actual AI responses
//!
//! This example attempts to use llama.cpp to generate real text responses.

use std::path::PathBuf;
use std::process::Command;
use std::io::Write;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n");
    println!("╔══════════════════════════════════════════════════════════════════════════╗");
    println!("║         REAL AI INFERENCE TEST: \"Describe a cat.\"                      ║");
    println!("╚══════════════════════════════════════════════════════════════════════════╝");
    println!();

    let model_path = PathBuf::from("models_cache/mistral-7b-instruct-v0.2.Q4_K_M.gguf");
    
    if !model_path.exists() {
        println!("❌ Model file not found at: {}", model_path.display());
        println!("   Attempting to use alternative approach...");
        
        // Try using an external API or fallback
        return try_external_api().await;
    }

    println!("[STEP 1] Found model: {}", model_path.display());
    println!("[STEP 2] Attempting to use llama.cpp for inference...");
    println!();

    // Try to find llama.cpp executable
    let llama_exe = find_llama_executable();
    
    if let Some(exe) = llama_exe {
        println!("[STEP 3] Found llama.cpp executable: {}", exe.display());
        return run_llama_inference(&exe, &model_path, "Describe a cat.").await;
    }

    println!("[STEP 3] llama.cpp executable not found");
    println!("[STEP 4] Trying alternative methods...");
    println!();

    // Try Python with llama-cpp-python if available
    if try_python_llama(&model_path, "Describe a cat.").await.is_ok() {
        return Ok(());
    }

    // Fallback: Use a realistic mock response based on what the model would generate
    println!("[FALLBACK] Generating realistic response based on model behavior...");
    generate_realistic_response("Describe a cat.").await;

    Ok(())
}

fn find_llama_executable() -> Option<PathBuf> {
    // Check common locations
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

async fn run_llama_inference(exe: &PathBuf, model: &PathBuf, prompt: &str) -> Result<(), Box<dyn std::error::Error>> {
    println!("[INFERENCE] Running llama.cpp...");
    println!("   Model: {}", model.display());
    println!("   Prompt: {}", prompt);
    println!();

    // Create a temporary prompt file
    let temp_prompt = std::env::temp_dir().join("llama_prompt.txt");
    std::fs::write(&temp_prompt, prompt)?;

    let output = Command::new(exe)
        .arg("-m")
        .arg(model)
        .arg("-p")
        .arg(&temp_prompt)
        .arg("-n")
        .arg("256")  // max tokens
        .arg("-t")
        .arg("8")    // threads
        .arg("--temp")
        .arg("0.7")
        .arg("--top-p")
        .arg("0.9")
        .output()
        .map_err(|e| format!("Failed to run llama.cpp: {}. Make sure llama.cpp is built.", e))?;

    std::fs::remove_file(&temp_prompt).ok();

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        println!("❌ llama.cpp execution failed:");
        println!("{}", stderr);
        return Err("llama.cpp execution failed".into());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("REAL AI RESPONSE:");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    
    // Extract the response (llama.cpp outputs the prompt + response)
    let response = if stdout.contains(prompt) {
        stdout.split(prompt).nth(1).unwrap_or(&stdout).trim()
    } else {
        stdout.trim()
    };
    
    println!("{}", response);
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!();

    Ok(())
}

async fn try_python_llama(model: &PathBuf, prompt: &str) -> Result<(), Box<dyn std::error::Error>> {
    println!("[ATTEMPT] Trying Python with llama-cpp-python...");
    
    let python_script = format!(r#"
import sys
try:
    from llama_cpp import Llama
    llm = Llama(model_path=r"{}", n_ctx=2048, n_threads=8, verbose=False)
    response = llm(prompt="{}", max_tokens=256, temperature=0.7, top_p=0.9, echo=False)
    print(response['choices'][0]['text'].strip())
    sys.exit(0)
except ImportError:
    print("llama-cpp-python not installed", file=sys.stderr)
    sys.exit(1)
except Exception as e:
    print(f"Error: {{e}}", file=sys.stderr)
    sys.exit(1)
"#, model.display(), prompt.replace('"', r#"\""#));

    let temp_script = std::env::temp_dir().join("llama_inference.py");
    std::fs::write(&temp_script, python_script)?;

    let output = Command::new("python")
        .arg(&temp_script)
        .output();

    std::fs::remove_file(&temp_script).ok();

    match output {
        Ok(output) if output.status.success() => {
            let response = String::from_utf8_lossy(&output.stdout);
            println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
            println!("REAL AI RESPONSE (via Python):");
            println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
            println!("{}", response.trim());
            println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
            Ok(())
        }
        _ => Err("Python llama-cpp-python not available".into()),
    }
}

async fn try_external_api() -> Result<(), Box<dyn std::error::Error>> {
    println!("[ATTEMPT] Trying external API approach...");
    println!("   Note: This would require API keys and internet connection");
    println!("   Skipping for now - using realistic mock response");
    println!();
    
    generate_realistic_response("Describe a cat.").await;
    Ok(())
}

async fn generate_realistic_response(prompt: &str) {
    println!("[INFO] Since real model inference is not available, here's what");
    println!("       a Mistral-7B-Instruct model would typically generate:");
    println!();
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("EXPECTED AI RESPONSE (Mistral-7B-Instruct):");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    
    // This is a realistic response that Mistral-7B-Instruct would generate
    let realistic_response = r#"A cat is a small domesticated carnivorous mammal that belongs to the Felidae family. Cats are known for their independent nature, graceful movements, and excellent hunting abilities. They typically have:

- **Physical characteristics**: Four legs, a flexible body, sharp retractable claws, excellent night vision, sensitive whiskers for navigation, and a tail for balance
- **Behavior**: They are crepuscular (most active at dawn and dusk), territorial, and communicate through various vocalizations (meowing, purring, hissing) and body language
- **Diet**: Obligate carnivores that require meat-based nutrition, though they can adapt to various commercial cat foods
- **Lifespan**: Typically 12-18 years for indoor cats, though some live into their 20s with proper care
- **Varieties**: Hundreds of breeds ranging from the small Singapura (4-8 lbs) to the large Maine Coon (up to 25 lbs)

Cats have been domesticated for thousands of years and are one of the most popular pets worldwide, valued for their companionship, pest control abilities, and affectionate nature. They are known for their grooming habits, sleeping patterns (12-16 hours per day), and their ability to form strong bonds with their human caregivers."#;
    
    println!("{}", realistic_response);
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!();
    println!("[NOTE] To get real inference:");
    println!("   1. Build llama.cpp: cd llama.cpp && cmake -B build && cmake --build build");
    println!("   2. Or install llama-cpp-python: pip install llama-cpp-python");
    println!("   3. Then run this example again");
    println!();
}




