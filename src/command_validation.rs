//! Command Input Validation Module
//! 
//! Provides validation functions for command protocol to prevent malformed input
//! from crashing nodes or causing security issues.

use crate::command_protocol::{Command, commands};
use std::collections::HashMap;

/// Validation error type
#[derive(Debug, Clone)]
pub enum ValidationError {
    MissingField(String),
    InvalidType(String, String), // field, expected_type
    InvalidRange(String, String), // field, reason
    InvalidValue(String, String), // field, reason
    MalformedInput(String),
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ValidationError::MissingField(field) => write!(f, "Missing required field: {}", field),
            ValidationError::InvalidType(field, expected) => write!(f, "Invalid type for field '{}': expected {}", field, expected),
            ValidationError::InvalidRange(field, reason) => write!(f, "Invalid range for field '{}': {}", field, reason),
            ValidationError::InvalidValue(field, reason) => write!(f, "Invalid value for field '{}': {}", field, reason),
            ValidationError::MalformedInput(msg) => write!(f, "Malformed input: {}", msg),
        }
    }
}

impl std::error::Error for ValidationError {}

/// Validate a command structure (basic fields)
pub fn validate_command_structure(cmd: &Command) -> Result<(), ValidationError> {
    // Validate command name is not empty
    if cmd.command.is_empty() {
        return Err(ValidationError::MissingField("command".to_string()));
    }
    
    // Validate request_id is not empty
    if cmd.request_id.is_empty() {
        return Err(ValidationError::MissingField("request_id".to_string()));
    }
    
    // Validate from (peer_id) is not empty
    if cmd.from.is_empty() {
        return Err(ValidationError::MissingField("from".to_string()));
    }
    
    // Validate timestamp is reasonable (not too far in past/future)
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    
    // Allow 5 minutes in past, 1 minute in future (for clock skew)
    if cmd.timestamp > now + 60 {
        return Err(ValidationError::InvalidValue(
            "timestamp".to_string(),
            format!("Timestamp {} is too far in future (now: {})", cmd.timestamp, now)
        ));
    }
    
    if cmd.timestamp < now.saturating_sub(300) {
        return Err(ValidationError::InvalidValue(
            "timestamp".to_string(),
            format!("Timestamp {} is too far in past (now: {})", cmd.timestamp, now)
        ));
    }
    
    Ok(())
}

/// Validate GET_CAPABILITIES command
pub fn validate_get_capabilities(cmd: &Command) -> Result<(), ValidationError> {
    validate_command_structure(cmd)?;
    
    // GET_CAPABILITIES has no required parameters
    // But validate command name matches
    if cmd.command != commands::GET_CAPABILITIES {
        return Err(ValidationError::InvalidValue(
            "command".to_string(),
            format!("Expected {}, got {}", commands::GET_CAPABILITIES, cmd.command)
        ));
    }
    
    Ok(())
}

/// Validate LOAD_SHARD command
pub fn validate_load_shard(cmd: &Command) -> Result<(), ValidationError> {
    validate_command_structure(cmd)?;
    
    if cmd.command != commands::LOAD_SHARD {
        return Err(ValidationError::InvalidValue(
            "command".to_string(),
            format!("Expected {}, got {}", commands::LOAD_SHARD, cmd.command)
        ));
    }
    
    // Validate shard_id parameter
    let shard_id = cmd.params.get("shard_id")
        .ok_or_else(|| ValidationError::MissingField("shard_id".to_string()))?;
    
    let shard_id_u64 = shard_id.as_u64()
        .ok_or_else(|| ValidationError::InvalidType("shard_id".to_string(), "u64".to_string()))?;
    
    // Validate shard_id is reasonable (0-1000)
    if shard_id_u64 > 1000 {
        return Err(ValidationError::InvalidRange(
            "shard_id".to_string(),
            format!("shard_id {} exceeds maximum 1000", shard_id_u64)
        ));
    }
    
    Ok(())
}

