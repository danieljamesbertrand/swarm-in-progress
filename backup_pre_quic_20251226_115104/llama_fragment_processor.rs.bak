//! Llama Fragment-Based Distributed Processing
//! 
//! This module implements a distributed Llama inference system that splits
//! work into fragments and distributes them across nodes in the swarm for
//! parallel processing.

use crate::command_protocol::{Command, CommandResponse, commands};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

/// Fragment of work to be processed by a single node
#[derive(Debug, Clone)]
pub struct LlamaFragment {
    pub fragment_id: String,
    pub job_id: String,
    pub fragment_index: usize,
    pub total_fragments: usize,
    pub input_data: Value,
    pub model_name: String,
    pub parameters: HashMap<String, Value>,
    pub context_window_start: usize,
    pub context_window_end: usize,
}

/// Complete job that will be split into fragments
#[derive(Debug, Clone)]
pub struct LlamaJob {
    pub job_id: String,
    pub model_name: String,
    pub input_data: Value,
    pub parameters: HashMap<String, Value>,
    pub total_fragments: usize,
    pub fragments: Vec<LlamaFragment>,
    pub created_at: u64,
}

/// Result from processing a single fragment
#[derive(Debug, Clone)]
pub struct FragmentResult {
    pub fragment_id: String,
    pub job_id: String,
    pub fragment_index: usize,
    pub output: Value,
    pub tokens_generated: u32,
    pub processing_time_ms: f64,
    pub node_id: String,
}

/// Complete job result after aggregating all fragments
#[derive(Debug, Clone)]
pub struct JobResult {
    pub job_id: String,
    pub combined_output: String,
    pub total_tokens: u32,
    pub total_processing_time_ms: f64,
    pub fragment_results: Vec<FragmentResult>,
    pub completed_at: u64,
}

impl LlamaJob {
    /// Create a new Llama job from a request
    pub fn from_request(request: &Command, num_fragments: usize) -> Result<Self, String> {
        if request.command != commands::EXECUTE_TASK {
            return Err("Command is not EXECUTE_TASK".to_string());
        }

        let task_type = request.params
            .get("task_type")
            .and_then(|v| v.as_str())
            .ok_or("Missing task_type parameter")?;

        if task_type != "ai_inference" && task_type != "llama_inference" {
            return Err(format!("Task type must be ai_inference or llama_inference, got: {}", task_type));
        }

        let model_name = request.params
            .get("model_name")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or("Missing model_name parameter")?;

        let input_data = request.params
            .get("input_data")
            .cloned()
            .ok_or("Missing input_data parameter")?;

        // Extract parameters
        let mut parameters = HashMap::new();
        if let Some(max_tokens) = request.params.get("max_tokens") {
            parameters.insert("max_tokens".to_string(), max_tokens.clone());
        }
        if let Some(temperature) = request.params.get("temperature") {
            parameters.insert("temperature".to_string(), temperature.clone());
        }
        if let Some(top_p) = request.params.get("top_p") {
            parameters.insert("top_p".to_string(), top_p.clone());
        }

        let job_id = format!("job-{}", SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos());

        let mut job = LlamaJob {
            job_id: job_id.clone(),
            model_name,
            input_data,
            parameters,
            total_fragments: num_fragments,
            fragments: Vec::new(),
            created_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        };

        // Split into fragments
        job.split_into_fragments()?;

        Ok(job)
    }

    /// Split the job into fragments for distributed processing
    fn split_into_fragments(&mut self) -> Result<(), String> {
        self.fragments.clear();

        // Clone input_data to avoid borrow issues
        let input_data = self.input_data.clone();

        // Determine how to split based on input type
        match input_data {
            Value::String(text) => {
                // Split text into chunks
                self.split_text_into_fragments(&text)?;
            }
            Value::Array(items) => {
                // Split array items across fragments
                self.split_array_into_fragments(&items)?;
            }
            _ => {
                // For other types, create fragments with full context
                self.create_context_fragments()?;
            }
        }

        Ok(())
    }

    /// Split text input into fragments
    fn split_text_into_fragments(&mut self, text: &str) -> Result<(), String> {
        let chars_per_fragment = (text.len() as f64 / self.total_fragments as f64).ceil() as usize;
        
        for i in 0..self.total_fragments {
            let start = i * chars_per_fragment;
            let end = ((i + 1) * chars_per_fragment).min(text.len());
            
            if start >= text.len() {
                break;
            }

            let fragment_text = &text[start..end];
            let context_start = start.saturating_sub(chars_per_fragment / 2);
            let context_end = (end + chars_per_fragment / 2).min(text.len());

            let fragment = LlamaFragment {
                fragment_id: format!("{}-frag-{}", self.job_id, i),
                job_id: self.job_id.clone(),
                fragment_index: i,
                total_fragments: self.total_fragments,
                input_data: json!(fragment_text),
                model_name: self.model_name.clone(),
                parameters: self.parameters.clone(),
                context_window_start: context_start,
                context_window_end: context_end,
            };

            self.fragments.push(fragment);
        }

        Ok(())
    }

