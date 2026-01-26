use std::sync::atomic::AtomicUsize;

use anyhow::Result;
use kanal::AsyncReceiver;
use kanal::AsyncSender;

use crate::types::UpstreamData;

pub struct Channel {
    rx: AsyncReceiver<UpstreamData>,
    pub tx: AsyncSender<UpstreamData>,
}

impl Default for Channel {
    fn default() -> Self {
        let (tx, rx) = kanal::unbounded_async::<UpstreamData>();
        Self { rx, tx }
    }
}

impl Channel {
    pub async fn bind_forward_loop(&mut self, tx: AsyncSender<UpstreamData>) -> Result<()> {
        let counter = AtomicUsize::new(0);
        while let Ok(data) = self.rx.recv().await {
            if let Err(e) = tx.send(data).await {
                tracing::error!("Failed to forward data from channel upstream: {}", e);
                break;
            }
            counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let current = counter.load(std::sync::atomic::Ordering::Relaxed);
            if current % 100 == 0 {
                tracing::info!("Forwarded {} records from channel upstream", current);
            }
        }
        Ok(())
    }

    pub async fn close(&mut self) -> Result<()> {
        self.tx.close()?;
        Ok(())
    }
}
