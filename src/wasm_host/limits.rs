/// Hard resource limits for WASM plugins
#[derive(Debug, Clone)]
pub struct WasmLimits {
    /// Maximum memory pages (1 page = 64 KiB)
    /// Default: 2048 pages = 128 MiB
    pub max_memory_pages: u32,

    /// Maximum response size in bytes
    /// Default: 16 MiB
    pub max_response_bytes: usize,
}

impl Default for WasmLimits {
    fn default() -> Self {
        Self {
            max_memory_pages: 2048,              // 128 MiB
            max_response_bytes: 16 * 1024 * 1024, // 16 MiB
        }
    }
}

impl WasmLimits {
    /// Create limits with custom values
    pub fn new(max_memory_pages: u32, max_response_bytes: usize) -> Self {
        Self {
            max_memory_pages,
            max_response_bytes,
        }
    }

    /// Strict limits for untrusted plugins
    pub fn strict() -> Self {
        Self {
            max_memory_pages: 1024,         // 64 MiB
            max_response_bytes: 8 * 1024 * 1024, // 8 MiB
        }
    }

    /// Permissive limits for testing
    pub fn permissive() -> Self {
        Self {
            max_memory_pages: 4096,         // 256 MiB
            max_response_bytes: 64 * 1024 * 1024, // 64 MiB
        }
    }
}