    /// Split array input into fragments
    fn split_array_into_fragments(&mut self, items: &[Value]) -> Result<(), String> {
        let items_per_fragment = (items.len() as f64 / self.total_fragments as f64).ceil() as usize;
        
        for i in 0..self.total_fragments {
            let start = i * items_per_fragment;
            let end = ((i + 1) * items_per_fragment).min(items.len());
            
            if start >= items.len() {
                break;
            }

            let fragment_items: Vec<Value> = items[start..end].to_vec();

            let fragment = LlamaFragment {
                fragment_id: format!("{}-frag-{}", self.job_id, i),
                job_id: self.job_id.clone(),
                fragment_index: i,
                total_fragments: self.total_fragments,
                input_data: json!(fragment_items),
                model_name: self.model_name.clone(),
                parameters: self.parameters.clone(),
                context_window_start: start,
                context_window_end: end,
            };

            self.fragments.push(fragment);
        }

        Ok(())
    }

    /// Create fragments with full context (for non-splittable inputs)
    fn create_context_fragments(&mut self) -> Result<(), String> {
        for i in 0..self.total_fragments {
            let fragment = LlamaFragment {
                fragment_id: format!("{}-frag-{}", self.job_id, i),
                job_id: self.job_id.clone(),
                fragment_index: i,
                total_fragments: self.total_fragments,
                input_data: self.input_data.clone(),
                model_name: self.model_name.clone(),
                parameters: self.parameters.clone(),
                context_window_start: 0,
                context_window_end: 0,
            };

            self.fragments.push(fragment);
        }

        Ok(())
    }

    /// Convert a fragment to a Command for sending to a node
    pub fn fragment_to_command(&self, fragment: &LlamaFragment, target_peer: &str) -> Command {
        let mut params = HashMap::new();
        params.insert("task_type".to_string(), json!("llama_fragment"));
        params.insert("job_id".to_string(), json!(fragment.job_id));
        params.insert("fragment_id".to_string(), json!(fragment.fragment_id));
        params.insert("fragment_index".to_string(), json!(fragment.fragment_index));
        params.insert("total_fragments".to_string(), json!(fragment.total_fragments));
        params.insert("model_name".to_string(), json!(fragment.model_name));
        params.insert("input_data".to_string(), fragment.input_data.clone());
        params.insert("context_window_start".to_string(), json!(fragment.context_window_start));
        params.insert("context_window_end".to_string(), json!(fragment.context_window_end));
        
        // Add model parameters
        for (key, value) in &fragment.parameters {
            params.insert(key.clone(), value.clone());
        }

        Command {
            command: commands::EXECUTE_TASK.to_string(),
            request_id: format!("req-{}-{}", fragment.job_id, fragment.fragment_index),
            from: "coordinator".to_string(),
            to: Some(target_peer.to_string()),
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            params,
        }
    }
}

impl FragmentResult {
    /// Create a fragment result from a response
    pub fn from_response(response: &CommandResponse, fragment_id: &str, node_id: &str) -> Result<Self, String> {
        let result = response.result.as_ref()
            .ok_or("Response missing result")?;

        let job_id = result.get("job_id")
            .and_then(|v| v.as_str())
            .ok_or("Missing job_id in result")?
            .to_string();

        let fragment_index = result.get("fragment_index")
            .and_then(|v| v.as_u64())
            .ok_or("Missing fragment_index in result")? as usize;

        let output = result.get("output")
            .cloned()
            .ok_or("Missing output in result")?;

        let tokens_generated = result.get("tokens_generated")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32;

        let processing_time_ms = result.get("processing_time_ms")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);

        Ok(FragmentResult {
            fragment_id: fragment_id.to_string(),
            job_id,
            fragment_index,
            output,
            tokens_generated,
            processing_time_ms,
            node_id: node_id.to_string(),
        })
    }
}

