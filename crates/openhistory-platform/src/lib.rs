//! Consent-gated shell for macOS Accessibility and Windows UI Automation adapters.
//!
//! Privileged observers are kept behind target-specific modules and cannot start until both product
//! consent and the operating-system permission probe are positive.

use async_trait::async_trait;
use openhistory_adapters::{ActivityAdapter, AdapterError, EventSender};

/// Explicit collection gate shared by both operating systems.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CollectionGate {
    /// User completed the in-product consent flow.
    pub consented: bool,
    /// Platform permission probe currently succeeds.
    pub platform_permission: bool,
}

impl CollectionGate {
    /// Collection can run only while both controls are positive.
    #[must_use]
    pub const fn is_open(self) -> bool {
        self.consented && self.platform_permission
    }
}

/// Target-selected adapter facade. Native observer implementations replace the fixture pump.
#[derive(Default)]
pub struct PlatformAdapter {
    gate: CollectionGate,
    running: bool,
}

impl PlatformAdapter {
    /// Updates consent and current permission atomically from the core's perspective.
    pub fn set_gate(&mut self, gate: CollectionGate) {
        self.gate = gate;
        if !gate.is_open() {
            self.running = false;
        }
    }

    /// Reports visible adapter health without source content.
    #[must_use]
    pub const fn is_running(&self) -> bool {
        self.running
    }
}

#[async_trait]
impl ActivityAdapter for PlatformAdapter {
    fn id(&self) -> &'static str {
        if cfg!(target_os = "macos") {
            "macos-accessibility"
        } else if cfg!(target_os = "windows") {
            "windows-automation"
        } else {
            "unsupported-platform"
        }
    }

    async fn start(&mut self, _sender: EventSender) -> Result<(), AdapterError> {
        if !self.gate.is_open() {
            self.running = false;
            return Err(AdapterError::PermissionRequired);
        }
        self.running = true;
        Ok(())
    }

    async fn stop(&mut self) -> Result<(), AdapterError> {
        self.running = false;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use openhistory_adapters::bounded_event_channel;

    use super::*;

    #[tokio::test]
    async fn consent_and_permission_are_both_required_and_revocable() {
        let (sender, _receiver) = bounded_event_channel(4);
        let mut adapter = PlatformAdapter::default();
        assert_eq!(
            adapter.start(sender.clone()).await,
            Err(AdapterError::PermissionRequired)
        );
        adapter.set_gate(CollectionGate {
            consented: true,
            platform_permission: true,
        });
        adapter.start(sender).await.unwrap();
        assert!(adapter.is_running());
        adapter.set_gate(CollectionGate {
            consented: true,
            platform_permission: false,
        });
        assert!(!adapter.is_running());
    }
}

