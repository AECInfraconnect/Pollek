use crate::{config::WasmHostConfig, host_state::HostState, plugin_key::PluginKey};
use anyhow::{Context, Result};
use std::sync::Arc;
use wasmtime::{
    Caller, Config, Engine, InstanceAllocationStrategy, InstancePre, Linker, Module, PoolingAllocationConfig,
};

pub struct CompiledPlugin {
    pub key: PluginKey,
    pub module: Arc<Module>,
    pub instance_pre: Arc<InstancePre<HostState>>,
}

pub fn build_engine(cfg: &WasmHostConfig) -> Result<Engine> {
    let mut config = Config::new();

    // Required if the host uses async functions or wants async execution.
    config.async_support(true);

    // Copy-on-write memory initialization can reduce instantiation cost for modules
    // with initialized memory segments.
    config.memory_init_cow(cfg.enable_memory_init_cow);

    if cfg.enable_pooling_allocator {
        let mut pool = PoolingAllocationConfig::default();
        pool.total_core_instances(cfg.max_core_instances);
        pool.total_memories(cfg.total_memories);
        pool.total_tables(cfg.total_tables);
        let pages = (cfg.max_memory_bytes / 65536) as u64;
        pool.memory_pages(pages);
        pool.table_elements(cfg.table_elements);

        // Optional: keep a number of warm affine slots for frequently used modules.
        pool.max_unused_warm_slots(cfg.max_core_instances / 2);

        config.allocation_strategy(InstanceAllocationStrategy::Pooling(pool));
    }

    if cfg.enable_wasmtime_cache {
        // Uses default cache config. For production, load an explicit cache config path.
        config.cache_config_load_default()?;
    }

    Engine::new(&config)
}

pub fn build_linker(engine: &Engine) -> Result<Linker<HostState>> {
    let mut linker = Linker::new(engine);

    // Example host import: emit audit from plugin.
    linker.func_wrap(
        "pollen_host",
        "audit",
        |mut caller: Caller<'_, HostState>, ptr: i32, len: i32| -> i32 {
            // In production, read guest memory safely and enforce max size.
            let state = caller.data_mut();
            state.dirty = true;
            tracing::debug!(plugin_id = %state.plugin_id, ptr, len, "plugin audit called");
            0
        },
    )?;

    // Example host import: get monotonic time.
    linker.func_wrap(
        "pollen_host",
        "now_ms",
        || -> i64 {
            use std::time::{SystemTime, UNIX_EPOCH};
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as i64
        },
    )?;

    Ok(linker)
}

pub fn compile_plugin(
    engine: &Engine,
    linker: &Linker<HostState>,
    key: PluginKey,
    wasm_bytes: &[u8],
) -> Result<CompiledPlugin> {
    // Signature and hash verification must happen before this function.
    let module = Module::new(engine, wasm_bytes).context("failed to compile wasm module")?;

    let instance_pre = linker
        .instantiate_pre(&module)
        .context("failed to pre-instantiate plugin imports")?;

    Ok(CompiledPlugin {
        key,
        module: Arc::new(module),
        instance_pre: Arc::new(instance_pre),
    })
}
