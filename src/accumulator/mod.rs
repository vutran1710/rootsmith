pub mod merkle_accumulator;
pub mod sdk_accumulator;
pub mod sparse_merkle_accumulator;
pub mod variant;

pub use sdk_accumulator::{to_sdk_trait, SDKTrait, SdkAccumulator};

pub use variant::AccumulatorVariant;
