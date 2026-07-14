use crate::{compiled_plugin::CompiledPlugin, host_state::HostState};
use anyhow::{bail, Result};
use std::time::{Duration, Instant};
use wasmtime::{Engine, Instance, Memory, Store, TypedFunc};

pub struct PluginWorker {
    pub generation: u64,
    pub uses: u64,
    pub store: Store<HostState>,
    pub instance: Instance,
    pub memory: Memory,
    pub alloc: TypedFunc<i32, i32>,
    pub dealloc: TypedFunc<(i32, i32), ()>,
    pub reset: TypedFunc<(), i32>,
    pub decide: TypedFunc<(i32, i32), i64>,
}

impl PluginWorker {
    pub async fn new(
        engine: &Engine,
        compiled: &CompiledPlugin,
        generation: u64,
        request_id: String,
        timeout: Duration,
        max_memory_bytes: usize,
        table_elements: u32,
    ) -> Result<Self> {
        let state = HostState {
            tenant_id: compiled.key.tenant_id.clone(),
            plugin_id: compiled.key.plugin_id.clone(),
            version: compiled.key.version.clone(),
            request_id,
            deadline: Instant::now() + timeout,
            dirty: false,
            limits: wasmtime::StoreLimitsBuilder::new()
                .memory_size(max_memory_bytes)
                .table_elements(table_elements as usize)
                .instances(2)
                .memories(1)
                .tables(1)
                .build(),
        };

        let mut store = Store::new(engine, state);
        store.limiter(|state| &mut state.limits);

        let instance = compiled
            .instance_pre
            .instantiate_async(&mut store)
            .await
            .map_err(|e| {
                anyhow::anyhow!("failed to instantiate pre-warmed plugin worker: {e:#}")
            })?;

        let memory = instance
            .get_memory(&mut store, "memory")
            .ok_or_else(|| anyhow::anyhow!("plugin missing exported memory"))?;

        let alloc = instance
            .get_typed_func::<i32, i32>(&mut store, "alloc")
            .map_err(|e| anyhow::anyhow!("plugin missing alloc(size)->ptr: {e:#}"))?;

        let dealloc = instance
            .get_typed_func::<(i32, i32), ()>(&mut store, "dealloc")
            .map_err(|e| anyhow::anyhow!("plugin missing dealloc(ptr,len): {e:#}"))?;

        let reset = instance
            .get_typed_func::<(), i32>(&mut store, "pollek_plugin_reset")
            .map_err(|e| anyhow::anyhow!("plugin missing pollek_plugin_reset(): {e:#}"))?;

        let decide = instance
            .get_typed_func::<(i32, i32), i64>(&mut store, "pollek_plugin_decide")
            .map_err(|e| anyhow::anyhow!("plugin missing pollek_plugin_decide(ptr,len): {e:#}"))?;

        Ok(Self {
            generation,
            uses: 0,
            store,
            instance,
            memory,
            alloc,
            dealloc,
            reset,
            decide,
        })
    }

    pub async fn invoke_json(
        &mut self,
        input: &[u8],
        max_output: usize,
        fuel_limit: u64,
    ) -> Result<Vec<u8>> {
        if input.len() > 1024 * 1024 {
            bail!("plugin input too large");
        }

        self.store
            .set_fuel(fuel_limit)
            .map_err(|e| anyhow::anyhow!("failed to set fuel limit: {e:#}"))?;

        self.uses += 1;

        let ptr = self
            .alloc
            .call_async(&mut self.store, input.len() as i32)
            .await?;
        if ptr <= 0 {
            bail!("plugin allocation failed");
        }

        self.memory
            .write(&mut self.store, ptr as usize, input)
            .map_err(|e| anyhow::anyhow!("failed to write input to guest memory: {e:#}"))?;

        let result = match self
            .decide
            .call_async(&mut self.store, (ptr, input.len() as i32))
            .await
        {
            Ok(r) => r,
            Err(e) => {
                if let Some(trap) = e.downcast_ref::<wasmtime::Trap>() {
                    tracing::error!("WASM Plugin trapped: {:?}", trap);
                    bail!("Plugin trapped: {:?}", trap);
                }
                bail!("Plugin decide call failed: {:?}", e);
            }
        };

        // Always deallocate input.
        self.dealloc
            .call_async(&mut self.store, (ptr, input.len() as i32))
            .await
            .ok();

        let out_ptr = (result >> 32) as i32;
        let out_len = (result & 0xffff_ffff) as i32;

        if out_ptr <= 0 || out_len < 0 {
            bail!("plugin returned invalid output pointer/length");
        }
        if out_len as usize > max_output {
            bail!("plugin output too large");
        }

        let mut out = vec![0u8; out_len as usize];
        self.memory
            .read(&mut self.store, out_ptr as usize, &mut out)
            .map_err(|e| anyhow::anyhow!("failed to read output from guest memory: {e:#}"))?;

        self.dealloc
            .call_async(&mut self.store, (out_ptr, out_len))
            .await
            .ok();

        Ok(out)
    }

    pub async fn reset_for_reuse(&mut self) -> Result<bool> {
        let rc = self.reset.call_async(&mut self.store, ()).await?;
        Ok(rc == 0 && !self.store.data().dirty)
    }
}
