use std::sync::Mutex;

use wasmer::Memory;
use wasmer::Store;

static PLUGIN_MEMORY: Mutex<Option<Memory>> = Mutex::new(None);

pub fn set_plugin_memory(memory: Memory) {
    *PLUGIN_MEMORY.lock().unwrap() = Some(memory);
}

pub fn read_memory_safe(store: &Store, memory: &Memory, ptr: i32, len: i32) -> Option<Vec<u8>> {
    if ptr < 0 || len < 0 || len > 4096 {
        return None;
    }

    let view = memory.view(store);
    let data_len = len as usize;
    let start = ptr as u64;

    if start + data_len as u64 > view.data_size() {
        return None;
    }

    let mut buf = vec![0u8; data_len];
    view.read(start, &mut buf).ok()?;
    Some(buf)
}
