#![no_std]

extern crate alloc;
use alloc::string::String;
use alloc::vec::Vec;
use core::slice;

pub mod sdk {
    use alloc::string::String;
    use alloc::vec::Vec;

    pub enum UpstreamData<'a> {
        Bytes(&'a [u8]),
        Json(&'a [u8]),
        Text(&'a [u8]),
    }

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

    pub fn to_fixed_bytes(s: &str, size: usize) -> Vec<u8> {
        let bytes = s.as_bytes();
        let mut result = vec![0u8; size];
        let len = bytes.len().min(size);
        result[..len].copy_from_slice(&bytes[..len]);
        result
    }
}

use sdk::ParseError;
use sdk::Record;
use sdk::Result;
use sdk::UpstreamData;

fn deserialize_upstream_data(input: &[u8]) -> Result<UpstreamData> {
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

use serde::Deserialize;

#[derive(Deserialize)]
struct JsonData {
    namespace: String,
    key: String,
    value: String,
    timestamp: Option<u64>,
}

pub fn decode(input: UpstreamData) -> Result<Option<Record>> {
    match input {
        UpstreamData::Json(bytes) => {
            let data: JsonData =
                serde_json_core::from_slice(bytes).map_err(|_| ParseError::JsonError)?;

            let mut namespace = [0u8; 16];
            let namespace_bytes = sdk::to_fixed_bytes(&data.namespace, 16);
            namespace.copy_from_slice(&namespace_bytes);

            let mut key = [0u8; 16];
            let key_bytes = sdk::to_fixed_bytes(&data.key, 16);
            key.copy_from_slice(&key_bytes);

            let value = data.value.into_bytes();
            let timestamp = data.timestamp.unwrap_or(0);

            Ok(Some(Record {
                namespace,
                key,
                value,
                timestamp,
            }))
        }
        _ => Ok(None),
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

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}

#[no_mangle]
pub extern "C" fn process(ptr: *const u8, len: usize) -> *mut u8 {
    if ptr.is_null() {
        return encode_error("null input pointer");
    }

    let input = unsafe { slice::from_raw_parts(ptr, len) };

    let upstream_data = match deserialize_upstream_data(input) {
        Ok(data) => data,
        Err(_) => return encode_error("failed to deserialize input"),
    };

    match decode(upstream_data) {
        Ok(Some(record)) => encode_record(&record),
        Ok(None) => encode_error("unsupported data type"),
        Err(_) => encode_error("parse error"),
    }
}

fn encode_record(record: &Record) -> *mut u8 {
    let total_len = 16 + 16 + 4 + record.value.len() + 8;

    let buf_ptr = alloc(total_len);
    unsafe {
        let buf = slice::from_raw_parts_mut(buf_ptr, total_len);

        let mut offset = 0;
        buf[offset..offset + 16].copy_from_slice(&record.namespace);
        offset += 16;

        buf[offset..offset + 16].copy_from_slice(&record.key);
        offset += 16;

        buf[offset..offset + 4].copy_from_slice(&(record.value.len() as u32).to_le_bytes());
        offset += 4;

        buf[offset..offset + record.value.len()].copy_from_slice(&record.value);
        offset += record.value.len();

        buf[offset..offset + 8].copy_from_slice(&record.timestamp.to_le_bytes());
    }

    encode_response(0, buf_ptr, total_len)
}

fn encode_error(msg: &str) -> *mut u8 {
    encode_response(1, msg.as_ptr(), msg.len())
}

fn encode_response(status: u32, payload_ptr: *const u8, payload_len: usize) -> *mut u8 {
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
