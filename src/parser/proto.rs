//! Manual protobuf parser for IncomingRecord
//!
//! Parses the simple protobuf format output by WASM plugins.
//! Format:
//!   Field 1: namespace (bytes, 32 bytes)
//!   Field 2: key (bytes, 32 bytes)
//!   Field 3: value (bytes, 32 bytes)
//!   Field 4: timestamp (varint)

use anyhow::anyhow;
use anyhow::Result;

use crate::types::IncomingRecord;

/// Parse a protobuf-encoded IncomingRecord from bytes.
///
/// Expected wire format:
/// ```text
/// [0x0A][0x20][32 bytes namespace]
/// [0x12][0x20][32 bytes key]
/// [0x1A][0x20][32 bytes value]
/// [0x20][varint timestamp]
/// ```
pub fn parse_proto_message(data: &[u8]) -> Result<IncomingRecord> {
    let mut offset = 0;
    let mut namespace = [0u8; 32];
    let mut key = [0u8; 32];
    let mut value = [0u8; 32];
    let mut timestamp: u64 = 0;

    while offset < data.len() {
        let tag = data[offset];
        offset += 1;

        match tag {
            // Field 1: namespace (wire type 2 = length-delimited)
            0x0A => {
                if offset >= data.len() {
                    return Err(anyhow!("Truncated namespace length"));
                }
                let len = data[offset] as usize;
                offset += 1;
                if len != 32 || offset + 32 > data.len() {
                    return Err(anyhow!("Invalid namespace: expected 32 bytes, got {}", len));
                }
                namespace.copy_from_slice(&data[offset..offset + 32]);
                offset += 32;
            }
            // Field 2: key (wire type 2 = length-delimited)
            0x12 => {
                if offset >= data.len() {
                    return Err(anyhow!("Truncated key length"));
                }
                let len = data[offset] as usize;
                offset += 1;
                if len != 32 || offset + 32 > data.len() {
                    return Err(anyhow!("Invalid key: expected 32 bytes, got {}", len));
                }
                key.copy_from_slice(&data[offset..offset + 32]);
                offset += 32;
            }
            // Field 3: value (wire type 2 = length-delimited)
            0x1A => {
                if offset >= data.len() {
                    return Err(anyhow!("Truncated value length"));
                }
                let len = data[offset] as usize;
                offset += 1;
                if len != 32 || offset + 32 > data.len() {
                    return Err(anyhow!("Invalid value: expected 32 bytes, got {}", len));
                }
                value.copy_from_slice(&data[offset..offset + 32]);
                offset += 32;
            }
            // Field 4: timestamp (wire type 0 = varint)
            0x20 => {
                let (ts, bytes_read) = decode_varint(&data[offset..])?;
                timestamp = ts;
                offset += bytes_read;
            }
            // Unknown field - skip based on wire type
            _ => {
                let wire_type = tag & 0x07;
                match wire_type {
                    0 => {
                        // Varint - skip
                        let (_, bytes_read) = decode_varint(&data[offset..])?;
                        offset += bytes_read;
                    }
                    2 => {
                        // Length-delimited - skip
                        if offset >= data.len() {
                            return Err(anyhow!("Truncated length"));
                        }
                        let len = data[offset] as usize;
                        offset += 1 + len;
                    }
                    _ => {
                        return Err(anyhow!("Unsupported wire type: {}", wire_type));
                    }
                }
            }
        }
    }

    Ok(IncomingRecord {
        namespace,
        key,
        value,
        timestamp,
    })
}

/// Decode a varint from bytes, returning (value, bytes_consumed).
fn decode_varint(data: &[u8]) -> Result<(u64, usize)> {
    let mut result: u64 = 0;
    let mut shift = 0;
    let mut offset = 0;

    loop {
        if offset >= data.len() {
            return Err(anyhow!("Truncated varint"));
        }
        let byte = data[offset];
        offset += 1;

        result |= ((byte & 0x7F) as u64) << shift;
        if byte & 0x80 == 0 {
            break;
        }
        shift += 7;
        if shift > 63 {
            return Err(anyhow!("Varint too large"));
        }
    }

    Ok((result, offset))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_incoming_record() {
        // Construct a valid protobuf message
        let mut data = Vec::new();

        // Field 1: namespace
        data.push(0x0A); // tag
        data.push(32); // length
        data.extend_from_slice(&[1u8; 32]); // namespace bytes

        // Field 2: key
        data.push(0x12); // tag
        data.push(32); // length
        data.extend_from_slice(&[2u8; 32]); // key bytes

        // Field 3: value
        data.push(0x1A); // tag
        data.push(32); // length
        data.extend_from_slice(&[3u8; 32]); // value bytes

        // Field 4: timestamp (varint 1000)
        data.push(0x20); // tag
        data.push(0xE8); // 1000 = 0x3E8 as varint
        data.push(0x07);

        let record = parse_proto_message(&data).unwrap();
        assert_eq!(record.namespace, [1u8; 32]);
        assert_eq!(record.key, [2u8; 32]);
        assert_eq!(record.value, [3u8; 32]);
        assert_eq!(record.timestamp, 1000);
    }
}
