pub mod merkle_accumulator;
pub mod sdk_accumulator;
pub mod sparse_merkle_accumulator;
pub mod variant;
pub mod zk_accumulator;
pub mod zk_adapter;

pub use sdk_accumulator::{to_sdk_trait, SDKTrait, SdkAccumulator};
pub use zk_accumulator::{ZkAccumulator, ZKTrait};

pub use variant::AccumulatorVariant;
