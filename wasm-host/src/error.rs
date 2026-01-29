use thiserror::Error;

#[derive(Debug, Error)]
pub enum WasmHostError {
    #[error("plugin must export `{0}`")]
    MissingExport(&'static str),

    #[error("out of bounds memory access: {0}")]
    OutOfBounds(&'static str),

    #[error("plugin response too large: {0} bytes")]
    ResponseTooLarge(usize),

    #[error("memory limit exceeded: plugin declares {declared} pages, max allowed is {max}")]
    MemoryLimitExceeded { declared: u32, max: u32 },

    #[error("plugin memory must declare a maximum")]
    NoMemoryMaximum,

    #[error("plugin error: {0}")]
    PluginError(String),

    #[error("unknown plugin status: {0}")]
    UnknownStatus(u32),

    #[error("plugin trapped: {0}")]
    PluginTrap(String),
}
