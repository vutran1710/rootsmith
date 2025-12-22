use anyhow::Result;
use crate::proof_registry::types::ProofNodeJson;

pub fn to_0x_hex(bytes: &[u8]) -> String {
    let mut s = String::from("0x");
    s.push_str(&hex::encode(bytes));
    s
}

/// Convert monotree Proof(Vec<(bool, Vec<u8>)>) -> ProofJson
pub fn proof_to_json(
    proof: Vec<(bool, Vec<u8>)>
) -> Vec<ProofNodeJson> {
    proof.into_iter().map(|(is_left, blob)| {
        ProofNodeJson {
            direction: if is_left { "left".into() } else { "right".into() },
            sibling: format!("0x{}", hex::encode(blob)),
        }
    }).collect()
}