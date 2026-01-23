use anyhow::{anyhow, Context, Result};
use prost::Message as ProstMessage;

use crate::types::IncomingRecord;

/// Generated protobuf types
pub mod proto {
    include!(concat!(env!("OUT_DIR"), "/rootsmith.rs"));
}

/// Parse a protobuf-encoded IncomingRecord from bytes
pub fn parse_proto_message(data: &[u8]) -> Result<IncomingRecord> {
    let proto_record = proto::IncomingRecord::decode(data)
        .context("Failed to decode protobuf IncomingRecord")?;

    // Validate field lengths
    if proto_record.namespace.len() != 32 {
        return Err(anyhow!(
            "Invalid namespace length: expected 32 bytes, got {}",
            proto_record.namespace.len()
        ));
    }
    if proto_record.key.len() != 32 {
        return Err(anyhow!(
            "Invalid key length: expected 32 bytes, got {}",
            proto_record.key.len()
        ));
    }
    if proto_record.value.len() != 32 {
        return Err(anyhow!(
            "Invalid value length: expected 32 bytes, got {}",
            proto_record.value.len()
        ));
    }

    // Convert to fixed-size arrays
    let mut namespace = [0u8; 32];
    let mut key = [0u8; 32];
    let mut value = [0u8; 32];

    namespace.copy_from_slice(&proto_record.namespace);
    key.copy_from_slice(&proto_record.key);
    value.copy_from_slice(&proto_record.value);

    Ok(IncomingRecord {
        namespace,
        key,
        value,
        timestamp: proto_record.timestamp,
    })
}
