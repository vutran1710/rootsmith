use serde_json;

use crate::parser::proto::parse_proto_message;
use crate::types::IncomingRecord;
use crate::wasm_host::traits::{
    RecordMeta, ToCustomJsonData, ToExtendedData, ToRawData, ToStandardData,
};

/// Wrapper for Standard format (protobuf IncomingRecord).
///
/// This wrapper implements `ToStandardData` and can be returned as a trait object.
#[derive(Debug, Clone)]
pub struct StandardWrapper {
    pub namespace: [u8; 32],
    pub key: [u8; 32],
    pub value: [u8; 32],
    pub timestamp: u64,
}

impl RecordMeta for StandardWrapper {
    fn namespace(&self) -> [u8; 32] {
        self.namespace
    }

    fn key(&self) -> [u8; 32] {
        self.key
    }

    fn timestamp(&self) -> u64 {
        self.timestamp
    }
}

impl ToStandardData for StandardWrapper {
    fn value(&self) -> [u8; 32] {
        self.value
    }
}

impl From<IncomingRecord> for StandardWrapper {
    fn from(record: IncomingRecord) -> Self {
        Self {
            namespace: record.namespace,
            key: record.key,
            value: record.value,
            timestamp: record.timestamp,
        }
    }
}

/// Wrapper for Custom JSON format.
///
/// This wrapper implements `ToCustomJsonData` and can be returned as a trait object.
#[derive(Debug, Clone)]
pub struct CustomJsonWrapper {
    pub namespace: [u8; 32],
    pub key: [u8; 32],
    pub timestamp: u64,
    pub payload_json: String,
}

impl RecordMeta for CustomJsonWrapper {
    fn namespace(&self) -> [u8; 32] {
        self.namespace
    }

    fn key(&self) -> [u8; 32] {
        self.key
    }

    fn timestamp(&self) -> u64 {
        self.timestamp
    }
}

impl ToCustomJsonData for CustomJsonWrapper {
    fn payload_json(&self) -> &str {
        &self.payload_json
    }
}

/// Wrapper for Extended format (variable-length value with metadata).
///
/// This wrapper implements `ToExtendedData` and can be returned as a trait object.
#[derive(Debug, Clone)]
pub struct ExtendedWrapper {
    pub namespace: [u8; 32],
    pub key: [u8; 32],
    pub timestamp: u64,
    pub value: Vec<u8>,
    pub metadata_json: Option<String>,
}

impl RecordMeta for ExtendedWrapper {
    fn namespace(&self) -> [u8; 32] {
        self.namespace
    }

    fn key(&self) -> [u8; 32] {
        self.key
    }

    fn timestamp(&self) -> u64 {
        self.timestamp
    }
}

impl ToExtendedData for ExtendedWrapper {
    fn value(&self) -> &[u8] {
        &self.value
    }

    fn metadata_json(&self) -> Option<&str> {
        self.metadata_json.as_deref()
    }
}

/// Wrapper for Raw format (minimal structure).
///
/// This wrapper implements `ToRawData` and can be returned as a trait object.
#[derive(Debug, Clone)]
pub struct RawWrapper {
    pub key: [u8; 32],
    pub value: Vec<u8>,
    pub timestamp: Option<u64>,
}

impl ToRawData for RawWrapper {
    fn key(&self) -> [u8; 32] {
        self.key
    }

    fn raw_bytes(&self) -> &[u8] {
        &self.value
    }

    fn timestamp(&self) -> Option<u64> {
        self.timestamp
    }
}

/// Helper to detect and parse plugin output format from bytes.
///
/// Returns the appropriate wrapper type based on the bytes.
pub fn detect_and_parse(bytes: Vec<u8>) -> Result<PluginOutputWrapper, String> {
    // Try protobuf first (Standard format)
    if let Ok(record) = parse_proto_message(&bytes) {
        return Ok(PluginOutputWrapper::Standard(StandardWrapper::from(record)));
    }

    // Try JSON
    if let Ok(json) = serde_json::from_slice::<serde_json::Value>(&bytes) {
        // If JSON has namespace, key, timestamp fields, try Custom format
        if let (Some(ns_str), Some(k_str), Some(ts)) = (
            json.get("namespace").and_then(|v| v.as_str()),
            json.get("key").and_then(|v| v.as_str()),
            json.get("timestamp").and_then(|v| v.as_u64()),
        ) {
            // Try to parse namespace and key as hex
            if let (Ok(namespace_bytes), Ok(key_bytes)) = (hex::decode(ns_str), hex::decode(k_str)) {
                if namespace_bytes.len() == 32 && key_bytes.len() == 32 {
                    let mut namespace = [0u8; 32];
                    let mut key = [0u8; 32];
                    namespace.copy_from_slice(&namespace_bytes);
                    key.copy_from_slice(&key_bytes);

                    // Extract payload (could be in "payload" field or entire JSON)
                    let final_payload = json.get("payload")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| json.to_string());

                    return Ok(PluginOutputWrapper::CustomJson(CustomJsonWrapper {
                        namespace,
                        key,
                        timestamp: ts,
                        payload_json: final_payload,
                    }));
                }
            }
        }

        // Fallback: treat as Raw with JSON bytes
        return Ok(PluginOutputWrapper::Raw(RawWrapper {
            key: [0u8; 32],
            value: bytes,
            timestamp: None,
        }));
    }

    // Fall back to raw bytes
    Ok(PluginOutputWrapper::Raw(RawWrapper {
        key: [0u8; 32],
        value: bytes,
        timestamp: None,
    }))
}

/// Enum to hold different wrapper types for pattern matching.
///
/// This is used internally by the host; external code receives trait objects.
#[derive(Debug, Clone)]
pub enum PluginOutputWrapper {
    Standard(StandardWrapper),
    CustomJson(CustomJsonWrapper),
    Extended(ExtendedWrapper),
    Raw(RawWrapper),
}

impl PluginOutputWrapper {
    /// Convert to trait object for Standard format.
    pub fn as_standard(&self) -> Option<Box<dyn ToStandardData>> {
        match self {
            PluginOutputWrapper::Standard(w) => Some(Box::new(w.clone())),
            _ => None,
        }
    }

    /// Convert to trait object for Custom JSON format.
    pub fn as_custom_json(&self) -> Option<Box<dyn ToCustomJsonData>> {
        match self {
            PluginOutputWrapper::CustomJson(w) => Some(Box::new(w.clone())),
            _ => None,
        }
    }

    /// Convert to trait object for Extended format.
    pub fn as_extended(&self) -> Option<Box<dyn ToExtendedData>> {
        match self {
            PluginOutputWrapper::Extended(w) => Some(Box::new(w.clone())),
            _ => None,
        }
    }

    /// Convert to trait object for Raw format.
    pub fn as_raw(&self) -> Option<Box<dyn ToRawData>> {
        match self {
            PluginOutputWrapper::Raw(w) => Some(Box::new(w.clone())),
            _ => None,
        }
    }
}
