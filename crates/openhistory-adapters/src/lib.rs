//! Platform-neutral adapter lifecycle and bounded event channel.

use async_trait::async_trait;
use openhistory_domain::EventEnvelope;
use thiserror::Error;
use tokio::sync::mpsc;

/// Adapter failures are visible and recoverable without stopping the core.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum AdapterError {
    /// Required consent or operating-system permission is absent.
    #[error("permission is required")]
    PermissionRequired,
    /// Platform semantic information is temporarily unavailable.
    #[error("adapter is temporarily unavailable")]
    Unavailable,
    /// Bounded channel is no longer accepting events.
    #[error("event channel is closed")]
    ChannelClosed,
}

/// Event sender with a fixed capacity to prevent unbounded memory growth.
pub type EventSender = mpsc::Sender<Result<EventEnvelope, AdapterError>>;

/// Event receiver used by the normalization core.
pub type EventReceiver = mpsc::Receiver<Result<EventEnvelope, AdapterError>>;

/// Creates the default bounded adapter channel.
#[must_use]
pub fn bounded_event_channel(capacity: usize) -> (EventSender, EventReceiver) {
    mpsc::channel(capacity.max(1))
}

/// Common lifecycle implemented by platform and synthetic adapters.
#[async_trait]
pub trait ActivityAdapter: Send {
    /// Stable adapter identity for diagnostics.
    fn id(&self) -> &'static str;

    /// Starts collection into the bounded channel.
    async fn start(&mut self, sender: EventSender) -> Result<(), AdapterError>;

    /// Stops collection promptly and releases observers.
    async fn stop(&mut self) -> Result<(), AdapterError>;
}

/// Deterministic adapter used by fixture and recovery tests.
pub struct FakeAdapter {
    events: Vec<EventEnvelope>,
    fault: Option<AdapterError>,
    running: bool,
}

impl FakeAdapter {
    /// Creates an adapter that replays events in order.
    #[must_use]
    pub fn new(events: Vec<EventEnvelope>) -> Self {
        Self {
            events,
            fault: None,
            running: false,
        }
    }

    /// Configures one visible fault after event replay.
    #[must_use]
    pub fn with_fault(mut self, fault: AdapterError) -> Self {
        self.fault = Some(fault);
        self
    }

    /// Reports whether the adapter is collecting.
    #[must_use]
    pub const fn is_running(&self) -> bool {
        self.running
    }
}

#[async_trait]
impl ActivityAdapter for FakeAdapter {
    fn id(&self) -> &'static str {
        "synthetic"
    }

    async fn start(&mut self, sender: EventSender) -> Result<(), AdapterError> {
        self.running = true;
        for event in self.events.iter().cloned() {
            sender
                .send(Ok(event))
                .await
                .map_err(|_| AdapterError::ChannelClosed)?;
        }
        if let Some(error) = self.fault.clone() {
            sender
                .send(Err(error))
                .await
                .map_err(|_| AdapterError::ChannelClosed)?;
        }
        Ok(())
    }

    async fn stop(&mut self) -> Result<(), AdapterError> {
        self.running = false;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn adapter_fault_does_not_close_receiver() {
        let (sender, mut receiver) = bounded_event_channel(2);
        let mut adapter = FakeAdapter::new(Vec::new()).with_fault(AdapterError::Unavailable);
        adapter.start(sender.clone()).await.unwrap();
        assert_eq!(receiver.recv().await.unwrap(), Err(AdapterError::Unavailable));
        assert!(adapter.is_running());
        adapter.stop().await.unwrap();
        assert!(!adapter.is_running());

        let mut restarted = FakeAdapter::new(Vec::new());
        restarted.start(sender).await.unwrap();
        assert!(restarted.is_running());
    }
}

