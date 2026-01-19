// ================= USER PLUGIN CODE =================
// This is how a user would write their plugin

#[derive(DecodeFromEnvelope)]
#[decode(from = "json")]
pub struct MyWhateverEventName {
    pub id: sdk::String,
    pub ts_ms: u64,
    pub user_id: sdk::String,
    pub action: sdk::String,
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
