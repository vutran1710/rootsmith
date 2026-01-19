#![no_std]

// Include infrastructure (SDK, allocator, protobuf encoder, etc.)
// This is included from the same directory (src/wasm_host/infra.rs)
mod infra {
    include!("infra.rs");
}

use core::slice;
use infra::sdk::{DecodeFromEnvelope, IncomingRecord, ToRecord};
use infra::{encode_protobuf, encode_response, store_string};

// Include user plugin code from tests/plugins/client.rs
include!("../../tests/plugins/client.rs");

// ================= PLUGIN PROCESS FUNCTION =================
#[no_mangle]
pub extern "C" fn process(ptr: *const u8, len: usize) -> *mut u8 {
    if ptr.is_null() {
        return encode_response(1, b"null input pointer");
    }

    let input = unsafe { slice::from_raw_parts(ptr, len) };
    
    // Decode from JSON envelope
    let event = match MyWhateverEventName::decode_from_json(input) {
        Some(e) => e,
        None => return encode_response(1, b"failed to decode JSON"),
    };
    
    // Convert to IncomingRecord
    let record = event.to_incoming_record();
    
    // Encode as protobuf
    let (protobuf_ptr, protobuf_len) = encode_protobuf(&record);
    let protobuf_slice = unsafe { slice::from_raw_parts(protobuf_ptr, protobuf_len) };
    
    encode_response(0, protobuf_slice)
}
