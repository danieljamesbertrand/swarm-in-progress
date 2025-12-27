//! Shard Optimization Module - Size vs Speed tradeoffs for distributed AI inference
//!
//! This module provides quantization types and optimization configurations
//! that allow trading model size for inference speed.
//!
//! ## Quantization Levels
//! - FP32: Full precision (training only)
//! - FP16: Half precision (default production)
//! - INT8: 8-bit quantized (good balance)
//! - Q4_K_M: 4-bit quantized (recommended for consumer GPUs)
//!
//! ## Usage
//! ```rust,ignore
//! use punch_simple::shard_optimization::{QuantizationType, ShardOptimization};
//!
//! let opt = ShardOptimization::balanced();
//! println!("Size factor: {}x", opt.quantization.size_factor());
//! println!("Speed factor: {}x", opt.quantization.speed_factor());
//! ```

use serde::{Deserialize, Serialize};

/// Quantization type for model weights
/// 
/// Lower precision = smaller size + faster inference, but may reduce quality.
/// The Q4_K_M format is the recommended sweet spot for most use cases.
#[allow(non_camel_case_types)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum QuantizationType {
    /// 32-bit floating point (full precision, training only)
    FP32,
    /// 16-bit floating point (default production)
    #[default]
    FP16,
    /// Brain float 16 (better numerical properties for training)
    BF16,
    /// 8-bit integer quantization
    INT8,
    /// GGML 8-bit quantization
    Q8_0,
    /// GGML 6-bit quantization (good quality)
    Q6_K,
    /// GGML 5-bit medium quantization
    Q5_K_M,
    /// GGML 5-bit small quantization
    Q5_K_S,
    /// GGML 4-bit medium quantization (RECOMMENDED)
    Q4_K_M,
    /// GGML 4-bit small quantization
    Q4_K_S,
    /// GGML 4-bit basic quantization (fastest 4-bit)
    Q4_0,
    /// GGML 3-bit medium quantization (experimental)
    Q3_K_M,
    /// GGML 3-bit small quantization (experimental)
    Q3_K_S,
    /// GGML 2-bit quantization (research only)
    Q2_K,
    /// 1-bit quantization (binary weights, experimental)
    Binary,
}

impl QuantizationType {
    /// Size factor relative to FP32 (1.0 = same size as FP32)
    /// 
    /// Example: Q4_K_M returns 0.125 (8x smaller than FP32)
    pub fn size_factor(&self) -> f32 {
        match self {
            Self::FP32 => 1.0,
            Self::FP16 | Self::BF16 => 0.5,
            Self::INT8 | Self::Q8_0 => 0.25,
            Self::Q6_K => 0.1875,
            Self::Q5_K_M | Self::Q5_K_S => 0.156,
            Self::Q4_K_M | Self::Q4_K_S | Self::Q4_0 => 0.125,
            Self::Q3_K_M | Self::Q3_K_S => 0.094,
            Self::Q2_K => 0.0625,
            Self::Binary => 0.03125,
        }
    }

    /// Speed multiplier relative to FP32 (higher = faster)
    /// 
    /// Example: Q4_K_M returns 5.0 (5x faster than FP32)
    pub fn speed_factor(&self) -> f32 {
        match self {
            Self::FP32 => 1.0,
            Self::FP16 | Self::BF16 => 2.0,
            Self::INT8 | Self::Q8_0 => 3.5,
            Self::Q6_K => 4.0,
            Self::Q5_K_M | Self::Q5_K_S => 4.5,
            Self::Q4_K_M => 5.0,
            Self::Q4_K_S | Self::Q4_0 => 6.0,
            Self::Q3_K_M | Self::Q3_K_S => 7.0,
            Self::Q2_K => 10.0,
            Self::Binary => 15.0,
        }
    }