impl JobResult {
    /// Aggregate fragment results into a complete job result
    pub fn from_fragments(job_id: &str, fragment_results: Vec<FragmentResult>) -> Self {
        // Sort fragments by index
        let mut sorted_results = fragment_results;
        sorted_results.sort_by_key(|r| r.fragment_index);

        // Combine outputs
        let mut combined_parts = Vec::new();
        let mut total_tokens = 0;
        let mut total_time = 0.0;

        for result in &sorted_results {
            if let Some(text) = result.output.as_str() {
                combined_parts.push(text);
            }
            total_tokens += result.tokens_generated;
            total_time += result.processing_time_ms;
        }

        let combined_output = combined_parts.join("");

        JobResult {
            job_id: job_id.to_string(),
            combined_output,
            total_tokens,
            total_processing_time_ms: total_time,
            fragment_results: sorted_results,
            completed_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        }
    }

    /// Convert to CommandResponse
    pub fn to_response(&self, original_request: &Command) -> CommandResponse {
        let mut result = HashMap::new();
        result.insert("output".to_string(), json!(self.combined_output));
        result.insert("total_tokens".to_string(), json!(self.total_tokens));
        result.insert("total_processing_time_ms".to_string(), json!(self.total_processing_time_ms));
        result.insert("fragments_processed".to_string(), json!(self.fragment_results.len()));
        result.insert("job_id".to_string(), json!(self.job_id));

        CommandResponse::success(
            &original_request.command,
            &original_request.request_id,
            &original_request.to.as_ref().unwrap_or(&"coordinator".to_string()),
            &original_request.from,
            result,
        )
    }
}

/// Process a single fragment using actual Llama model
/// Downloads and loads model shards from rsync server if needed
pub async fn process_fragment(fragment: &LlamaFragment) -> Result<FragmentResult, String> {
    use crate::llama_model_loader::{LlamaModelManager, RsyncConfig};
    
    let start_time = std::time::Instant::now();
    
    // Initialize model manager
    let mut model_manager = LlamaModelManager::new(RsyncConfig::default());
    
    // Get model shard path (downloads if needed)
    let shard_name = format!("{}.safetensors", fragment.model_name);
    let shard_path = model_manager.get_shard_path(&fragment.model_name, &shard_name).await
        .map_err(|e| format!("Failed to get model shard: {}", e))?;
    
    println!("[FRAGMENT] Processing fragment {} with model {} from {}", 
        fragment.fragment_index, 
        fragment.model_name,
        shard_path.display()
    );
    
    // Load and process with actual model
    // Note: This is a placeholder - actual model loading would use candle or llama.cpp
    let output = process_fragment_with_model(fragment, &shard_path).await?;

    let processing_time = start_time.elapsed().as_millis() as f64;

    Ok(FragmentResult {
        fragment_id: fragment.fragment_id.clone(),
        job_id: fragment.job_id.clone(),
        fragment_index: fragment.fragment_index,
        output: json!(output),
        tokens_generated: estimate_tokens(&output),
        processing_time_ms: processing_time,
        node_id: "local-node".to_string(),
    })
}

/// Process fragment with actual Llama model
/// This function loads the model and runs inference
async fn process_fragment_with_model(fragment: &LlamaFragment, model_path: &Path) -> Result<String, String> {
    use crate::llama_inference::create_inference_engine;
    
    // Extract input text
    let input_text = match &fragment.input_data {
        Value::String(text) => text.clone(),
        Value::Array(items) => {
            // Convert array to text
            items.iter()
                .map(|v| v.as_str().unwrap_or(""))
                .collect::<Vec<_>>()
                .join(" ")
        }
        _ => return Err("Invalid input data type".to_string()),
    };

    // Get model parameters
    let max_tokens = fragment.parameters
        .get("max_tokens")
        .and_then(|v| v.as_u64())
        .unwrap_or(100) as usize;
    
    let temperature = fragment.parameters
        .get("temperature")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.7);

    // Create and load inference engine
    let engine = create_inference_engine(model_path, &fragment.model_name).await
        .map_err(|e| format!("Failed to load model: {}", e))?;
    
    // Run inference
    let output = engine.infer(&input_text, max_tokens, temperature).await
        .map_err(|e| format!("Inference failed: {}", e))?;

    Ok(output)
}

