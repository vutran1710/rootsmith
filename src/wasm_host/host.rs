use anyhow::Result;
use wasmer::{imports, Engine, Instance, Memory, MemoryView, Module, Store, TypedFunction};
use wasmer_compiler_cranelift::Cranelift;

use crate::wasm_host::error::WasmHostError;
use crate::wasm_host::limits::WasmLimits;

/// WASM plugin host with sandboxing and resource limits
pub struct WasmPluginHost {
    store: Store,
    memory: Memory,
    limits: WasmLimits,

    // Required exports
    alloc: TypedFunction<i32, i32>,
    process: TypedFunction<(i32, i32), i32>,

    // Optional exports
    dealloc: Option<TypedFunction<(i32, i32), ()>>,
    get_api_version: Option<TypedFunction<(), i32>>,
}

impl std::fmt::Debug for WasmPluginHost {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WasmPluginHost")
            .field("limits", &self.limits)
            .field("has_dealloc", &self.dealloc.is_some())
            .field("has_get_api_version", &self.get_api_version.is_some())
            .finish()
    }
}

impl WasmPluginHost {
    /// Load a WASM plugin from file with resource limits
    ///
    /// # Arguments
    /// * `path` - Path to the .wasm file
    /// * `limits` - Resource limits (memory, response size)
    ///
    /// # Errors
    /// - Returns error if plugin is missing required exports (memory, alloc, process)
    /// - Returns error if plugin memory exceeds limits
    /// - Returns error if plugin has no memory maximum declared
    pub fn load(path: &str, limits: WasmLimits) -> Result<Self> {
        let engine: Engine = Cranelift::default().into();
        let mut store = Store::new(engine);

        // Load and instantiate module with empty imports (no WASI, no FS, no network)
        let module = Module::from_file(&store, path)?;
        let instance = Instance::new(&mut store, &module, &imports! {})?;

        // Resolve required exports
        let memory = instance
            .exports
            .get_memory("memory")
            .map_err(|_| WasmHostError::MissingExport("memory"))?
            .clone();

        let alloc = instance
            .exports
            .get_typed_function::<i32, i32>(&store, "alloc")
            .map_err(|_| WasmHostError::MissingExport("alloc"))?;

        let process = instance
            .exports
            .get_typed_function::<(i32, i32), i32>(&store, "process")
            .map_err(|_| WasmHostError::MissingExport("process"))?;

        // Resolve optional exports
        let dealloc = instance
            .exports
            .get_typed_function::<(i32, i32), ()>(&store, "dealloc")
            .ok();

        let get_api_version = instance
            .exports
            .get_typed_function::<(), i32>(&store, "get_api_version")
            .ok();

        // Validate memory limits
        Self::validate_memory_limits(&memory, &store, &limits)?;

        Ok(Self {
            store,
            memory,
            limits,
            alloc,
            process,
            dealloc,
            get_api_version,
        })
    }

    /// Validate that plugin memory has a maximum and it doesn't exceed limits
    fn validate_memory_limits(memory: &Memory, store: &Store, limits: &WasmLimits) -> Result<()> {
        let mem_type = memory.ty(store);

        // Check if maximum is declared
        let max_pages = mem_type
            .maximum
            .ok_or(WasmHostError::NoMemoryMaximum)?;

        // Check if maximum exceeds limit
        if max_pages.0 > limits.max_memory_pages {
            return Err(WasmHostError::MemoryLimitExceeded {
                declared: max_pages.0,
                max: limits.max_memory_pages,
            }
            .into());
        }

        Ok(())
    }

    /// Get the API version from the plugin (if exported)
    ///
    /// Returns (major, minor) encoded as (MAJOR << 16) | MINOR
    pub fn api_version(&mut self) -> Option<(u16, u16)> {
        let f = self.get_api_version.as_ref()?;
        let v = f.call(&mut self.store).ok()? as u32;
        Some(((v >> 16) as u16, (v & 0xFFFF) as u16))
    }

    /// Process input bytes through the plugin
    ///
    /// This is the main entry point for executing the plugin:
    /// 1. Allocates memory in plugin
    /// 2. Writes input bytes
    /// 3. Calls process()
    /// 4. Reads response header (status + length)
    /// 5. Reads response payload
    /// 6. Deallocates memory (if plugin provides dealloc)
    ///
    /// # Process() Return Layout
    ///
    /// The pointer returned by process() must point to:
    /// ```text
    /// [u32 status][u32 len][u8 payload...]
    /// ```
    ///
    /// - status = 0 → success, payload is output bytes
    /// - status = 1 → error, payload is UTF-8 error string
    ///
    /// # Errors
    /// - Returns `PluginError` if plugin returns status=1 with error message
    /// - Returns `ResponseTooLarge` if response exceeds max_response_bytes
    /// - Returns `PluginTrap` for runtime traps
    pub fn process_bytes(&mut self, input: &[u8]) -> Result<Vec<u8>> {
        // 1. Allocate memory in plugin
        let in_ptr = self
            .alloc
            .call(&mut self.store, input.len() as i32)
            .map_err(|e| WasmHostError::PluginTrap(e.to_string()))?;

        // 2. Write input to plugin memory
        self.write_memory(in_ptr, input)?;

        // 3. Call process()
        let resp_ptr = self
            .process
            .call(&mut self.store, in_ptr, input.len() as i32)
            .map_err(|e| WasmHostError::PluginTrap(e.to_string()))?;

        // 4. Read response header: [u32 status][u32 len]
        let hdr = self.read_memory(resp_ptr, 8)?;
        let status = u32::from_le_bytes(hdr[0..4].try_into().unwrap());
        let len = u32::from_le_bytes(hdr[4..8].try_into().unwrap()) as usize;

        // 5. Enforce response size limit
        if len > self.limits.max_response_bytes {
            return Err(WasmHostError::ResponseTooLarge(len).into());
        }

        // 6. Read payload
        let payload = self.read_memory(resp_ptr + 8, len)?;

        // 7. Deallocate response memory
        if let Some(dealloc) = &self.dealloc {
            let total = (8 + len) as i32;
            let _ = dealloc.call(&mut self.store, resp_ptr, total);
        }

        // 8. Handle response status
        match status {
            0 => Ok(payload),
            1 => Err(WasmHostError::PluginError(String::from_utf8_lossy(&payload).to_string()).into()),
            other => Err(WasmHostError::UnknownStatus(other).into()),
        }
    }

    /// Get a view of plugin memory
    fn mem_view(&self) -> MemoryView<'_> {
        self.memory.view(&self.store)
    }

    /// Write bytes to plugin memory at the given pointer
    ///
    /// # Errors
    /// Returns error if write would exceed memory bounds
    fn write_memory(&mut self, ptr: i32, data: &[u8]) -> Result<()> {
        let view = self.mem_view();
        let start = ptr as u64;
        let end = start + data.len() as u64;

        if end > view.data_size() {
            return Err(WasmHostError::OutOfBounds("write").into());
        }

        for (i, b) in data.iter().enumerate() {
            view.write(start + i as u64, &[*b])?;
        }
        Ok(())
    }

    /// Read bytes from plugin memory at the given pointer
    ///
    /// # Errors
    /// Returns error if read would exceed memory bounds
    fn read_memory(&mut self, ptr: i32, len: usize) -> Result<Vec<u8>> {
        let view = self.mem_view();
        let start = ptr as u64;
        let end = start + len as u64;

        if end > view.data_size() {
            return Err(WasmHostError::OutOfBounds("read").into());
        }

        let mut out = vec![0u8; len];
        view.read(start, &mut out)?;
        Ok(out)
    }
}