    /// Quality retention factor (0.0 - 1.0, higher = better quality)
    /// 
    /// Example: Q4_K_M returns 0.96 (96% of original quality)
    pub fn quality_factor(&self) -> f32 {
        match self {
            Self::FP32 => 1.0,
            Self::FP16 | Self::BF16 => 0.999,
            Self::INT8 | Self::Q8_0 => 0.99,
            Self::Q6_K => 0.98,
            Self::Q5_K_M => 0.97,
            Self::Q5_K_S => 0.965,
            Self::Q4_K_M => 0.96,  // Sweet spot!
            Self::Q4_K_S => 0.95,
            Self::Q4_0 => 0.94,
            Self::Q3_K_M => 0.90,
            Self::Q3_K_S => 0.88,
            Self::Q2_K => 0.85,
            Self::Binary => 0.75,
        }
    }

    /// GGUF file suffix for this quantization type
    pub fn gguf_suffix(&self) -> &'static str {
        match self {
            Self::FP32 => "f32",
            Self::FP16 => "f16",
            Self::BF16 => "bf16",
            Self::INT8 => "i8",
            Self::Q8_0 => "q8_0",
            Self::Q6_K => "q6_k",
            Self::Q5_K_M => "q5_k_m",
            Self::Q5_K_S => "q5_k_s",
            Self::Q4_K_M => "q4_k_m",
            Self::Q4_K_S => "q4_k_s",
            Self::Q4_0 => "q4_0",
            Self::Q3_K_M => "q3_k_m",
            Self::Q3_K_S => "q3_k_s",
            Self::Q2_K => "q2_k",
            Self::Binary => "b1",
        }
    }

    /// Parse quantization type from GGUF filename
    pub fn from_filename(filename: &str) -> Option<Self> {
        let lower = filename.to_lowercase();
        
        if lower.contains("q2_k") { return Some(Self::Q2_K); }
        if lower.contains("q3_k_s") { return Some(Self::Q3_K_S); }
        if lower.contains("q3_k_m") { return Some(Self::Q3_K_M); }
        if lower.contains("q4_0") { return Some(Self::Q4_0); }
        if lower.contains("q4_k_s") { return Some(Self::Q4_K_S); }
        if lower.contains("q4_k_m") { return Some(Self::Q4_K_M); }
        if lower.contains("q5_k_s") { return Some(Self::Q5_K_S); }
        if lower.contains("q5_k_m") { return Some(Self::Q5_K_M); }
        if lower.contains("q6_k") { return Some(Self::Q6_K); }
        if lower.contains("q8_0") { return Some(Self::Q8_0); }
        if lower.contains("f32") { return Some(Self::FP32); }
        if lower.contains("f16") { return Some(Self::FP16); }
        if lower.contains("bf16") { return Some(Self::BF16); }
        
        None
    }

    /// Get all available quantization types, ordered by quality (highest first)
    pub fn all_by_quality() -> Vec<Self> {
        vec![
            Self::FP32, Self::FP16, Self::BF16, Self::INT8, Self::Q8_0,
            Self::Q6_K, Self::Q5_K_M, Self::Q5_K_S, Self::Q4_K_M, Self::Q4_K_S,
            Self::Q4_0, Self::Q3_K_M, Self::Q3_K_S, Self::Q2_K, Self::Binary,
        ]
    }

    /// Get all available quantization types, ordered by speed (fastest first)
    pub fn all_by_speed() -> Vec<Self> {
        let mut types = Self::all_by_quality();
        types.reverse();
        types
    }

    /// Estimate memory usage in MB for a given parameter count
    pub fn memory_mb(&self, params_billions: f32) -> f32 {
        // Each param in FP32 = 4 bytes
        let fp32_size_mb = params_billions * 1000.0 * 4.0;
        fp32_size_mb * self.size_factor()
    }

    /// Check if this quantization fits in available memory
    pub fn fits_in_memory(&self, params_billions: f32, available_memory_mb: u64) -> bool {
        self.memory_mb(params_billions) <= available_memory_mb as f32
    }
}

impl std::fmt::Display for QuantizationType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.gguf_suffix().to_uppercase())
    }
}

/// Optimization priority for shard selection
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum OptimizationPriority {
    /// Prioritize inference speed (chatbot, real-time)
    Speed,
    /// Prioritize output quality (legal, medical, code gen)
    Quality,
    /// Balance speed and quality (default)
    #[default]
    Balanced,
    /// Minimize memory usage (edge devices)
    Memory,
}

