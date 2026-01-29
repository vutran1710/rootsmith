use serde::Deserialize;
use serde::Serialize;

pub type Namespace = [u8; 16];
pub type Key16 = [u8; 16];

/// Data received from upstream connectors.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum UpstreamData {
    Bytes(Vec<u8>),
    Json(serde_json::Value),
    Text(String),
}

impl UpstreamData {
    /// Convert the upstream data to a byte vector, if possible.
    pub fn as_bytes(&self) -> Vec<u8> {
        match self {
            UpstreamData::Bytes(data) => data.clone(),
            UpstreamData::Text(text) => text.as_bytes().to_vec(),
            UpstreamData::Json(json) => serde_json::to_vec(json).unwrap_or_default(),
        }
    }
}

/// Data from connectors, parsed into records.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Record {
    pub namespace: Namespace,
    pub key: Key16,
    pub value: UpstreamData,
    pub timestamp: u64,
    pub metadata: Option<serde_json::Value>,
}

impl Record {
    pub const MAX_ENCODED_BYTES: usize = 4 * 1024 * 1024;

    /// Serialize this record to bytes using postcard.
    pub fn to_postcard_bytes(&self) -> Result<Vec<u8>, postcard::Error> {
        postcard::to_allocvec(self)
    }

    /// Deserialize record from postcard bytes.
    /// Enforces a max size to avoid corrupted/hostile blobs.
    pub fn from_postcard_bytes(bytes: &[u8]) -> Result<Self, postcard::Error> {
        if bytes.len() > Self::MAX_ENCODED_BYTES {
            return Err(postcard::Error::DeserializeBadEncoding);
        }
        postcard::from_bytes(bytes)
    }
}
