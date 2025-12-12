pub mod websocket;
pub mod kafka;
pub mod sqs;
pub mod mqtt;
pub mod noop;
pub mod mock;
pub mod variant;

pub use variant::UpstreamVariant;
pub use noop::NoopUpstream;
pub use mock::MockUpstream;