/// Configuration for shard optimization
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ShardOptimization {
    /// Quantization level for model weights
    pub quantization: QuantizationType,
    /// Number of layers per shard (can vary by node capability)
    pub layers_per_shard: u32,
    /// Enable speculative decoding with a draft model
    pub speculative_decoding: bool,
    /// Path to draft model for speculative decoding
    pub draft_model_path: Option<String>,
    /// Draft model quantization (usually more aggressive)
    pub draft_quantization: Option<QuantizationType>,
    /// Use paged attention for memory efficiency
    pub use_paged_attention: bool,
    /// Use flash attention for speed
    pub use_flash_attention: bool,
    /// Compress activations between shards
    pub compress_activations: bool,
    /// Activation compression quantization
    pub activation_quantization: QuantizationType,
    /// KV cache quantization for memory savings
    pub kv_cache_quantization: Option<QuantizationType>,
}

impl Default for ShardOptimization {
    fn default() -> Self {
        Self::balanced()
    }
}

impl ShardOptimization {
    /// Create a speed-optimized configuration
    pub fn speed() -> Self {
        Self {
            quantization: QuantizationType::Q4_0,
            layers_per_shard: 8,
            speculative_decoding: true,
            draft_model_path: None,
            draft_quantization: Some(QuantizationType::Q2_K),
            use_paged_attention: true,
            use_flash_attention: true,
            compress_activations: true,
            activation_quantization: QuantizationType::INT8,
            kv_cache_quantization: Some(QuantizationType::Q4_0),
        }
    }

    /// Create a quality-optimized configuration
    pub fn quality() -> Self {
        Self {
            quantization: QuantizationType::FP16,
            layers_per_shard: 8,
            speculative_decoding: false,
            draft_model_path: None,
            draft_quantization: None,
            use_paged_attention: true,
            use_flash_attention: true,
            compress_activations: false,
            activation_quantization: QuantizationType::FP16,
            kv_cache_quantization: None,
        }
    }

    /// Create a balanced configuration (RECOMMENDED)
    pub fn balanced() -> Self {
        Self {
            quantization: QuantizationType::Q4_K_M,
            layers_per_shard: 8,
            speculative_decoding: false,
            draft_model_path: None,
            draft_quantization: None,
            use_paged_attention: true,
            use_flash_attention: true,
            compress_activations: true,
            activation_quantization: QuantizationType::INT8,
            kv_cache_quantization: Some(QuantizationType::Q8_0),
        }
    }

    /// Create a memory-optimized configuration for edge devices
    pub fn memory() -> Self {
        Self {
            quantization: QuantizationType::Q2_K,
            layers_per_shard: 4,
            speculative_decoding: false,
            draft_model_path: None,
            draft_quantization: None,
            use_paged_attention: true,
            use_flash_attention: false,
            compress_activations: true,
            activation_quantization: QuantizationType::Q4_0,
            kv_cache_quantization: Some(QuantizationType::Q4_0),
        }
    }

    /// Create configuration based on priority
    pub fn from_priority(priority: OptimizationPriority) -> Self {
        match priority {
            OptimizationPriority::Speed => Self::speed(),
            OptimizationPriority::Quality => Self::quality(),
            OptimizationPriority::Balanced => Self::balanced(),
            OptimizationPriority::Memory => Self::memory(),
        }
    }

    /// Estimate total size factor relative to FP32
    pub fn total_size_factor(&self) -> f32 {
        let mut factor = self.quantization.size_factor();
        
        if let Some(kv_quant) = &self.kv_cache_quantization {
            // KV cache is typically 20-30% of memory
            factor *= 0.75 + 0.25 * kv_quant.size_factor();
        }
        
        factor
    }

    /// Estimate total speed factor relative to FP32
    pub fn total_speed_factor(&self) -> f32 {
        let mut factor = self.quantization.speed_factor();
        
        if self.use_flash_attention {
            factor *= 1.5;
        }
        
        if self.speculative_decoding {
            factor *= 3.0; // Speculative decoding can give 3-5x speedup
        }
        
        factor
    }

