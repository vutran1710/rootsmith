#![no_std]

extern crate alloc;

use alloc::vec::Vec;
use core::slice;

#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

pub use serde;
pub use serde::Deserialize;
pub use serde::Serialize;

pub fn json_bytes<T: Serialize>(obj: &T) -> Vec<u8> {
    let mut buf = Vec::new();
    let mut serializer = serde_json_core::ser::Serializer::new(&mut buf);
    let _ = obj.serialize(&mut serializer);
    buf
}

#[macro_export]
macro_rules! json {
    ({ $($key:ident: $val:expr),* $(,)? }) => {{
        #[derive($crate::serde::Serialize)]
        struct JsonObj {
            $($key: $crate::alloc::string::String),*
        }

        let obj = JsonObj {
            $($key: $crate::alloc::string::ToString::to_string(&$val)),*
        };

        $crate::json_bytes(&obj)
    }};

    ($($key:ident: $val:expr),* $(,)?) => {{
        $crate::json!({ $($key: $val),* })
    }};
}

#[macro_export]
macro_rules! log {
    (level = $lvl:expr, $($arg:tt)*) => {{
        let msg = $crate::alloc::format!($($arg)*);
        $crate::__log_impl($lvl, msg.as_str());
    }};
}

#[macro_export]
macro_rules! info {
    ($($arg:tt)*) => {
        $crate::log!(level = 1, $($arg)*)
    };
}

#[macro_export]
macro_rules! warn {
    ($($arg:tt)*) => {
        $crate::log!(level = 2, $($arg)*)
    };
}

#[macro_export]
macro_rules! error {
    ($($arg:tt)*) => {
        $crate::log!(level = 3, $($arg)*)
    };
}

#[doc(hidden)]
pub fn __log_impl(level: u32, msg: &str) {
    unsafe {
        extern "C" {
            fn __host_log(level: u32, ptr: *const u8, len: usize);
        }
        __host_log(level, msg.as_ptr(), msg.len());
    }
}

pub enum UpstreamData<'a> {
    Bytes(&'a [u8]),
    Json(&'a [u8]),
    Text(&'a [u8]),
}

#[derive(Serialize)]
pub struct Record {
    pub namespace: [u8; 16],
    pub key: [u8; 16],
    pub value: Vec<u8>,
    pub timestamp: u64,
    pub metadata: Option<Vec<u8>>,
}

#[derive(Debug)]
pub enum ParseError {
    MissingField,
    InvalidValue,
    JsonError,
}

pub type Result<T> = core::result::Result<T, ParseError>;

pub fn to_fixed_bytes(s: &str, size: usize) -> [u8; 16] {
    let bytes = s.as_bytes();
    let mut result = [0u8; 16];
    let len = bytes.len().min(size);
    result[..len].copy_from_slice(&bytes[..len]);
    result
}

pub fn from_json<'a, T: Deserialize<'a>>(input: &'a [u8]) -> Result<T> {
    serde_json_core::from_slice(input)
        .map(|(val, _)| val)
        .map_err(|_| ParseError::JsonError)
}

pub fn deserialize_upstream_data(input: &[u8]) -> Result<UpstreamData<'_>> {
    if input.is_empty() {
        return Err(ParseError::InvalidValue);
    }

    let tag = input[0];
    if input.len() < 5 {
        return Err(ParseError::InvalidValue);
    }

    let len = u32::from_le_bytes(
        input[1..5]
            .try_into()
            .map_err(|_| ParseError::InvalidValue)?,
    ) as usize;
    if input.len() < 5 + len {
        return Err(ParseError::InvalidValue);
    }

    let data = &input[5..5 + len];

    match tag {
        0 => Ok(UpstreamData::Bytes(data)),
        1 => Ok(UpstreamData::Json(data)),
        2 => Ok(UpstreamData::Text(data)),
        _ => Err(ParseError::InvalidValue),
    }
}

static mut HEAP_PTR: usize = 64 * 1024;

#[no_mangle]
pub extern "C" fn alloc(size: usize) -> *mut u8 {
    unsafe {
        let ptr = HEAP_PTR;
        HEAP_PTR = HEAP_PTR.checked_add(size).expect("heap overflow");
        ptr as *mut u8
    }
}

// Only define panic handler when building for WASM (no_std environment)
// When std is available, it provides its own panic handler
#[cfg(target_arch = "wasm32")]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}

pub fn to_postcard_bytes(record: &Record) -> Vec<u8> {
    postcard::to_allocvec(record).unwrap_or_default()
}

#[macro_export]
macro_rules! rootsmith_plugin {
    ($decode_fn:expr) => {
        extern "C" {
            fn __host_log(level: u32, ptr: *const u8, len: usize);
        }

        #[no_mangle]
        pub extern "C" fn get_api_version() -> u32 {
            1 << 16
        }

        #[no_mangle]
        pub extern "C" fn process(ptr: *const u8, len: usize) -> *mut u8 {
            use core::slice;

            if ptr.is_null() {
                return $crate::encode_error("null input pointer");
            }

            let input = unsafe { slice::from_raw_parts(ptr, len) };

            let upstream_data = match $crate::deserialize_upstream_data(input) {
                Ok(data) => data,
                Err(_) => return $crate::encode_error("failed to deserialize input"),
            };

            match $decode_fn(upstream_data) {
                Ok(Some(record)) => {
                    let bytes = $crate::to_postcard_bytes(&record);
                    let buf_ptr = $crate::alloc(bytes.len());
                    unsafe {
                        let buf = slice::from_raw_parts_mut(buf_ptr, bytes.len());
                        buf.copy_from_slice(&bytes);
                    }
                    $crate::encode_response(0, buf_ptr, bytes.len())
                }
                Ok(None) => $crate::encode_error("unsupported data type"),
                Err(_) => $crate::encode_error("parse error"),
            }
        }
    };
}

#[doc(hidden)]
pub fn encode_error(msg: &str) -> *mut u8 {
    encode_response(1, msg.as_ptr(), msg.len())
}

#[doc(hidden)]
pub fn encode_response(status: u32, payload_ptr: *const u8, payload_len: usize) -> *mut u8 {
    let total_len = 8 + payload_len;
    let out_ptr = alloc(total_len);

    unsafe {
        let out = slice::from_raw_parts_mut(out_ptr, total_len);
        out[0..4].copy_from_slice(&status.to_le_bytes());
        out[4..8].copy_from_slice(&(payload_len as u32).to_le_bytes());
        out[8..].copy_from_slice(slice::from_raw_parts(payload_ptr, payload_len));
    }

    out_ptr
}
