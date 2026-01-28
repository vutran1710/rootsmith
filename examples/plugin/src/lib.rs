#![no_std]

extern crate alloc;

use alloc::format;
use alloc::string::String;

use rootsmith_plugin_sdk::*;

#[derive(Deserialize)]
struct UserEvent {
    user_id: String,
    event_type: String,
    timestamp: u64,
    data: Option<String>,
}

pub fn decode(input: UpstreamData) -> Result<Option<Record>> {
    match input {
        UpstreamData::Json(bytes) => {
            let event: UserEvent = from_json(bytes)?;

            let namespace = format!("user_events_{}", event.user_id);
            let key = format!("{}_{}", event.event_type, event.timestamp);
            let value = event.data.unwrap_or_default();

            Ok(Some(Record {
                namespace: to_fixed_bytes(&namespace, 16),
                key: to_fixed_bytes(&key, 16),
                value: value.into_bytes(),
                timestamp: event.timestamp,
                metadata: None,
            }))
        }
        _ => Ok(None),
    }
}

rootsmith_plugin!(decode);
