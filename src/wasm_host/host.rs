use anyhow::Result;
use wasmer::imports;
use wasmer::Engine;
use wasmer::Instance;
use wasmer::Memory;
use wasmer::MemoryView;
use wasmer::Module;
use wasmer::Store;
use wasmer::TypedFunction;
use wasmer_compiler_cranelift::Cranelift;

use crate::wasm_host::error::WasmHostError;
use crate::wasm_host::limits::WasmLimits;
use crate::wasm_host::parsed_record::ParsedRecord;
use crate::wasm_host::parsed_record::PluginOutput;

pub struct WasmPluginHost {
    store: Store,
    memory: Memory,
    limits: WasmLimits,
    alloc: TypedFunction<i32, i32>,
    process_fn: TypedFunction<(i32, i32), i32>,
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
    pub fn load(path: &str, limits: Option<WasmLimits>) -> Result<Self> {
        let engine: Engine = Cranelift::default().into();
        let mut store = Store::new(engine);

        let module = Module::from_file(&store, path)?;
        let instance = Instance::new(&mut store, &module, &imports! {})?;

        let memory = instance
            .exports
            .get_memory("memory")
            .map_err(|_| WasmHostError::MissingExport("memory"))?
            .clone();

        let alloc = instance
            .exports
            .get_typed_function::<i32, i32>(&store, "alloc")
            .map_err(|_| WasmHostError::MissingExport("alloc"))?;

        let process_fn = instance
            .exports
            .get_typed_function::<(i32, i32), i32>(&store, "process")
            .map_err(|_| WasmHostError::MissingExport("process"))?;

        let dealloc = instance
            .exports
            .get_typed_function::<(i32, i32), ()>(&store, "dealloc")
            .ok();

        let get_api_version = instance
            .exports
            .get_typed_function::<(), i32>(&store, "get_api_version")
            .ok();

        Self::validate_memory_limits(&memory, &store, &limits.clone().unwrap_or_default())?;

        Ok(Self {
            store,
            memory,
            limits: limits.unwrap_or_default(),
            alloc,
            process_fn,
            dealloc,
            get_api_version,
        })
    }

    fn validate_memory_limits(memory: &Memory, store: &Store, limits: &WasmLimits) -> Result<()> {
        let mem_type = memory.ty(store);
        let max_pages = mem_type.maximum.ok_or(WasmHostError::NoMemoryMaximum)?;

        if max_pages.0 > limits.max_memory_pages {
            return Err(WasmHostError::MemoryLimitExceeded {
                declared: max_pages.0,
                max: limits.max_memory_pages,
            }
            .into());
        }

        Ok(())
    }

    pub fn api_version(&mut self) -> Option<(u16, u16)> {
        let f = self.get_api_version.as_ref()?;
        let v = f.call(&mut self.store).ok()? as u32;
        Some(((v >> 16) as u16, (v & 0xFFFF) as u16))
    }

    pub fn process(&mut self, input: &[u8]) -> Result<Box<dyn PluginOutput>> {
        let bytes = self.process_bytes(input)?;
        let record = ParsedRecord::from_protobuf(&bytes)
            .ok_or_else(|| WasmHostError::PluginError("Failed to parse output".to_string()))?;
        Ok(Box::new(record))
    }

    fn process_bytes(&mut self, input: &[u8]) -> Result<Vec<u8>> {
        let in_ptr = self
            .alloc
            .call(&mut self.store, input.len() as i32)
            .map_err(|e| WasmHostError::PluginTrap(e.to_string()))?;

        self.write_memory(in_ptr, input)?;

        let resp_ptr = self
            .process_fn
            .call(&mut self.store, in_ptr, input.len() as i32)
            .map_err(|e| WasmHostError::PluginTrap(e.to_string()))?;

        let hdr = self.read_memory(resp_ptr, 8)?;
        let status = u32::from_le_bytes(hdr[0..4].try_into().unwrap());
        let len = u32::from_le_bytes(hdr[4..8].try_into().unwrap()) as usize;

        if len > self.limits.max_response_bytes {
            return Err(WasmHostError::ResponseTooLarge(len).into());
        }

        let payload = self.read_memory(resp_ptr + 8, len)?;

        if let Some(dealloc) = &self.dealloc {
            let total = (8 + len) as i32;
            let _ = dealloc.call(&mut self.store, resp_ptr, total);
        }

        match status {
            0 => Ok(payload),
            1 => Err(
                WasmHostError::PluginError(String::from_utf8_lossy(&payload).to_string()).into(),
            ),
            other => Err(WasmHostError::UnknownStatus(other).into()),
        }
    }

    fn mem_view(&self) -> MemoryView<'_> {
        self.memory.view(&self.store)
    }

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
