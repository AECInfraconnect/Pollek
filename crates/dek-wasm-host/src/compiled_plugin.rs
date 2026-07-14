use crate::{config::WasmHostConfig, host_state::HostState, plugin_key::PluginKey};
use anyhow::Result;
use std::sync::Arc;
use wasmtime::{
    Caller, Config, Engine, InstanceAllocationStrategy, InstancePre, Linker, Module,
    PoolingAllocationConfig,
};

pub struct CompiledPlugin {
    pub key: PluginKey,
    pub module: Arc<Module>,
    pub instance_pre: Arc<InstancePre<HostState>>,
}

pub fn build_engine(cfg: &WasmHostConfig) -> Result<Engine> {
    let mut config = Config::new();

    // Async execution is enabled implicitly in wasmtime 46 when the `_async`
    // instantiation/call APIs are used; `Config::async_support` was deprecated
    // to a no-op, so it is no longer set here.

    // Fuel for limiting CPU instructions
    config.consume_fuel(true);

    // Copy-on-write memory initialization can reduce instantiation cost for modules
    // with initialized memory segments.
    config.memory_init_cow(cfg.enable_memory_init_cow);

    if cfg.enable_pooling_allocator {
        let mut pool = PoolingAllocationConfig::default();
        pool.total_core_instances(cfg.max_core_instances);
        pool.total_memories(cfg.total_memories);
        pool.total_tables(cfg.total_tables);
        pool.max_memory_size(cfg.max_memory_bytes);
        pool.table_elements(cfg.table_elements as usize);

        // Optional: keep a number of warm affine slots for frequently used modules.
        pool.max_unused_warm_slots(cfg.max_core_instances / 2);

        config.allocation_strategy(InstanceAllocationStrategy::Pooling(pool));
    }

    if cfg.enable_wasmtime_cache {
        // wasmtime 46 replaced `Config::cache_config_load_default()` with an
        // explicit `Cache` handle. `Cache::from_file(None)` loads the default
        // on-disk cache config; on failure we continue without the cache.
        match wasmtime::Cache::from_file(None) {
            Ok(cache) => {
                config.cache(Some(cache));
            }
            Err(err) => {
                tracing::warn!(
                    error = %err,
                    "failed to load Wasmtime cache config; continuing without the cache"
                );
            }
        }
    }

    Engine::new(&config).map_err(|e| anyhow::anyhow!("failed to build wasm engine: {e:#}"))
}

pub fn build_linker(engine: &Engine) -> Result<Linker<HostState>> {
    let mut linker = Linker::new(engine);

    // Example host import: emit audit from plugin.
    linker.func_wrap(
        "pollek_host",
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
    linker.func_wrap("pollek_host", "now_ms", || -> i64 {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64
    })?;

    Ok(linker)
}

pub fn compile_plugin(
    engine: &Engine,
    linker: &Linker<HostState>,
    key: PluginKey,
    wasm_bytes: &[u8],
) -> Result<CompiledPlugin> {
    // Signature and hash verification must happen before this function.
    let module = Module::new(engine, wasm_bytes)
        .map_err(|e| anyhow::anyhow!("failed to compile wasm module: {e:#}"))?;

    let instance_pre = linker
        .instantiate_pre(&module)
        .map_err(|e| anyhow::anyhow!("failed to pre-instantiate plugin imports: {e:#}"))?;

    Ok(CompiledPlugin {
        key,
        module: Arc::new(module),
        instance_pre: Arc::new(instance_pre),
    })
}
