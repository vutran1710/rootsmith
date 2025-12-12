pub mod proof_s3;
pub mod proof_github;
pub mod variant;

pub use variant::{ProofRegistryVariant, NoopProofRegistry, MockProofRegistry};
