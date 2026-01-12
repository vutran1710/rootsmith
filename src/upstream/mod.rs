pub mod http;
pub mod kafka;
pub mod mock;
pub mod mqtt;
pub mod noop;
pub mod sqs;
pub mod variant;
pub mod websocket;

pub use http::HttpSource;
pub use mock::MockUpstream;
pub use noop::NoopUpstream;
pub use variant::UpstreamVariant;