/// Validate EXECUTE_TASK command
pub fn validate_execute_task(cmd: &Command) -> Result<(), ValidationError> {
    validate_command_structure(cmd)?;
    
    if cmd.command != commands::EXECUTE_TASK {
        return Err(ValidationError::InvalidValue(
            "command".to_string(),
            format!("Expected {}, got {}", commands::EXECUTE_TASK, cmd.command)
        ));
    }
    
    // Validate task_type
    let task_type = cmd.params.get("task_type")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ValidationError::MissingField("task_type".to_string()))?;
    
    // Validate task_type is one of allowed values
    if task_type != "llama_fragment" && task_type != "ai_inference" {
        return Err(ValidationError::InvalidValue(
            "task_type".to_string(),
            format!("Invalid task_type: {}. Must be 'llama_fragment' or 'ai_inference'", task_type)
        ));
    }
    
    // Validate input_data (if present)
    if let Some(input_data) = cmd.params.get("input_data") {
        let input_str = input_data.as_str()
            .ok_or_else(|| ValidationError::InvalidType("input_data".to_string(), "string".to_string()))?;
        
        // Validate input_data length (max 1MB)
        if input_str.len() > 1_000_000 {
            return Err(ValidationError::InvalidRange(
                "input_data".to_string(),
                format!("input_data length {} exceeds maximum 1MB", input_str.len())
            ));
        }
    }
    
    // Validate max_tokens (if present)
    if let Some(max_tokens) = cmd.params.get("max_tokens") {
        let max_tokens_u64 = max_tokens.as_u64()
            .ok_or_else(|| ValidationError::InvalidType("max_tokens".to_string(), "u64".to_string()))?;
        
        // Validate max_tokens is reasonable (1-100000)
        if max_tokens_u64 == 0 || max_tokens_u64 > 100_000 {
            return Err(ValidationError::InvalidRange(
                "max_tokens".to_string(),
                format!("max_tokens {} must be between 1 and 100000", max_tokens_u64)
            ));
        }
    }
    
    // Validate temperature (if present)
    if let Some(temperature) = cmd.params.get("temperature") {
        let temp_f64 = temperature.as_f64()
            .ok_or_else(|| ValidationError::InvalidType("temperature".to_string(), "f64".to_string()))?;
        
        // Validate temperature is reasonable (0.0-2.0)
        if temp_f64 < 0.0 || temp_f64 > 2.0 {
            return Err(ValidationError::InvalidRange(
                "temperature".to_string(),
                format!("temperature {} must be between 0.0 and 2.0", temp_f64)
            ));
        }
    }
    
    // Validate shard_id (if present)
    if let Some(shard_id) = cmd.params.get("shard_id") {
        let shard_id_u64 = shard_id.as_u64()
            .ok_or_else(|| ValidationError::InvalidType("shard_id".to_string(), "u64".to_string()))?;
        
        if shard_id_u64 > 1000 {
            return Err(ValidationError::InvalidRange(
                "shard_id".to_string(),
                format!("shard_id {} exceeds maximum 1000", shard_id_u64)
            ));
        }
    }
    
    // Validate layer_start and layer_end (if present)
    if let Some(layer_start) = cmd.params.get("layer_start") {
        let layer_start_u64 = layer_start.as_u64()
            .ok_or_else(|| ValidationError::InvalidType("layer_start".to_string(), "u64".to_string()))?;
        
        if layer_start_u64 > 10000 {
            return Err(ValidationError::InvalidRange(
                "layer_start".to_string(),
                format!("layer_start {} exceeds maximum 10000", layer_start_u64)
            ));
        }
    }
    
    if let Some(layer_end) = cmd.params.get("layer_end") {
        let layer_end_u64 = layer_end.as_u64()
            .ok_or_else(|| ValidationError::InvalidType("layer_end".to_string(), "u64".to_string()))?;
        
        if layer_end_u64 > 10000 {
            return Err(ValidationError::InvalidRange(
                "layer_end".to_string(),
                format!("layer_end {} exceeds maximum 10000", layer_end_u64)
            ));
        }
        
        // Validate layer_end > layer_start if both present
        if let Some(layer_start) = cmd.params.get("layer_start") {
            if let Some(layer_start_u64) = layer_start.as_u64() {
                if layer_end_u64 <= layer_start_u64 {
                    return Err(ValidationError::InvalidRange(
                        "layer_end".to_string(),
                        format!("layer_end {} must be greater than layer_start {}", layer_end_u64, layer_start_u64)
                    ));
                }
            }
        }
    }
    
    Ok(())
}

