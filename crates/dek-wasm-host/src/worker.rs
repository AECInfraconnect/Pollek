use crate::{compiled_plugin::CompiledPlugin, host_state::HostState};
use anyhow::{bail, Context, Result};
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
    ) -> Result<Self> {
        let state = HostState {
            tenant_id: compiled.key.tenant_id.clone(),
            plugin_id: compiled.key.plugin_id.clone(),
            version: compiled.key.version.clone(),
            request_id,
            deadline: Instant::now() + timeout,
            dirty: false,
        };

        let mut store = Store::new(engine, state);

        let instance = compiled
            .instance_pre
            .instantiate_async(&mut store)
            .await
            .context("failed to instantiate pre-warmed plugin worker")?;

        let memory = instance
            .get_memory(&mut store, "memory")
            .context("plugin missing exported memory")?;

        let alloc = instance
            .get_typed_func::<i32, i32>(&mut store, "alloc")
            .context("plugin missing alloc(size)->ptr")?;

        let dealloc = instance
            .get_typed_func::<(i32, i32), ()>(&mut store, "dealloc")
            .context("plugin missing dealloc(ptr,len)")?;

        let reset = instance
            .get_typed_func::<(), i32>(&mut store, "pollen_plugin_reset")
            .context("plugin missing pollen_plugin_reset()")?;

        let decide = instance
            .get_typed_func::<(i32, i32), i64>(&mut store, "pollen_plugin_decide")
            .context("plugin missing pollen_plugin_decide(ptr,len)")?;

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

    pub async fn invoke_json(&mut self, input: &[u8], max_output: usize) -> Result<Vec<u8>> {
        if input.len() > 1024 * 1024 {
            bail!("plugin input too large");
        }

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
            .context("failed to write input to guest memory")?;

        let result = self
            .decide
            .call_async(&mut self.store, (ptr, input.len() as i32))
            .await
            .context("plugin decide call failed")?;

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
            .context("failed to read output from guest memory")?;

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
