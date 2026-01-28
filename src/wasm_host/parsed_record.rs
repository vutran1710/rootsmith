use crate::types::Namespace;
use crate::types::Record;
use crate::types::UpstreamData;

pub trait PluginOutput {
    fn get_namespace(&self) -> [u8; 32];
    fn get_key(&self) -> [u8; 32];
    fn get_value(&self) -> [u8; 32];
    fn get_timestamp(&self) -> u64;
}

#[derive(Debug, Clone)]
pub struct ParsedRecord {
    pub namespace: Namespace,
    pub key: [u8; 32],
    pub value: [u8; 32],
    pub timestamp: u64,
}

impl PluginOutput for ParsedRecord {
    fn get_namespace(&self) -> [u8; 32] {
        self.namespace
    }

    fn get_key(&self) -> [u8; 32] {
        self.key
    }

    fn get_value(&self) -> [u8; 32] {
        self.value
    }

    fn get_timestamp(&self) -> u64 {
        self.timestamp
    }
}

impl ParsedRecord {
    pub fn from_protobuf(bytes: &[u8]) -> Option<Self> {
        let mut offset = 0;
        let mut namespace = None;
        let mut key = None;
        let mut value = None;
        let mut timestamp = None;

        while offset < bytes.len() {
            let tag = bytes.get(offset)?;
            offset += 1;

            match tag {
                10 => {
                    let len = *bytes.get(offset)? as usize;
                    offset += 1;
                    if len != 32 || offset + len > bytes.len() {
                        return None;
                    }
                    let mut arr = [0u8; 32];
                    arr.copy_from_slice(&bytes[offset..offset + len]);
                    namespace = Some(arr);
                    offset += len;
                }
                18 => {
                    let len = *bytes.get(offset)? as usize;
                    offset += 1;
                    if len != 32 || offset + len > bytes.len() {
                        return None;
                    }
                    let mut arr = [0u8; 32];
                    arr.copy_from_slice(&bytes[offset..offset + len]);
                    key = Some(arr);
                    offset += len;
                }
                26 => {
                    let len = *bytes.get(offset)? as usize;
                    offset += 1;
                    if len != 32 || offset + len > bytes.len() {
                        return None;
                    }
                    let mut arr = [0u8; 32];
                    arr.copy_from_slice(&bytes[offset..offset + len]);
                    value = Some(arr);
                    offset += len;
                }
                32 => {
                    let (ts, varint_len) = decode_varint(&bytes[offset..]);
                    timestamp = Some(ts);
                    offset += varint_len;
                }
                _ => break,
            }
        }

        Some(Self {
            namespace: namespace?,
            key: key?,
            value: value?,
            timestamp: timestamp?,
        })
    }

    pub fn into_record(self) -> Record {
        Record {
            namespace: self.namespace,
            key: self.key,
            value: UpstreamData::Bytes(self.value.to_vec()),
            timestamp: self.timestamp,
        }
    }

    pub fn namespace_str(&self) -> String {
        self.namespace
            .iter()
            .take_while(|&&b| b != 0 && b.is_ascii_graphic())
            .map(|&b| b as char)
            .collect()
    }

    pub fn key_str(&self) -> String {
        self.key
            .iter()
            .take_while(|&&b| b != 0 && b.is_ascii_graphic())
            .map(|&b| b as char)
            .collect()
    }

    pub fn value_str(&self) -> String {
        self.value
            .iter()
            .take_while(|&&b| b != 0 && b.is_ascii_graphic())
            .map(|&b| b as char)
            .collect()
    }
}

fn decode_varint(bytes: &[u8]) -> (u64, usize) {
    let mut result: u64 = 0;
    let mut shift = 0;
    let mut offset = 0;
    for &byte in bytes {
        result |= ((byte & 0x7F) as u64) << shift;
        offset += 1;
        if byte & 0x80 == 0 {
            break;
        }
        shift += 7;
    }
    (result, offset)
}