    /// Estimate quality retention (0.0 - 1.0)
    pub fn estimated_quality(&self) -> f32 {
        let mut quality = self.quantization.quality_factor();
        
        if self.compress_activations && self.activation_quantization != QuantizationType::FP16 {
            quality *= 0.99; // Small quality loss from activation compression
        }
        
        if let Some(kv_quant) = &self.kv_cache_quantization {
            quality *= 0.99 + 0.01 * kv_quant.quality_factor();
        }
        
        quality
    }
}

/// Select optimal quantization based on available memory and priority
pub fn select_quantization(
    model_params_billions: f32,
    available_memory_mb: u64,
    priority: OptimizationPriority,
) -> QuantizationType {
    let candidates = match priority {
        OptimizationPriority::Speed => QuantizationType::all_by_speed(),
        OptimizationPriority::Quality => QuantizationType::all_by_quality(),
        OptimizationPriority::Balanced | OptimizationPriority::Memory => {
            // For balanced/memory, prefer Q4_K_M first
            vec![
                QuantizationType::Q4_K_M,
                QuantizationType::Q4_K_S,
                QuantizationType::Q4_0,
                QuantizationType::Q5_K_M,
                QuantizationType::Q3_K_M,
                QuantizationType::Q2_K,
                QuantizationType::Q6_K,
                QuantizationType::Q8_0,
                QuantizationType::FP16,
                QuantizationType::FP32,
            ]
        }
    };

    for quant in candidates {
        if quant.fits_in_memory(model_params_billions, available_memory_mb) {
            return quant;
        }
    }

    // Fallback to most aggressive quantization
    QuantizationType::Q2_K
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quantization_size_factors() {
        assert_eq!(QuantizationType::FP32.size_factor(), 1.0);
        assert_eq!(QuantizationType::FP16.size_factor(), 0.5);
        assert_eq!(QuantizationType::Q4_K_M.size_factor(), 0.125);
        
        // Verify ordering: smaller quantization = smaller size
        assert!(QuantizationType::Q4_K_M.size_factor() < QuantizationType::FP16.size_factor());
        assert!(QuantizationType::Q2_K.size_factor() < QuantizationType::Q4_K_M.size_factor());
    }

    #[test]
    fn test_quantization_speed_factors() {
        assert_eq!(QuantizationType::FP32.speed_factor(), 1.0);
        assert!(QuantizationType::Q4_K_M.speed_factor() > QuantizationType::FP16.speed_factor());
        assert!(QuantizationType::Q2_K.speed_factor() > QuantizationType::Q4_K_M.speed_factor());
    }

    #[test]
    fn test_quantization_quality_factors() {
        assert_eq!(QuantizationType::FP32.quality_factor(), 1.0);
        assert!(QuantizationType::FP16.quality_factor() > 0.99);
        assert!(QuantizationType::Q4_K_M.quality_factor() > 0.95);
        assert!(QuantizationType::Q2_K.quality_factor() < QuantizationType::Q4_K_M.quality_factor());
    }

    #[test]
    fn test_quantization_from_filename() {
        assert_eq!(
            QuantizationType::from_filename("llama-7b-q4_k_m.gguf"),
            Some(QuantizationType::Q4_K_M)
        );
        assert_eq!(
            QuantizationType::from_filename("model-Q8_0.gguf"),
            Some(QuantizationType::Q8_0)
        );
        assert_eq!(
            QuantizationType::from_filename("unknown-format.bin"),
            None
        );
    }

    #[test]
    fn test_memory_estimation() {
        // 7B model in FP32 = 7 * 1000 * 4 = 28000 MB = 28 GB
        let mem = QuantizationType::FP32.memory_mb(7.0);
        assert!((mem - 28000.0).abs() < 1.0);
        
        // 7B model in Q4_K_M = 28000 * 0.125 = 3500 MB = 3.5 GB
        let mem = QuantizationType::Q4_K_M.memory_mb(7.0);
        assert!((mem - 3500.0).abs() < 1.0);
    }

    #[test]
    fn test_fits_in_memory() {
        // 7B model Q4_K_M needs ~3.5GB, should fit in 8GB
        assert!(QuantizationType::Q4_K_M.fits_in_memory(7.0, 8000));
        
        // 70B model FP16 needs ~140GB, won't fit in 24GB
        assert!(!QuantizationType::FP16.fits_in_memory(70.0, 24000));
        
        // 70B model Q4_K_M needs ~35GB, needs sharding to fit in 24GB per node
        // With 2 shards, each needs ~17.5GB which fits in 24GB
        assert!(!QuantizationType::Q4_K_M.fits_in_memory(70.0, 24000));
        assert!(QuantizationType::Q4_K_M.fits_in_memory(35.0, 24000)); // Half model fits
    }

    #[test]
    fn test_select_quantization() {
        // With 8GB, 7B model should use Q4_K_M (balanced)
        let quant = select_quantization(7.0, 8000, OptimizationPriority::Balanced);
        assert_eq!(quant, QuantizationType::Q4_K_M);
        
        // With 16GB, 7B model for quality should use FP16
        let quant = select_quantization(7.0, 16000, OptimizationPriority::Quality);
        assert_eq!(quant, QuantizationType::FP16);
        
        // With 4GB, 7B model should use Q2_K
        let quant = select_quantization(7.0, 4000, OptimizationPriority::Balanced);
        assert!(quant.size_factor() <= QuantizationType::Q4_K_M.size_factor());
    }

    #[test]
    fn test_shard_optimization_presets() {
        let speed = ShardOptimization::speed();
        let quality = ShardOptimization::quality();
        let balanced = ShardOptimization::balanced();
        
        // Speed should have faster total speed
        assert!(speed.total_speed_factor() > balanced.total_speed_factor());
        
        // Quality should have higher estimated quality
        assert!(quality.estimated_quality() > speed.estimated_quality());
        
        // Balanced should be in between
        assert!(balanced.estimated_quality() > speed.estimated_quality());
    }

    #[test]
    fn test_quantization_display() {
        assert_eq!(format!("{}", QuantizationType::Q4_K_M), "Q4_K_M");
        assert_eq!(format!("{}", QuantizationType::FP16), "F16");
    }

    #[test]
    fn test_gguf_suffix() {
        assert_eq!(QuantizationType::Q4_K_M.gguf_suffix(), "q4_k_m");
        assert_eq!(QuantizationType::FP16.gguf_suffix(), "f16");
        assert_eq!(QuantizationType::Q8_0.gguf_suffix(), "q8_0");
    }

    #[test]
    fn test_from_priority() {
        let opt = ShardOptimization::from_priority(OptimizationPriority::Speed);
        assert_eq!(opt.quantization, QuantizationType::Q4_0);
        assert!(opt.speculative_decoding);
        
        let opt = ShardOptimization::from_priority(OptimizationPriority::Quality);
        assert_eq!(opt.quantization, QuantizationType::FP16);
        assert!(!opt.speculative_decoding);
    }

    #[test]
    fn test_llama_70b_memory_requirements() {
        // Real-world test: Llama 70B
        let params = 70.0;
        
        // FP32: ~280GB - unrealistic
        let fp32_mem = QuantizationType::FP32.memory_mb(params);
        assert!(fp32_mem > 250000.0);
        
        // FP16: ~140GB - needs 2x A100 80GB
        let fp16_mem = QuantizationType::FP16.memory_mb(params);
        assert!(fp16_mem > 130000.0 && fp16_mem < 150000.0);
        
        // Q4_K_M: ~35GB - fits on RTX 4090 24GB with sharding!
        let q4_mem = QuantizationType::Q4_K_M.memory_mb(params);
        assert!(q4_mem > 30000.0 && q4_mem < 40000.0);
        
        // With 4 shards, each shard needs ~8.75GB
        let shard_mem = q4_mem / 4.0;
        assert!(shard_mem < 10000.0); // Fits in 10GB
    }
}

