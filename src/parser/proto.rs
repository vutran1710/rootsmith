use anyhow::anyhow;
use anyhow::Context;
use anyhow::Result;
use prost::Message as ProstMessage;

use crate::types::IncomingRecord;

// Include the generated protobuf code
pub mod proto {
    include!(concat!(env!("OUT_DIR"), "/rootsmith.rs"));
}

pub trait RealMessage: From<Vec<u8>> + Into<IncomingRecord> {}

/// Generic wrapper for any protobuf message with a custom conversion function
pub struct GenericProtoMessage<T, F>
where
    T: ProstMessage + Default,
    F: Fn(&T) -> IncomingRecord,
{
    proto_msg: T,
    converter: F,
}

impl<T, F> GenericProtoMessage<T, F>
where
    T: ProstMessage + Default,
    F: Fn(&T) -> IncomingRecord,
{
    /// Create a new GenericProtoMessage with custom converter function
    pub fn new(data: Vec<u8>, converter: F) -> Result<Self> {
        let proto_msg = T::decode(&data[..]).context("Failed to decode protobuf message")?;
        Ok(Self {
            proto_msg,
            converter,
        })
    }

    /// Create from already decoded proto message
    pub fn from_proto(proto_msg: T, converter: F) -> Self {
        Self {
            proto_msg,
            converter,
        }
    }
}

impl<T, F> Into<IncomingRecord> for GenericProtoMessage<T, F>
where
    T: ProstMessage + Default,
    F: Fn(&T) -> IncomingRecord,
{
    fn into(self) -> IncomingRecord {
        (self.converter)(&self.proto_msg)
    }
}

/// Wrapper for protobuf IncomingRecord that implements RealMessage trait
pub struct ProtoIncomingRecord(proto::IncomingRecord);

impl ProtoIncomingRecord {
    /// Default converter for proto::IncomingRecord
    fn default_converter(msg: &proto::IncomingRecord) -> IncomingRecord {
        let namespace: [u8; 32] = msg
            .namespace
            .clone()
            .try_into()
            .expect("namespace must be exactly 32 bytes");

        let key: [u8; 32] = msg
            .key
            .clone()
            .try_into()
            .expect("key must be exactly 32 bytes");

        let value: [u8; 32] = msg
            .value
            .clone()
            .try_into()
            .expect("value must be exactly 32 bytes");

        IncomingRecord {
            namespace,
            key,
            value,
            timestamp: msg.timestamp,
        }
    }
}

impl From<Vec<u8>> for ProtoIncomingRecord {
    fn from(data: Vec<u8>) -> Self {
        let proto_record =
            proto::IncomingRecord::decode(&data[..]).expect("Failed to decode protobuf message");
        ProtoIncomingRecord(proto_record)
    }
}

impl Into<IncomingRecord> for ProtoIncomingRecord {
    fn into(self) -> IncomingRecord {
        Self::default_converter(&self.0)
    }
}

impl RealMessage for ProtoIncomingRecord {}

/// Parse a protobuf-encoded binary message into an IncomingRecord.
///
/// # Arguments
/// * `data` - The protobuf-encoded binary data
///
/// # Returns
/// * `Ok(IncomingRecord)` if the data is valid and has correct field sizes
/// * `Err` if deserialization fails or field sizes are incorrect
///
/// # Example
/// ```ignore
/// let proto_bytes = vec![...]; // protobuf-encoded bytes
/// let record = parse_proto_message(&proto_bytes)?;
/// ```
pub fn parse_proto_message(data: &[u8]) -> Result<IncomingRecord> {
    let proto_record =
        proto::IncomingRecord::decode(data).context("Failed to decode protobuf message")?;

    convert_proto_to_record(proto_record)
}