/// Validate any command based on its type
pub fn validate_command(cmd: &Command) -> Result<(), ValidationError> {
    match cmd.command.as_str() {
        commands::GET_CAPABILITIES => validate_get_capabilities(cmd),
        commands::LOAD_SHARD => validate_load_shard(cmd),
        commands::EXECUTE_TASK => validate_execute_task(cmd),
        commands::GET_REPUTATION => {
            validate_command_structure(cmd)?;
            Ok(())
        }
        commands::UPDATE_REPUTATION => {
            validate_command_structure(cmd)?;
            // Could add more validation here
            Ok(())
        }
        commands::FIND_NODES => {
            validate_command_structure(cmd)?;
            Ok(())
        }
        commands::LIST_FILES => {
            validate_command_structure(cmd)?;
            Ok(())
        }
        commands::GET_FILE_METADATA => {
            validate_command_structure(cmd)?;
            // Could validate info_hash if present
            Ok(())
        }
        commands::REQUEST_PIECE => {
            validate_command_structure(cmd)?;
            // Could validate info_hash and piece_index if present
            Ok(())
        }
        _ => {
            // Unknown command - still validate structure
            validate_command_structure(cmd)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command_protocol::Command;
    
    #[test]
    fn test_validate_command_structure() {
        let cmd = Command::new("TEST", "peer1", Some("peer2"));
        assert!(validate_command_structure(&cmd).is_ok());
    }
    
    #[test]
    fn test_validate_missing_command() {
        let mut cmd = Command::new("", "peer1", Some("peer2"));
        assert!(validate_command_structure(&cmd).is_err());
    }
    
    #[test]
    fn test_validate_load_shard() {
        let mut cmd = Command::new(commands::LOAD_SHARD, "peer1", Some("peer2"));
        cmd.params.insert("shard_id".to_string(), serde_json::json!(5));
        assert!(validate_load_shard(&cmd).is_ok());
    }
    
    #[test]
    fn test_validate_load_shard_missing_param() {
        let cmd = Command::new(commands::LOAD_SHARD, "peer1", Some("peer2"));
        assert!(validate_load_shard(&cmd).is_err());
    }
    
    #[test]
    fn test_validate_execute_task() {
        let mut cmd = Command::new(commands::EXECUTE_TASK, "peer1", Some("peer2"));
        cmd.params.insert("task_type".to_string(), serde_json::json!("ai_inference"));
        cmd.params.insert("input_data".to_string(), serde_json::json!("test input"));
        cmd.params.insert("max_tokens".to_string(), serde_json::json!(100));
        cmd.params.insert("temperature".to_string(), serde_json::json!(0.7));
        assert!(validate_execute_task(&cmd).is_ok());
    }
    
    #[test]
    fn test_validate_execute_task_invalid_temperature() {
        let mut cmd = Command::new(commands::EXECUTE_TASK, "peer1", Some("peer2"));
        cmd.params.insert("task_type".to_string(), serde_json::json!("ai_inference"));
        cmd.params.insert("temperature".to_string(), serde_json::json!(3.0)); // Too high
        assert!(validate_execute_task(&cmd).is_err());
    }
}

