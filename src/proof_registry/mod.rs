pub mod proof_s3;
pub mod proof_github;
pub mod noop;
pub mod mock;
pub mod variant;

pub use variant::ProofRegistryVariant;
pub use noop::NoopProofRegistry;
pub use mock::MockProofRegistry;
