pub mod mock;
pub mod noop;
pub mod proof_github;
pub mod proof_s3;
pub mod variant;

pub use mock::MockProofRegistry;
pub use noop::NoopProofRegistry;
pub use variant::ProofRegistryVariant;
