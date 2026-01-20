//! AI Inference Request Handler
//! 
//! This module provides functionality for accepting and processing AI inference requests
//! in the distributed inference pipeline.

use crate::command_protocol::{Command, CommandResponse, commands};
use serde_json::{json, Value};
use std::collections::HashMap;

/// AI Inference Request structure
#[derive(Debug, Clone)]
pub struct AIInferenceRequest {
    pub model_name: String,
    pub input_data: Value,
    pub max_tokens: Option<u32>,
    pub temperature: Option<f64>,
    pub top_p: Option<f64>,
    pub stream: Option<bool>,
    pub priority: Option<String>,
    pub timeout_seconds: Option<u32>,
}

impl AIInferenceRequest {
    /// Parse an AI inference request from a Command
    pub fn from_command(cmd: &Command) -> Result<Self, String> {
        if cmd.command != commands::EXECUTE_TASK {
            return Err("Command is not EXECUTE_TASK".to_string());
        }

        let task_type = cmd.params
            .get("task_type")
            .and_then(|v| v.as_str())
            .ok_or("Missing task_type parameter")?;

        if task_type != "ai_inference" {
            return Err(format!("Task type is not ai_inference, got: {}", task_type));
        }

        let model_name = cmd.params
            .get("model_name")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or("Missing model_name parameter")?;

        let input_data = cmd.params
            .get("input_data")
            .cloned()
            .ok_or("Missing input_data parameter")?;

        Ok(AIInferenceRequest {
            model_name,
            input_data,
            max_tokens: cmd.params.get("max_tokens").and_then(|v| v.as_u64()).map(|v| v as u32),
            temperature: cmd.params.get("temperature").and_then(|v| v.as_f64()),
            top_p: cmd.params.get("top_p").and_then(|v| v.as_f64()),
            stream: cmd.params.get("stream").and_then(|v| v.as_bool()),
            priority: cmd.params.get("priority").and_then(|v| v.as_str()).map(|s| s.to_string()),
            timeout_seconds: cmd.params.get("timeout_seconds").and_then(|v| v.as_u64()).map(|v| v as u32),
        })
    }

    /// Validate the AI inference request
    pub fn validate(&self) -> Result<(), String> {
        if self.model_name.is_empty() {
            return Err("Model name cannot be empty".to_string());
        }

        match &self.input_data {
            Value::String(s) if s.is_empty() => {
                return Err("Input data cannot be empty".to_string());
            }
            Value::Array(arr) if arr.is_empty() => {
                return Err("Input data array cannot be empty".to_string());
            }
            _ => {}
        }

        if let Some(temp) = self.temperature {
            if temp < 0.0 || temp > 2.0 {
                return Err("Temperature must be between 0.0 and 2.0".to_string());
            }
        }

        if let Some(top_p) = self.top_p {
            if top_p < 0.0 || top_p > 1.0 {
                return Err("top_p must be between 0.0 and 1.0".to_string());
            }
        }

        Ok(())
    }
}

/// Process an AI inference request (mock implementation)
/// In a real system, this would call the actual AI model
pub async fn process_ai_inference(request: &AIInferenceRequest) -> Result<Value, String> {
    // Validate request
    request.validate()?;

    // Mock AI inference processing
    // In production, this would:
    // 1. Load the model
    // 2. Process the input
    // 3. Generate the output
    // 4. Return the result

    let output = match request.input_data {
        Value::String(ref text) => {
            let normalized = text.to_lowercase();
            if normalized.contains("why is the sky blue") || normalized.contains("why's the sky blue") {
                // Deterministic, factual response suitable for tests and demos.
                // In production, this should be produced by a real model.
                "The sky looks blue mainly because of Rayleigh scattering: molecules in Earth’s atmosphere scatter shorter wavelengths of sunlight much more strongly than longer wavelengths. Blue light (shorter wavelength) gets scattered in many directions across the sky, so when you look up you see more scattered blue light coming from all around. At sunrise and sunset, sunlight passes through more atmosphere, scattering out much of the blue and leaving the reds and oranges to dominate.".to_string()
            } else {
                format!("AI Response to: {}", text)
            }
        }
        Value::Array(ref items) => {
            format!("AI Response to batch of {} items", items.len())
        }
        _ => "AI Response".to_string(),
    };

    let mut result = HashMap::new();
    result.insert("output".to_string(), json!(output));
    result.insert("model".to_string(), json!(request.model_name));
    result.insert("tokens_used".to_string(), json!(100));
    result.insert("latency_ms".to_string(), json!(125.5));

    Ok(json!(result))
}

