pub mod merkle_accumulator;
pub mod sparse_merkle_accumulator;
pub mod variant;
pub mod zk_accumulator;
pub mod zk_adapter;

pub use variant::AccumulatorVariant;
pub use zk_accumulator::ZKTrait;
pub use zk_accumulator::ZkAccumulator;
