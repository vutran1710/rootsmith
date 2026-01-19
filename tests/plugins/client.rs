// ================= USER PLUGIN CODE =================
// This is how a user would write their plugin

struct MyWhateverEventName {
    id: infra::sdk::String,
    ts_ms: u64,
    user_id: infra::sdk::String,
    action: infra::sdk::String,
}

// In production, #[derive(DecodeFromEnvelope)] would generate this
impl DecodeFromEnvelope for MyWhateverEventName {
    fn decode_from_json(input: &[u8]) -> Option<Self> {
        unsafe { infra::STRING_OFFSET = 0; } // Reset buffer
        
        let input_str = core::str::from_utf8(input).ok()?;
        
        // Extract id
        let id_start = input_str.find(r#""id":"#)? + 5;
        let id_end = input_str[id_start..].find('"')?;
        let id_str = &input_str[id_start..id_start + id_end];
        let id = store_string(id_str);
        
        // Extract ts_ms
        let ts_start = input_str.find(r#""ts_ms":"#)? + 8;
        let ts_end = input_str[ts_start..].find(|c: char| !c.is_ascii_digit())?;
        let ts_ms = input_str[ts_start..ts_start + ts_end].parse().ok()?;
        
        // Extract user_id
        let user_id_start = input_str.find(r#""user_id":"#)? + 10;
        let user_id_end = input_str[user_id_start..].find('"')?;
        let user_id_str = &input_str[user_id_start..user_id_start + user_id_end];
        let user_id = store_string(user_id_str);
        
        // Extract action
        let action_start = input_str.find(r#""action":"#)? + 9;
        let action_end = input_str[action_start..].find('"')?;
        let action_str = &input_str[action_start..action_start + action_end];
        let action = store_string(action_str);
        
        Some(MyWhateverEventName {
            id,
            ts_ms,
            user_id,
            action,
        })
    }
}

impl ToRecord for MyWhateverEventName {
    fn to_incoming_record(&self) -> IncomingRecord {
        // Convert user_id to namespace (hash-like, simplified)
        let mut namespace = [0u8; 32];
        let user_id_str = self.user_id.as_str();
        let user_id_bytes = user_id_str.as_bytes();
        for (i, &b) in user_id_bytes.iter().take(32).enumerate() {
            namespace[i] = b;
        }
        // Fill rest with hash-like pattern
        for i in user_id_bytes.len().min(32)..32 {
            namespace[i] = (i * 7) as u8;
        }
        
        // Use id as key
        let mut key = [0u8; 32];
        let id_str = self.id.as_str();
        let id_bytes = id_str.as_bytes();
        for (i, &b) in id_bytes.iter().take(32).enumerate() {
            key[i] = b;
        }
        for i in id_bytes.len().min(32)..32 {
            key[i] = (i * 11) as u8;
        }
        
        // Encode action as value
        let mut value = [0u8; 32];
        let action_str = self.action.as_str();
        let action_bytes = action_str.as_bytes();
        for (i, &b) in action_bytes.iter().take(32).enumerate() {
            value[i] = b;
        }
        for i in action_bytes.len().min(32)..32 {
            value[i] = (i * 13) as u8;
        }
        
        IncomingRecord {
            namespace,
            key,
            value,
            timestamp: self.ts_ms / 1000, // Convert ms to seconds
        }
    }
}

plugin! {
    name: "my-plugin",
    api_version: 1,
    type Input = MyWhateverEventName,
    record_kind: "my_event"
}
