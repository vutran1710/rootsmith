// Infrastructure code for WASM plugins
// This file contains SDK definitions, allocator, protobuf encoding, etc.
// It's included when building client.rs
// Note: #![no_std] is declared in the parent file (client.rs)

// Simplified SDK-like interface for demonstration
// In production, this would come from the actual ingestor_plugin_sdk crate
pub mod sdk {
    // In no_std, we'll use a simple string representation
    pub struct String {
        pub ptr: *const u8,
        pub len: usize,
    }
    
    impl String {
        pub fn as_str(&self) -> &str {
            unsafe { core::str::from_utf8_unchecked(core::slice::from_raw_parts(self.ptr, self.len)) }
        }
    }
    
    pub trait DecodeFromEnvelope {
        fn decode_from_json(input: &[u8]) -> Option<Self> where Self: Sized;
    }
    
    pub trait ToRecord {
        fn to_incoming_record(&self) -> IncomingRecord;
    }
    
    pub struct IncomingRecord {
        pub namespace: [u8; 32],
        pub key: [u8; 32],
        pub value: [u8; 32],
        pub timestamp: u64,
    }
    
    #[macro_export]
    macro_rules! plugin {
        (name: $name:expr, api_version: $ver:expr, type Input = $input_type:ty, record_kind: $kind:expr) => {
            #[no_mangle]
            pub extern "C" fn get_api_version() -> u32 {
                $ver << 16
            }
        };
    }
}

use core::panic::PanicInfo;
use core::slice;
use self::sdk::IncomingRecord;

// ================= PANIC HANDLER =================
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

// ================= BUMP ALLOCATOR =================
static mut HEAP_PTR: usize = 64 * 1024;

#[no_mangle]
pub extern "C" fn alloc(size: usize) -> *mut u8 {
    unsafe {
        let ptr = HEAP_PTR;
        HEAP_PTR = HEAP_PTR.checked_add(size).expect("heap overflow");
        ptr as *mut u8
    }
}

// ================= PROTOBUF ENCODER =================
pub fn encode_protobuf(record: &IncomingRecord) -> (*mut u8, usize) {
    let buf_size = 128;
    let buf_ptr = alloc(buf_size);
    let mut offset = 0;
    
    unsafe {
        let buf = slice::from_raw_parts_mut(buf_ptr, buf_size);
        
        // Field 1: namespace (bytes, tag=10)
        buf[offset] = 10;
        offset += 1;
        buf[offset] = 32;
        offset += 1;
        buf[offset..offset + 32].copy_from_slice(&record.namespace);
        offset += 32;
        
        // Field 2: key (bytes, tag=18)
        buf[offset] = 18;
        offset += 1;
        buf[offset] = 32;
        offset += 1;
        buf[offset..offset + 32].copy_from_slice(&record.key);
        offset += 32;
        
        // Field 3: value (bytes, tag=26)
        buf[offset] = 26;
        offset += 1;
        buf[offset] = 32;
        offset += 1;
        buf[offset..offset + 32].copy_from_slice(&record.value);
        offset += 32;
        
        // Field 4: timestamp (varint, tag=32)
        buf[offset] = 32;
        offset += 1;
        let varint_len = encode_varint(&mut buf[offset..], record.timestamp);
        offset += varint_len;
    }
    
    (buf_ptr, offset)
}

fn encode_varint(buf: &mut [u8], mut value: u64) -> usize {
    let mut offset = 0;
    loop {
        let byte = (value & 0x7F) as u8;
        value >>= 7;
        if value == 0 {
            buf[offset] = byte;
            offset += 1;
            break;
        } else {
            buf[offset] = byte | 0x80;
            offset += 1;
        }
    }
    offset
}

// ================= RESPONSE ENCODER =================
pub fn encode_response(status: u32, payload: &[u8]) -> *mut u8 {
    let total_len = 8 + payload.len();
    let out_ptr = alloc(total_len);

    unsafe {
        let out = slice::from_raw_parts_mut(out_ptr, total_len);
        out[0..4].copy_from_slice(&status.to_le_bytes());
        out[4..8].copy_from_slice(&(payload.len() as u32).to_le_bytes());
        out[8..].copy_from_slice(payload);
    }

    out_ptr
}

// Helper for string storage (used by user code)
pub static mut STRING_BUFFER: [u8; 512] = [0; 512];
pub static mut STRING_OFFSET: usize = 0;

pub fn store_string(s: &str) -> sdk::String {
    unsafe {
        let start = STRING_OFFSET;
        let len = s.len().min(512 - start);
        STRING_BUFFER[start..start + len].copy_from_slice(&s.as_bytes()[..len]);
        STRING_OFFSET = (start + len + 1).min(512);
        sdk::String {
            ptr: STRING_BUFFER.as_ptr().add(start),
            len,
        }
    }
}

// ================= JSON PARSING HELPERS =================
// Helper functions for JSON parsing (used by DecodeFromEnvelope implementations)

/// Extract a string field from JSON
pub fn extract_json_string_field(input: &str, field_name: &str) -> Option<sdk::String> {
    // Find the field pattern: "field_name":
    let mut search_pattern = [0u8; 64];
    search_pattern[0] = b'"';
    let name_bytes = field_name.as_bytes();
    if name_bytes.len() > 60 {
        return None;
    }
    search_pattern[1..1 + name_bytes.len()].copy_from_slice(name_bytes);
    search_pattern[1 + name_bytes.len()] = b'"';
    search_pattern[2 + name_bytes.len()] = b':';
    search_pattern[3 + name_bytes.len()] = b'"';
    
    let pattern_len = 4 + name_bytes.len();
    let pattern = core::str::from_utf8(&search_pattern[..pattern_len]).ok()?;
    let field_start = input.find(pattern)? + pattern_len;
    let field_end = input[field_start..].find('"')?;
    let value = &input[field_start..field_start + field_end];
    Some(store_string(value))
}

/// Extract a u64 field from JSON
pub fn extract_json_u64_field(input: &str, field_name: &str) -> Option<u64> {
    // Find the field pattern: "field_name":
    let mut search_pattern = [0u8; 64];
    search_pattern[0] = b'"';
    let name_bytes = field_name.as_bytes();
    if name_bytes.len() > 60 {
        return None;
    }
    search_pattern[1..1 + name_bytes.len()].copy_from_slice(name_bytes);
    search_pattern[1 + name_bytes.len()] = b'"';
    search_pattern[2 + name_bytes.len()] = b':';
    
    let pattern_len = 3 + name_bytes.len();
    let pattern = core::str::from_utf8(&search_pattern[..pattern_len]).ok()?;
    let value_start = input.find(pattern)? + pattern_len;
    // Find the end of the number (comma, }, or whitespace)
    let value_end = input[value_start..].find(|c: char| !c.is_ascii_digit()).unwrap_or(input[value_start..].len());
    input[value_start..value_start + value_end].parse().ok()
}
