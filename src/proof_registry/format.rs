use anyhow::Result;

use crate::proof_registry::types::ProofNodeJson;
use crate::types::Proof;

pub fn to_0x_hex(bytes: &[u8]) -> String {
    let mut s = String::from("0x");
    s.push_str(&hex::encode(bytes));
    s
}

/// Convert custom Proof type to ProofNodeJson
pub fn proof_to_json(proof: Proof) -> Vec<ProofNodeJson> {
    proof
        .nodes
        .into_iter()
        .map(|node| ProofNodeJson {
            direction: if node.is_left {
                "left".into()
            } else {
                "right".into()
            },
            sibling: format!("0x{}", hex::encode(node.sibling)),
        })
        .collect()
}