/// Estimate token count (rough approximation)
fn estimate_tokens(text: &str) -> u32 {
    // Rough estimate: ~4 characters per token for English
    (text.len() as f32 / 4.0).ceil() as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_llama_job_creation() {
        let request = Command::new(commands::EXECUTE_TASK, "peer1", Some("peer2"))
            .with_param("task_type", json!("llama_inference"))
            .with_param("model_name", json!("llama-2-7b"))
            .with_param("input_data", json!("This is a test input for distributed processing"));

        let job = LlamaJob::from_request(&request, 3).unwrap();
        assert_eq!(job.model_name, "llama-2-7b");
        assert_eq!(job.total_fragments, 3);
        assert_eq!(job.fragments.len(), 3);
    }

    #[test]
    fn test_text_fragment_splitting() {
        let request = Command::new(commands::EXECUTE_TASK, "peer1", Some("peer2"))
            .with_param("task_type", json!("llama_inference"))
            .with_param("model_name", json!("llama-2-7b"))
            .with_param("input_data", json!("ABCDEFGHIJKLMNOPQRSTUVWXYZ"));

        let job = LlamaJob::from_request(&request, 3).unwrap();
        assert_eq!(job.fragments.len(), 3);
        
        // Verify fragments are split correctly
        let fragment_texts: Vec<String> = job.fragments.iter()
            .map(|f| f.input_data.as_str().unwrap().to_string())
            .collect();
        
        // All fragments should have some content
        assert!(fragment_texts.iter().all(|t| !t.is_empty()));
    }

    #[test]
    fn test_array_fragment_splitting() {
        let items = json!(["item1", "item2", "item3", "item4", "item5"]);
        let request = Command::new(commands::EXECUTE_TASK, "peer1", Some("peer2"))
            .with_param("task_type", json!("llama_inference"))
            .with_param("model_name", json!("llama-2-7b"))
            .with_param("input_data", items);

        let job = LlamaJob::from_request(&request, 2).unwrap();
        assert_eq!(job.fragments.len(), 2);
    }

    #[test]
    fn test_fragment_to_command() {
        let request = Command::new(commands::EXECUTE_TASK, "peer1", Some("peer2"))
            .with_param("task_type", json!("llama_inference"))
            .with_param("model_name", json!("llama-2-7b"))
            .with_param("input_data", json!("Test input"));

        let job = LlamaJob::from_request(&request, 2).unwrap();
        let fragment = &job.fragments[0];
        let command = job.fragment_to_command(fragment, "target-peer");

        assert_eq!(command.command, commands::EXECUTE_TASK);
        assert_eq!(command.to, Some("target-peer".to_string()));
        assert_eq!(command.params.get("task_type"), Some(&json!("llama_fragment")));
        assert_eq!(command.params.get("fragment_id"), Some(&json!(fragment.fragment_id)));
    }

    #[tokio::test]
    #[ignore = "Requires rsync to be installed"]
    async fn test_process_fragment() {
        let request = Command::new(commands::EXECUTE_TASK, "peer1", Some("peer2"))
            .with_param("task_type", json!("llama_inference"))
            .with_param("model_name", json!("llama-2-7b"))
            .with_param("input_data", json!("Test fragment"));

        let job = LlamaJob::from_request(&request, 1).unwrap();
        let fragment = &job.fragments[0];
        
        let result = process_fragment(fragment).await.unwrap();
        assert_eq!(result.fragment_id, fragment.fragment_id);
        assert_eq!(result.fragment_index, 0);
        assert!(result.tokens_generated > 0);
    }

    #[test]
    fn test_job_result_aggregation() {
        let mut fragment_results = Vec::new();
        
        for i in 0..3 {
            fragment_results.push(FragmentResult {
                fragment_id: format!("frag-{}", i),
                job_id: "job-1".to_string(),
                fragment_index: i,
                output: json!(format!("Part {}", i)),
                tokens_generated: 50,
                processing_time_ms: 100.0,
                node_id: format!("node-{}", i),
            });
        }

        let job_result = JobResult::from_fragments("job-1", fragment_results);
        assert_eq!(job_result.total_tokens, 150);
        assert_eq!(job_result.fragment_results.len(), 3);
        assert!(job_result.combined_output.contains("Part 0"));
        assert!(job_result.combined_output.contains("Part 1"));
        assert!(job_result.combined_output.contains("Part 2"));
    }

    #[test]
    fn test_fragment_result_from_response() {
        let mut result = HashMap::new();
        result.insert("job_id".to_string(), json!("job-1"));
        result.insert("fragment_index".to_string(), json!(0));
        result.insert("output".to_string(), json!("Output text"));
        result.insert("tokens_generated".to_string(), json!(100));
        result.insert("processing_time_ms".to_string(), json!(150.5));

        let response = CommandResponse::success(
            commands::EXECUTE_TASK,
            "req-1",
            "executor",
            "requester",
            result,
        );

        let fragment_result = FragmentResult::from_response(&response, "frag-0", "node-1").unwrap();
        assert_eq!(fragment_result.fragment_index, 0);
        assert_eq!(fragment_result.tokens_generated, 100);
    }
}

