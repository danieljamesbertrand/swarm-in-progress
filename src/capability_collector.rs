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
        if let Ok(metadata) = std::fs::metadata(".") {
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
}

impl Default for CapabilityCollector {
    fn default() -> Self {
        Self::new()
    }
}

