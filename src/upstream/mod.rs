pub mod websocket;
pub mod kafka;
pub mod sqs;
pub mod mqtt;
pub mod variant;

pub use variant::{UpstreamVariant, NoopUpstream, MockUpstream};
