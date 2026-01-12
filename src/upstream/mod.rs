pub mod http;
pub mod mock;
pub mod noop;
pub mod variant;
pub mod websocket;

pub use http::HttpSource;
pub use mock::MockUpstream;
pub use noop::NoopUpstream;
pub use variant::UpstreamVariant;
