//! llama.cpp-backed inference for `EXECUTE_TASK/ai_inference`.
//!
//! This is a **real inference path** (as opposed to the deterministic mock used in CI/tests).
//! It shells out to `llama-cli` (llama.cpp) so we can run GGUF models without adding heavy
//! in-process bindings yet.
//!
//! Enablement is explicit to keep CI stable:
//! - `PUNCH_INFERENCE_BACKEND=llama_cpp`
//! - `LLAMA_CPP_EXE=/path/to/llama-cli(.exe)`
//! - `LLAMA_GGUF_PATH=/path/to/model.gguf`

use serde_json::Value;
use tokio::process::Command;

fn env(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn looks_like_path(s: &str) -> bool {
    s.contains('\\') || s.contains('/') || s.to_lowercase().ends_with(".gguf")
}

fn parse_input_text(input_data: &Value) -> Result<String, String> {
    match input_data {
        Value::String(s) => Ok(s.clone()),
        other => Err(format!(
            "input_data must be a string for llama.cpp backend, got: {other}"
        )),
    }
}

/// Run llama.cpp (`llama-cli`) and return generated text.
pub async fn infer_with_llama_cpp(
    model_name: &str,
    input_data: &Value,
    max_tokens: u32,
    temperature: f64,
    top_p: f64,
) -> Result<String, String> {
    let exe = env("LLAMA_CPP_EXE").ok_or("LLAMA_CPP_EXE not set (path to llama-cli)")?;

    // Prefer explicit GGUF path from env; otherwise allow `model_name` to be a path.
    let gguf_path = env("LLAMA_GGUF_PATH")
        .or_else(|| {
            if looks_like_path(model_name) {
                Some(model_name.to_string())
            } else {
                None
            }
        })
        .ok_or(
            "LLAMA_GGUF_PATH not set (path to model.gguf), and model_name did not look like a path",
        )?;

    let prompt = parse_input_text(input_data)?;

    let threads = env("LLAMA_THREADS")
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or_else(|| {
            std::thread::available_parallelism()
                .map(|n| n.get() as u32)
                .unwrap_or(8)
        });

    // Windows mmap can be finicky; allow forcing --no-mmap.
    let no_mmap = env("LLAMA_NO_MMAP").map(|v| v != "0").unwrap_or(true);

    let mut cmd = Command::new(&exe);
    cmd.arg("-m")
        .arg(&gguf_path)
        .arg("-p")
        .arg(&prompt)
        .arg("-n")
        .arg(max_tokens.to_string())
        .arg("-t")
        .arg(threads.to_string())
        .arg("--temp")
        .arg(format!("{temperature:.3}"))
        .arg("--top-p")
        .arg(format!("{top_p:.3}"));

    if no_mmap {
        cmd.arg("--no-mmap");
    }

    let output = cmd
        .output()
        .await
        .map_err(|e| format!("Failed to execute llama.cpp at '{exe}': {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        return Err(format!(
            "llama.cpp failed (exit={:?}).\nSTDERR:\n{}\nSTDOUT:\n{}",
            output.status.code(),
            stderr,
            stdout
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();

    // llama-cli often prints the prompt + completion. Try to strip the prompt.
    let completion = if stdout.contains(&prompt) {
        stdout
            .split(&prompt)
            .nth(1)
            .unwrap_or(&stdout)
            .trim()
            .to_string()
    } else {
        stdout.trim().to_string()
    };

    if completion.is_empty() {
        return Err("llama.cpp returned empty output".to_string());
    }

    Ok(completion)
}
