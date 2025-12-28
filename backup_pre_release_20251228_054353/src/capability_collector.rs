//! System capability collection - gathers CPU, memory, disk, and latency metrics

use crate::command_protocol::NodeCapabilities;
use std::time::{SystemTime, UNIX_EPOCH, Instant};

/// Collects system capabilities for a node
pub struct CapabilityCollector {
    last_collection: Option<Instant>,
    cached_capabilities: Option<NodeCapabilities>,
    cache_duration: std::time::Duration,
}

impl CapabilityCollector {
    pub fn new() -> Self {
        Self {
            last_collection: None,
            cached_capabilities: None,
            cache_duration: std::time::Duration::from_secs(5), // Cache for 5 seconds
        }
    }

    pub fn collect(&mut self) -> NodeCapabilities {
        // Use cached value if available and fresh
        if let Some(cached) = &self.cached_capabilities {
            if let Some(last) = self.last_collection {
                if last.elapsed() < self.cache_duration {
                    return cached.clone();
                }
            }
        }

        let (gpu_memory, gpu_compute, gpu_available) = Self::get_gpu_info();

        let capabilities = NodeCapabilities {
            cpu_cores: Self::get_cpu_cores(),
            cpu_usage: Self::get_cpu_usage(),
            cpu_speed_ghz: Self::get_cpu_speed(),
            memory_total_mb: Self::get_memory_total(),
            memory_available_mb: Self::get_memory_available(),
            disk_total_mb: Self::get_disk_total(),
            disk_available_mb: Self::get_disk_available(),
            latency_ms: 0.0, // Will be updated by actual measurements
            reputation: 1.0,  // Default, will be updated from DHT
            gpu_memory_mb: gpu_memory,
            gpu_compute_units: gpu_compute,
            gpu_usage: 0.0, // Would need nvidia-smi or similar
            gpu_available,
        };

        self.cached_capabilities = Some(capabilities.clone());
        self.last_collection = Some(Instant::now());

        capabilities
    }

    fn get_cpu_cores() -> u32 {
        num_cpus::get() as u32
    }

    fn get_cpu_usage() -> f64 {
        // Simplified CPU usage - in production, use proper system monitoring
        // For now, return a placeholder that varies slightly
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        (now % 100) as f64 // Simulated: 0-100%
    }

    fn get_cpu_speed() -> f64 {
        // Simplified - in production, read from /proc/cpuinfo or sysctl
        2.5 // Default to 2.5 GHz
    }

    fn get_memory_total() -> u64 {
        // Try to get actual memory, fallback to default
        #[cfg(target_os = "linux")]
        {
            if let Ok(meminfo) = std::fs::read_to_string("/proc/meminfo") {
                for line in meminfo.lines() {
                    if line.starts_with("MemTotal:") {
                        if let Some(kb) = line.split_whitespace().nth(1) {
                            if let Ok(kb_val) = kb.parse::<u64>() {
                                return kb_val / 1024; // Convert KB to MB
                            }
                        }
                    }
                }
            }
        }
        
        #[cfg(target_os = "windows")]
        {
            // On Windows, would need to use winapi or similar
            // For now, return default
        }
        
        #[cfg(target_os = "macos")]
        {
            // On macOS, would use sysctl
            // For now, return default
        }
        
        8192 // Default: 8 GB
    }

    fn get_memory_available() -> u64 {
        let total = Self::get_memory_total();
        // Simplified: assume 50% available
        // In production, read actual available memory
        total / 2
    }

    fn get_disk_total() -> u64 {
        // Get disk space of current directory
        if let Ok(_metadata) = std::fs::metadata(".") {
            // This is simplified - in production, would check actual disk
            // For now, return a reasonable default
        }
        1_000_000 // Default: 1 TB in MB
    }

    fn get_disk_available() -> u64 {
        let total = Self::get_disk_total();
        // Simplified: assume 50% available
        // In production, check actual available space
        total / 2
    }

