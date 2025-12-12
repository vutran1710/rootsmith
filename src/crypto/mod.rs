pub mod merkle;
pub mod sparse_merkle;
pub mod variant;

pub use merkle::MerkleTree;
pub use sparse_merkle::SparseMerkleTree;
pub use variant::{AccumulatorVariant, SimpleAccumulator, MockAccumulator};
