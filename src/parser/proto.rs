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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_valid_proto() {
        let proto_record = proto::IncomingRecord {
            namespace: vec![1u8; 32],
            key: vec![2u8; 32],
            value: vec![3u8; 32],
            timestamp: 1234567890,
        };

        let mut buf = Vec::new();
        proto_record.encode(&mut buf).unwrap();

        let result = parse_proto_message(&buf);
        assert!(result.is_ok());

        let record = result.unwrap();
        assert_eq!(record.namespace, [1u8; 32]);
        assert_eq!(record.key, [2u8; 32]);
        assert_eq!(record.value, [3u8; 32]);
        assert_eq!(record.timestamp, 1234567890);
    }

    #[test]
    fn test_parse_invalid_namespace_length() {
        let proto_record = proto::IncomingRecord {
            namespace: vec![1u8; 16], // Invalid: only 16 bytes
            key: vec![2u8; 32],
            value: vec![3u8; 32],
            timestamp: 1234567890,
        };

        let mut buf = Vec::new();
        proto_record.encode(&mut buf).unwrap();

        let result = parse_proto_message(&buf);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Invalid namespace length"));
    }

    #[test]
    fn test_parse_invalid_key_length() {
        let proto_record = proto::IncomingRecord {
            namespace: vec![1u8; 32],
            key: vec![2u8; 16], // Invalid: only 16 bytes
            value: vec![3u8; 32],
            timestamp: 1234567890,
        };

        let mut buf = Vec::new();
        proto_record.encode(&mut buf).unwrap();

        let result = parse_proto_message(&buf);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Invalid key length"));
    }

    #[test]
    fn test_parse_invalid_value_length() {
        let proto_record = proto::IncomingRecord {
            namespace: vec![1u8; 32],
            key: vec![2u8; 32],
            value: vec![3u8; 16], // Invalid: only 16 bytes
            timestamp: 1234567890,
        };

        let mut buf = Vec::new();
        proto_record.encode(&mut buf).unwrap();

        let result = parse_proto_message(&buf);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Invalid value length"));
    }

    #[test]
    fn test_parse_invalid_protobuf() {
        let invalid_data = b"not a valid protobuf message";
        let result = parse_proto_message(invalid_data);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_empty_data() {
        let result = parse_proto_message(&[]);
        assert!(result.is_err());
    }
}