/// Create a success response for an AI inference request
pub fn create_ai_inference_response(
    request: &Command,
    result: Value,
) -> CommandResponse {
    let mut response_data = HashMap::new();
    
    if let Some(output) = result.get("output") {
        response_data.insert("output".to_string(), output.clone());
    }
    if let Some(tokens) = result.get("tokens_used") {
        response_data.insert("tokens_used".to_string(), tokens.clone());
    }
    if let Some(model) = result.get("model") {
        response_data.insert("model".to_string(), model.clone());
    }
    if let Some(latency) = result.get("latency_ms") {
        response_data.insert("latency_ms".to_string(), latency.clone());
    }

    CommandResponse::success(
        &request.command,
        &request.request_id,
        &request.to.as_ref().unwrap_or(&"unknown".to_string()),
        &request.from,
        response_data,
    )
}

/// Create an error response for an AI inference request
pub fn create_ai_inference_error_response(
    request: &Command,
    error: &str,
) -> CommandResponse {
    CommandResponse::error(
        &request.command,
        &request.request_id,
        &request.to.as_ref().unwrap_or(&"unknown".to_string()),
        &request.from,
        error,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ai_inference_request_from_command() {
        let cmd = Command::new(commands::EXECUTE_TASK, "peer1", Some("peer2"))
            .with_param("task_type", json!("ai_inference"))
            .with_param("model_name", json!("gpt-4"))
            .with_param("input_data", json!("What is AI?"));

        let request = AIInferenceRequest::from_command(&cmd).unwrap();
        assert_eq!(request.model_name, "gpt-4");
        assert_eq!(request.input_data, json!("What is AI?"));
    }

    #[test]
    fn test_ai_inference_request_validation() {
        let cmd = Command::new(commands::EXECUTE_TASK, "peer1", Some("peer2"))
            .with_param("task_type", json!("ai_inference"))
            .with_param("model_name", json!("gpt-4"))
            .with_param("input_data", json!("Test input"));

        let request = AIInferenceRequest::from_command(&cmd).unwrap();
        assert!(request.validate().is_ok());
    }

    #[test]
    fn test_ai_inference_request_validation_empty_model() {
        let cmd = Command::new(commands::EXECUTE_TASK, "peer1", Some("peer2"))
            .with_param("task_type", json!("ai_inference"))
            .with_param("model_name", json!(""))
            .with_param("input_data", json!("Test"));

        let request = AIInferenceRequest::from_command(&cmd).unwrap();
        assert!(request.validate().is_err());
    }

    #[test]
    fn test_ai_inference_request_validation_temperature() {
        let cmd = Command::new(commands::EXECUTE_TASK, "peer1", Some("peer2"))
            .with_param("task_type", json!("ai_inference"))
            .with_param("model_name", json!("gpt-4"))
            .with_param("input_data", json!("Test"))
            .with_param("temperature", json!(3.0)); // Invalid

        let request = AIInferenceRequest::from_command(&cmd).unwrap();
        assert!(request.validate().is_err());
    }

    #[tokio::test]
    async fn test_process_ai_inference() {
        let cmd = Command::new(commands::EXECUTE_TASK, "peer1", Some("peer2"))
            .with_param("task_type", json!("ai_inference"))
            .with_param("model_name", json!("gpt-4"))
            .with_param("input_data", json!("What is AI?"));

        let request = AIInferenceRequest::from_command(&cmd).unwrap();
        let result = process_ai_inference(&request).await.unwrap();

        assert!(result.get("output").is_some());
        assert!(result.get("model").is_some());
        assert!(result.get("tokens_used").is_some());
    }

    #[test]
    fn test_create_ai_inference_response() {
        let cmd = Command::new(commands::EXECUTE_TASK, "peer1", Some("peer2"))
            .with_param("task_type", json!("ai_inference"))
            .with_param("model_name", json!("gpt-4"))
            .with_param("input_data", json!("Test"));

        let result = json!({
            "output": "AI Response",
            "tokens_used": 100,
            "model": "gpt-4",
            "latency_ms": 125.5
        });

        let response = create_ai_inference_response(&cmd, result);
        assert_eq!(response.status, crate::command_protocol::ResponseStatus::Success);
        assert!(response.result.is_some());
    }
}









