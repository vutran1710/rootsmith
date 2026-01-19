#![no_std]

use core::panic::PanicInfo;
use core::slice;

/// ================= ABI CONSTANTS =================
const STATUS_OK: u32 = 0;
#[allow(dead_code)]
const STATUS_ERR: u32 = 1;

/// ================= PANIC HANDLER =================
/// no_std bắt buộc phải có panic handler.
/// Ở WASM plugin sandbox, ta "trap" bằng loop.
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

/// ================= BUMP ALLOCATOR =================
///
/// Heap bắt đầu ở offset an toàn (tránh đè data/stack).
/// Đây là demo "production-ish": tối thiểu tránh UB do đè vùng nhớ.
///
/// Lưu ý: không có dealloc; host nên reload instance theo batch/epoch nếu cần reclaim.
static mut HEAP_PTR: usize = 64 * 1024; // 64KiB safe offset

#[no_mangle]
pub extern "C" fn alloc(size: usize) -> *mut u8 {
    unsafe {
        // Basic overflow guard
        let ptr = HEAP_PTR;
        HEAP_PTR = HEAP_PTR.checked_add(size).expect("heap overflow");
        ptr as *mut u8
    }
}

/// ================= PLUGIN MACRO =================
///
/// plugin! { api_version: 1 }
/// -> export function __plugin_api_version() returning 1
#[macro_export]
macro_rules! plugin {
    (api_version: $ver:expr) => {
        #[no_mangle]
        pub extern "C" fn __plugin_api_version() -> u32 {
            $ver
        }
    };
}

/// ================= MAIN ENTRY =================
///
/// Input:
///   ptr -> input bytes
///   len -> input length
///
/// Output:
///   ptr -> [status(u32)][len(u32)][payload...]
///
/// Note: Ở demo này, payload = reverse(input)
#[no_mangle]
pub extern "C" fn process(ptr: *const u8, len: usize) -> *mut u8 {
    if ptr.is_null() {
        return encode_response(STATUS_ERR, b"null input ptr");
    }

    let input = unsafe { slice::from_raw_parts(ptr, len) };

    // --- Example transform: reverse bytes ---
    let out_len = input.len();
    let total_len = 8 + out_len;
    let out_ptr = alloc(total_len);

    unsafe {
        let out = slice::from_raw_parts_mut(out_ptr, total_len);

        // status
        out[0..4].copy_from_slice(&STATUS_OK.to_le_bytes());
        // payload length
        out[4..8].copy_from_slice(&(out_len as u32).to_le_bytes());

        // payload (reversed)
        for i in 0..out_len {
            out[8 + i] = input[out_len - 1 - i];
        }
    }

    out_ptr
}

/// ================= RESPONSE ENCODER =================
fn encode_response(status: u32, payload: &[u8]) -> *mut u8 {
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

/// ================= DECLARE API VERSION =================
plugin! {
    api_version: 1
}