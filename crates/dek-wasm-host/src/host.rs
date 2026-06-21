use crate::{
    compiled_plugin::{build_engine, build_linker, compile_plugin},
    config::WasmHostConfig,
    host_state::HostState,
    plugin_key::PluginKey,
    pool::PluginWorkerPool,
};
use anyhow::Result;
use dashmap::DashMap;
use std::{
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
};
use wasmtime::{Engine, Linker};

pub struct WasmPluginHost {
    cfg: WasmHostConfig,
    engine: Arc<Engine>,
    linker: Linker<HostState>,
    pools: DashMap<String, Arc<PluginWorkerPool>>,
    generation: AtomicU64,
}

impl WasmPluginHost {
    pub fn new(cfg: WasmHostConfig) -> Result<Self> {
        let engine = Arc::new(build_engine(&cfg)?);
        let linker = build_linker(&engine)?;
        Ok(Self {
            cfg,
            engine,
            linker,
            pools: DashMap::new(),
            generation: AtomicU64::new(1),
        })
    }

    pub async fn load_plugin(&self, key: PluginKey, wasm_bytes: &[u8]) -> Result<()> {
        // Verify signature, manifest, ABI, allowed imports, and hash before compile.
        let compiled = Arc::new(compile_plugin(
            &self.engine,
            &self.linker,
            key.clone(),
            wasm_bytes,
        )?);
        let generation = self.generation.fetch_add(1, Ordering::SeqCst);

        let pool = Arc::new(PluginWorkerPool::new(
            self.engine.clone(),
            compiled,
            generation,
            self.cfg.default_min_warm,
            self.cfg.default_max_warm,
            self.cfg.default_max_concurrency,
            self.cfg.max_worker_uses,
            Duration::from_millis(self.cfg.invoke_timeout_ms),
        ));

        pool.prewarm().await?;

        let pool_key = format!(
            "{}:{}:{}:{}",
            key.tenant_id, key.plugin_id, key.version, key.wasm_sha256
        );
        self.pools.insert(pool_key, pool);
        Ok(())
    }

    pub async fn invoke(
        &self,
        pool_key: &str,
        request_id: String,
        input: &[u8],
    ) -> Result<Vec<u8>> {
        let pool = self
            .pools
            .get(pool_key)
            .ok_or_else(|| anyhow::anyhow!("plugin not loaded: {pool_key}"))?
            .clone();

        let acquire_timeout = Duration::from_millis(self.cfg.acquire_timeout_ms);
        let mut lease = pool.acquire(request_id, acquire_timeout).await?;

        let result = lease.worker_mut().invoke_json(input, 1024 * 1024).await;

        // Release only if invocation succeeded. If failed, the worker may be dirty.
        match result {
            Ok(out) => {
                pool.release(lease).await?;
                Ok(out)
            }
            Err(e) => {
                // Drop lease; worker is discarded.
                Err(e)
            }
        }
    }
}
