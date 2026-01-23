#![no_std]

// Include infrastructure (SDK, allocator, protobuf encoder, etc.)
// #[macro_use] makes plugin! macro available throughout the crate
#[macro_use]
pub mod infra {
    include!("../../../src/wasm_host/infra.rs");
}

#[macro_use]
extern crate wasm_plugin_sdk_derive;

use core::slice;
use infra::sdk::{DecodeFromEnvelope, ToRecord, ToStandardData, IncomingRecord};
use infra::{encode_protobuf, encode_response};

// Include user plugin code
// The plugin! macro is available at crate root via #[macro_export] from infra
// We need to make it available in this scope for client.rs
mod client {
    use crate::infra::sdk;
    
    // Re-export types needed by client.rs
    pub use crate::infra::sdk::{IncomingRecord, ToRecord, ToStandardData};
    
    // Define SDKTrait locally for plugin-side implementation
    // This matches the SDKTrait definition in src/accumulator/sdk_accumulator.rs
    // When used in host context, the blanket impl will make this work with host SDKTrait
    pub trait SDKTrait {
        fn namespace(&self) -> [u8; 32];
        fn key(&self) -> [u8; 32];
        fn value(&self) -> [u8; 32];
        fn timestamp(&self) -> u64;
    }
    
    // Define plugin macro locally - it will be available when client.rs is included
    // This matches the definition in infra.rs
    macro_rules! plugin {
        (name: $name:expr, api_version: $ver:expr, type Input = $input_type:ty, record_kind: $kind:expr) => {
            #[no_mangle]
            pub extern "C" fn get_api_version() -> u32 {
                $ver << 16
            }
        };
    }
    
    include!("../client.rs");
}

use client::MyWhateverEventName;

// ================= PLUGIN PROCESS FUNCTION =================
#[no_mangle]
pub extern "C" fn process(ptr: *const u8, len: usize) -> *mut u8 {
    if ptr.is_null() {
        return encode_response(1, b"null input pointer");
    }

    let input = unsafe { slice::from_raw_parts(ptr, len) };
    
    // Step 1: Decode from JSON envelope
    let event = match MyWhateverEventName::decode_from_json(input) {
        Some(e) => e,
        None => return encode_response(1, b"failed to decode JSON"),
    };
    
    // Step 2: Extract metadata using ToRecord trait
    let namespace = event.get_namespace();
    let key = event.get_key();
    let timestamp = event.get_timestamp();
    
    // Step 3: Extract data using ToStandardData trait
    let value = event.get_value();
    
    // Step 4: Combine metadata + data into IncomingRecord and encode as protobuf
    let record = IncomingRecord {
        namespace,
        key,
        value,
        timestamp,
    };
    
    let (protobuf_ptr, protobuf_len) = encode_protobuf(&record);
    let protobuf_slice = unsafe { slice::from_raw_parts(protobuf_ptr, protobuf_len) };
    
    encode_response(0, protobuf_slice)
}
