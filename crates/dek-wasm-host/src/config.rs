use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasmHostConfig {
    pub enable_pooling_allocator: bool,
    pub enable_memory_init_cow: bool,
    pub enable_wasmtime_cache: bool,

    pub max_core_instances: u32,
    pub total_memories: u32,
    pub total_tables: u32,
    pub max_memory_bytes: usize,
    pub table_elements: u32,

    pub default_min_warm: usize,
    pub default_max_warm: usize,
    pub default_max_concurrency: usize,
    pub acquire_timeout_ms: u64,
    pub invoke_timeout_ms: u64,
    pub max_worker_uses: u64,
}

impl Default for WasmHostConfig {
    fn default() -> Self {
        Self {
            enable_pooling_allocator: true,
            enable_memory_init_cow: true,
            enable_wasmtime_cache: true,
            max_core_instances: 128,
            total_memories: 128,
            total_tables: 128,
            max_memory_bytes: 64 * 1024 * 1024,
            table_elements: 4096,
            default_min_warm: 2,
            default_max_warm: 16,
            default_max_concurrency: 32,
            acquire_timeout_ms: 25,
            invoke_timeout_ms: 50,
            max_worker_uses: 10_000,
        }
    }
}
