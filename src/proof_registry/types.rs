use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProofNodeJson {
    pub direction: String, // "left" | "right"
    pub sibling: String,   // 0x...
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InclusionProofJson {
    pub root: String,
    pub key: String,
    pub value_hash: String,
    pub proof: Vec<ProofNodeJson>,
}
