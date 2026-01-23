// ================= USER PLUGIN CODE =================
// This is how a partner would write their plugin

#[derive(DecodeFromEnvelope)]
#[decode(from = "json")]
pub struct MyWhateverEventName {
    pub id: sdk::String,
    pub ts_ms: u64,
    pub user_id: sdk::String,
    pub action: sdk::String,
}

// Step 1: Implement ToRecord for metadata extraction (always required)
impl sdk::ToRecord for MyWhateverEventName {
    fn get_namespace(&self) -> [u8; 32] {
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
        namespace
    }
    
    fn get_key(&self) -> [u8; 32] {
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
        key
    }
    
    fn get_timestamp(&self) -> u64 {
        self.ts_ms / 1000 // Convert ms to seconds
    }
}

// Step 2: Implement one of the data format traits (choose based on needs)
// Option: ToStandardData (for fixed-size 32-byte value, protobuf IncomingRecord)
impl sdk::ToStandardData for MyWhateverEventName {
    fn get_value(&self) -> [u8; 32] {
        // Encode action as fixed-size value
        let mut value = [0u8; 32];
        let action_str = self.action.as_str();
        let action_bytes = action_str.as_bytes();
        for (i, &b) in action_bytes.iter().take(32).enumerate() {
            value[i] = b;
        }
        for i in action_bytes.len().min(32)..32 {
            value[i] = (i * 13) as u8;
        }
        value
    }
}

// Step 3: Implement SDKTrait for direct accumulator integration
// This allows MyWhateverEventName to be passed directly to SdkAccumulator
// Usage example (in host code):
//   let event = MyWhateverEventName { ... };
//   let trait_obj: Box<dyn SDKTrait> = Box::new(event);
//   accumulator.build(trait_obj)?;
//   // or
//   accumulator.commit_trait(&[trait_obj], tx).await?;
//
// Note: Since ToStandardData already provides all required methods via RecordMeta,
// this implementation delegates to those methods. The blanket impl in sdk_accumulator.rs
// would also work, but this explicit impl makes it clear and allows customization if needed.
impl SDKTrait for MyWhateverEventName {
    fn namespace(&self) -> [u8; 32] {
        // Delegate to ToRecord implementation
        <Self as sdk::ToRecord>::get_namespace(self)
    }

    fn key(&self) -> [u8; 32] {
        // Delegate to ToRecord implementation
        <Self as sdk::ToRecord>::get_key(self)
    }

    fn value(&self) -> [u8; 32] {
        // Delegate to ToStandardData implementation
        <Self as sdk::ToStandardData>::get_value(self)
    }

    fn timestamp(&self) -> u64 {
        // Delegate to ToRecord implementation
        <Self as sdk::ToRecord>::get_timestamp(self)
    }
}

// Alternative options (commented out):
// - ToCustomJsonData: for JSON payload output
// - ToExtendedData: for variable-length value with metadata
// - ToRawData: for raw bytes output

plugin! {
    name: "my-plugin",
    api_version: 1,
    type Input = MyWhateverEventName,
    record_kind: "my_event"
}
