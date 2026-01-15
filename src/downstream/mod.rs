pub mod blackhole;
pub mod channel;
pub mod mock;
pub mod s3;
pub mod variant;

pub use blackhole::BlackholeDownstream;
pub use channel::ChannelDownstream;
pub use mock::MockDownstream;
pub use s3::S3Downstream;
pub use variant::DownstreamVariant;
