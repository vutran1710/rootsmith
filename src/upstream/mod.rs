pub mod websocket;
pub mod kafka;
pub mod sqs;
pub mod mqtt;

use anyhow::Result;
use crate::types::Block;

pub trait UpstreamSource {
    fn connect(&mut self) -> Result<()>;
    fn receive_block(&mut self) -> Result<Option<Block>>;
    fn disconnect(&mut self) -> Result<()>;
}