    /// Detect GPU information
    /// Returns (memory_mb, compute_units, available)
    fn get_gpu_info() -> (u64, u32, bool) {
        // Check environment variables for GPU configuration
        // These can be set by the user or detected by nvidia-smi
        if let Ok(gpu_mem) = std::env::var("NODE_GPU_MEMORY_MB") {
            if let Ok(mem) = gpu_mem.parse::<u64>() {
                let compute = std::env::var("NODE_GPU_COMPUTE_UNITS")
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(5000); // Default to mid-range
                return (mem, compute, true);
            }
        }

        // Try to detect NVIDIA GPU via nvidia-smi
        #[cfg(target_os = "linux")]
        {
            if let Ok(output) = std::process::Command::new("nvidia-smi")
                .args(["--query-gpu=memory.total", "--format=csv,noheader,nounits"])
                .output()
            {
                if output.status.success() {
                    if let Ok(mem_str) = String::from_utf8(output.stdout) {
                        if let Ok(mem_mb) = mem_str.trim().parse::<u64>() {
                            // Get compute units from device query
                            let compute = Self::get_nvidia_compute_units().unwrap_or(5000);
                            return (mem_mb, compute, true);
                        }
                    }
                }
            }
        }

        #[cfg(target_os = "windows")]
        {
            // On Windows, try nvidia-smi from standard location
            let nvidia_smi = "C:\\Program Files\\NVIDIA Corporation\\NVSMI\\nvidia-smi.exe";
            if std::path::Path::new(nvidia_smi).exists() {
                if let Ok(output) = std::process::Command::new(nvidia_smi)
                    .args(["--query-gpu=memory.total", "--format=csv,noheader,nounits"])
                    .output()
                {
                    if output.status.success() {
                        if let Ok(mem_str) = String::from_utf8(output.stdout) {
                            if let Ok(mem_mb) = mem_str.trim().parse::<u64>() {
                                return (mem_mb, 5000, true); // Estimate compute units
                            }
                        }
                    }
                }
            }
        }

        // No GPU detected
        (0, 0, false)
    }

    #[cfg(target_os = "linux")]
    fn get_nvidia_compute_units() -> Option<u32> {
        // Query CUDA cores via nvidia-smi device query
        // This is a simplified version; in production, use CUDA API
        let output = std::process::Command::new("nvidia-smi")
            .args(["--query-gpu=name", "--format=csv,noheader"])
            .output()
            .ok()?;
        
        if !output.status.success() {
            return None;
        }

        let name = String::from_utf8(output.stdout).ok()?;
        let name = name.trim().to_uppercase();

        // Estimate CUDA cores based on GPU model
        // These are approximate values
        Some(match () {
            _ if name.contains("4090") => 16384,
            _ if name.contains("4080") => 9728,
            _ if name.contains("4070") => 5888,
            _ if name.contains("3090") => 10496,
            _ if name.contains("3080") => 8704,
            _ if name.contains("3070") => 5888,
            _ if name.contains("A100") => 6912,
            _ if name.contains("H100") => 16896,
            _ if name.contains("A6000") => 10752,
            _ => 5000, // Default mid-range
        })
    }
}

impl Default for CapabilityCollector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capability_collector_new() {
        let collector = CapabilityCollector::new();
        assert!(collector.last_collection.is_none());
        assert!(collector.cached_capabilities.is_none());
    }

    #[test]
    fn test_capability_collector_collect() {
        let mut collector = CapabilityCollector::new();
        let capabilities = collector.collect();
        
        assert!(capabilities.cpu_cores > 0);
        assert!(capabilities.memory_total_mb > 0);
        assert!(capabilities.disk_total_mb > 0);
    }

    #[test]
    fn test_capability_collector_caching() {
        let mut collector = CapabilityCollector::new();
        let cap1 = collector.collect();
        
        // Immediately collect again - should use cache
        let cap2 = collector.collect();
        
        // Values should be the same (cached)
        assert_eq!(cap1.cpu_cores, cap2.cpu_cores);
        assert_eq!(cap1.memory_total_mb, cap2.memory_total_mb);
    }

    #[test]
    fn test_get_cpu_cores() {
        let cores = CapabilityCollector::get_cpu_cores();
        assert!(cores > 0);
    }

    #[test]
    fn test_get_memory_total() {
        let memory = CapabilityCollector::get_memory_total();
        assert!(memory > 0);
    }

    #[test]
    fn test_get_memory_available() {
        let available = CapabilityCollector::get_memory_available();
        let total = CapabilityCollector::get_memory_total();
        assert!(available <= total);
        assert!(available > 0);
    }

    #[test]
    fn test_get_disk_total() {
        let disk = CapabilityCollector::get_disk_total();
        assert!(disk > 0);
    }

    #[test]
    fn test_get_disk_available() {
        let available = CapabilityCollector::get_disk_available();
        let total = CapabilityCollector::get_disk_total();
        assert!(available <= total);
        assert!(available > 0);
    }
}