/// Convert a protobuf IncomingRecord to the internal IncomingRecord type.
///
/// Validates that all byte arrays are exactly 32 bytes in length.
fn convert_proto_to_record(proto_record: proto::IncomingRecord) -> Result<IncomingRecord> {
    // Validate namespace is exactly 32 bytes
    let namespace: [u8; 32] = proto_record
        .namespace
        .try_into()
        .map_err(|v: Vec<u8>| anyhow!("namespace must be exactly 32 bytes, got {}", v.len()))?;

    // Validate key is exactly 32 bytes
    let key: [u8; 32] = proto_record
        .key
        .try_into()
        .map_err(|v: Vec<u8>| anyhow!("key must be exactly 32 bytes, got {}", v.len()))?;

    // Validate value is exactly 32 bytes
    let value: [u8; 32] = proto_record
        .value
        .try_into()
        .map_err(|v: Vec<u8>| anyhow!("value must be exactly 32 bytes, got {}", v.len()))?;

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

    /// Helper to create a valid protobuf IncomingRecord
    fn create_valid_proto_record() -> proto::IncomingRecord {
        proto::IncomingRecord {
            namespace: vec![0u8; 32],
            key: vec![1u8; 32],
            value: vec![2u8; 32],
            timestamp: 1234567890,
        }
    }

    #[test]
    fn test_parse_valid_proto_message() {
        let proto_record = create_valid_proto_record();
        let mut buf = Vec::new();
        proto_record.encode(&mut buf).unwrap();

        let result = parse_proto_message(&buf);
        assert!(result.is_ok());

        let record = result.unwrap();
        assert_eq!(record.namespace, [0u8; 32]);
        assert_eq!(record.key, [1u8; 32]);
        assert_eq!(record.value, [2u8; 32]);
        assert_eq!(record.timestamp, 1234567890);
    }

    #[test]
    fn test_parse_invalid_protobuf() {
        let invalid_data = vec![0xFF, 0xFF, 0xFF];
        let result = parse_proto_message(&invalid_data);
        assert!(result.is_err());
    }

    #[test]
    fn test_convert_invalid_namespace_size() {
        let proto_record = proto::IncomingRecord {
            namespace: vec![0u8; 16], // Wrong size
            key: vec![1u8; 32],
            value: vec![2u8; 32],
            timestamp: 1234567890,
        };

        let result = convert_proto_to_record(proto_record);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("namespace"));
    }

    #[test]
    fn test_convert_invalid_key_size() {
        let proto_record = proto::IncomingRecord {
            namespace: vec![0u8; 32],
            key: vec![1u8; 48], // Wrong size
            value: vec![2u8; 32],
            timestamp: 1234567890,
        };

        let result = convert_proto_to_record(proto_record);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("key"));
    }

    #[test]
    fn test_convert_invalid_value_size() {
        let proto_record = proto::IncomingRecord {
            namespace: vec![0u8; 32],
            key: vec![1u8; 32],
            value: vec![2u8; 8], // Wrong size
            timestamp: 1234567890,
        };

        let result = convert_proto_to_record(proto_record);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("value"));
    }

    #[test]
    fn test_roundtrip_encoding_decoding() {
        let proto_record = create_valid_proto_record();
        let mut buf = Vec::new();
        proto_record.encode(&mut buf).unwrap();

        let decoded = parse_proto_message(&buf).unwrap();

        assert_eq!(decoded.namespace, [0u8; 32]);
        assert_eq!(decoded.key, [1u8; 32]);
        assert_eq!(decoded.value, [2u8; 32]);
        assert_eq!(decoded.timestamp, 1234567890);
    }

    #[test]
    fn test_generic_proto_message_with_custom_converter() {
        let proto_record = create_valid_proto_record();
        let mut buf = Vec::new();
        proto_record.encode(&mut buf).unwrap();

        // Custom converter that fills namespace with 5s
        let custom_converter = |msg: &proto::IncomingRecord| IncomingRecord {
            namespace: [5u8; 32],
            key: msg.key.clone().try_into().expect("key must be 32 bytes"),
            value: msg
                .value
                .clone()
                .try_into()
                .expect("value must be 32 bytes"),
            timestamp: msg.timestamp,
        };

        let generic_msg = GenericProtoMessage::new(buf, custom_converter).unwrap();
        let record: IncomingRecord = generic_msg.into();

        assert_eq!(record.namespace, [5u8; 32]);
        assert_eq!(record.key, [1u8; 32]);
        assert_eq!(record.value, [2u8; 32]);
        assert_eq!(record.timestamp, 1234567890);
    }

    #[test]
    fn test_generic_proto_message_from_proto() {
        let proto_record = create_valid_proto_record();

        // Custom converter that fills all fields with 9s
        let custom_converter = |msg: &proto::IncomingRecord| IncomingRecord {
            namespace: [9u8; 32],
            key: [9u8; 32],
            value: [9u8; 32],
            timestamp: msg.timestamp,
        };

        let generic_msg = GenericProtoMessage::from_proto(proto_record, custom_converter);
        let record: IncomingRecord = generic_msg.into();

        assert_eq!(record.namespace, [9u8; 32]);
        assert_eq!(record.key, [9u8; 32]);
        assert_eq!(record.value, [9u8; 32]);
        assert_eq!(record.timestamp, 1234567890);
    }
}
