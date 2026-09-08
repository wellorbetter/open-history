//! Stable, versioned domain types shared by every OpenHistory adapter and consumer.

use std::cmp::Ordering;

use chrono::{DateTime, FixedOffset};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Schema version emitted by this implementation.
pub const EVENT_SCHEMA_VERSION: u16 = 1;

/// A normalized semantic activity event.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventEnvelope {
    /// Globally unique event identifier.
    pub event_id: Uuid,
    /// Domain schema version.
    pub schema_version: u16,
    /// Device-local wall-clock time with an explicit UTC offset.
    pub occurred_at: DateTime<FixedOffset>,
    /// Monotonic counter from the adapter process.
    pub monotonic_ticks: u64,
    /// Source and application provenance.
    pub source: SourceIdentity,
    /// Privacy classification determined at the capture boundary.
    pub privacy: PrivacyClassification,
    /// Completeness of the semantic data.
    pub quality: CaptureQuality,
    /// Redaction actions already applied.
    pub redactions: Vec<RedactionFlag>,
    /// Correlation identity for imported or browser-enriched activity.
    pub correlation_id: Option<String>,
    /// Typed event body.
    pub payload: SemanticPayload,
}

impl EventEnvelope {
    /// Creates an allowed V1 event while requiring every provenance field explicitly.
    #[must_use]
    pub fn new(
        occurred_at: DateTime<FixedOffset>,
        monotonic_ticks: u64,
        source: SourceIdentity,
        quality: CaptureQuality,
        payload: SemanticPayload,
    ) -> Self {
        Self {
            event_id: Uuid::now_v7(),
            schema_version: EVENT_SCHEMA_VERSION,
            occurred_at,
            monotonic_ticks,
            source,
            privacy: PrivacyClassification::Allowed,
            quality,
            redactions: Vec::new(),
            correlation_id: None,
            payload,
        }
    }

    /// Returns a deterministic key suitable for late-arrival reconciliation.
    #[must_use]
    pub fn ordering_key(&self) -> EventOrderingKey {
        EventOrderingKey {
            monotonic_ticks: self.monotonic_ticks,
            occurred_at: self.occurred_at,
            event_id: self.event_id,
        }
    }
}

/// Device-local adapter and application provenance.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceIdentity {
    /// Stable local identity for the adapter instance.
    pub source_id: String,
    /// Adapter family.
    pub adapter: AdapterKind,
    /// Platform application identity. Optional fields remain absent when unavailable.
    pub application: ApplicationIdentity,
}

/// Known event adapter families.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdapterKind {
    /// macOS Accessibility and workspace notifications.
    MacOsAccessibility,
    /// Windows UI Automation and WinEvent notifications.
    WindowsAutomation,
    /// Separately consented browser extension.
    BrowserExtension,
    /// Versioned external import.
    Import,
    /// Deterministic test adapter.
    Synthetic,
}

/// Application details that can be incomplete without being fabricated.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApplicationIdentity {
    /// Display name when exposed by the platform.
    pub display_name: Option<String>,
    /// Bundle identifier or package identity when exposed.
    pub platform_id: Option<String>,
    /// Process identifier at capture time.
    pub process_id: Option<u32>,
}

/// Data completeness reported by an adapter.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CaptureQuality {
    /// Only application-level identity is known.
    ApplicationOnly,
    /// Application and window identity are known.
    Window,
    /// Accessible control role or action is known.
    Semantic,
    /// Browser adapter supplied a positively identified normal context.
    Enriched,
}

/// Privacy state attached before durable storage.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrivacyClassification {
    /// Event may be persisted under current policy.
    Allowed,
    /// Payload was minimized before persistence.
    Redacted,
    /// Only a non-identifying gap boundary may be retained.
    PrivateGap,
}

/// Redactions already performed at the adapter boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RedactionFlag {
    /// Window title was removed.
    WindowTitle,
    /// Document identity was removed.
    DocumentIdentity,
    /// URL path and query were removed.
    UrlPath,
    /// Accessible text was removed.
    AccessibleText,
}

/// Lifecycle transitions that must close active time attribution.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LifecycleBoundary {
    /// Configured idle threshold was crossed.
    Idle,
    /// Device is sleeping.
    Sleep,
    /// User session was locked.
    Lock,
    /// User logged out.
    Logout,
    /// Application or device is shutting down.
    Shutdown,
    /// Qualifying activity resumed.
    Resume,
}

/// Semantic activity captured without pixels, audio, or raw keystrokes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data", rename_all = "snake_case")]
pub enum SemanticPayload {
    /// Foreground application changed.
    ApplicationActivated,
    /// Active window or document changed.
    WindowChanged {
        /// Minimized window label when allowed.
        window_title: Option<String>,
        /// Stable project or document identity when explicitly exposed.
        project_id: Option<String>,
    },
    /// Accessible semantic action, never raw input characters.
    ControlAction {
        /// Accessibility role such as button, text area, or menu item.
        role: String,
        /// Semantic category such as edit, invoke, select, or command.
        action: String,
        /// Bounded contextual label when policy allows it.
        context: Option<String>,
    },
    /// Browser or application navigation.
    Navigation {
        /// Page or destination title when allowed.
        title: Option<String>,
        /// Normalized URL when supplied by an approved browser adapter.
        url: Option<String>,
        /// Positive private-mode signal from the adapter.
        private_context: bool,
    },
    /// Inactivity or process lifecycle boundary.
    Lifecycle(LifecycleBoundary),
}

/// Stable sort key that survives equal wall-clock timestamps and clock rollback.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventOrderingKey {
    /// Monotonic adapter counter.
    pub monotonic_ticks: u64,
    /// Wall-clock time used after the monotonic counter.
    pub occurred_at: DateTime<FixedOffset>,
    /// UUID tie breaker.
    pub event_id: Uuid,
}

impl Ord for EventOrderingKey {
    fn cmp(&self, other: &Self) -> Ordering {
        self.monotonic_ticks
            .cmp(&other.monotonic_ticks)
            .then_with(|| self.occurred_at.cmp(&other.occurred_at))
            .then_with(|| self.event_id.cmp(&other.event_id))
    }
}

impl PartialOrd for EventOrderingKey {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[cfg(test)]
mod tests {
    use chrono::DateTime;

    use super::*;

    fn source() -> SourceIdentity {
        SourceIdentity {
            source_id: "synthetic-1".into(),
            adapter: AdapterKind::Synthetic,
            application: ApplicationIdentity {
                display_name: Some("Editor".into()),
                platform_id: None,
                process_id: None,
            },
        }
    }

    #[test]
    fn every_v1_payload_round_trips() {
        let at = DateTime::parse_from_rfc3339("2026-09-08T16:50:00+08:00").unwrap();
        let payloads = [
            SemanticPayload::ApplicationActivated,
            SemanticPayload::WindowChanged {
                window_title: Some("OpenHistory".into()),
                project_id: Some("open-history".into()),
            },
            SemanticPayload::ControlAction {
                role: "button".into(),
                action: "invoke".into(),
                context: Some("Run tests".into()),
            },
            SemanticPayload::Navigation {
                title: Some("Tauri documentation".into()),
                url: Some("https://tauri.app/start/".into()),
                private_context: false,
            },
            SemanticPayload::Lifecycle(LifecycleBoundary::Idle),
        ];

        for (index, payload) in payloads.into_iter().enumerate() {
            let event = EventEnvelope::new(
                at,
                u64::try_from(index).unwrap(),
                source(),
                CaptureQuality::Semantic,
                payload,
            );
            let encoded = serde_json::to_string(&event).unwrap();
            let decoded: EventEnvelope = serde_json::from_str(&encoded).unwrap();
            assert_eq!(decoded, event);
            assert_eq!(decoded.schema_version, EVENT_SCHEMA_VERSION);
        }
    }

    #[test]
    fn missing_platform_values_remain_missing() {
        let value = ApplicationIdentity {
            display_name: Some("Unsupported application".into()),
            platform_id: None,
            process_id: None,
        };
        let encoded = serde_json::to_value(&value).unwrap();
        assert!(encoded["platform_id"].is_null());
        assert!(encoded["process_id"].is_null());
    }

    #[test]
    fn monotonic_ticks_win_when_clock_rolls_back() {
        let earlier_wall_clock = DateTime::parse_from_rfc3339("2026-09-08T15:59:59+08:00").unwrap();
        let later_wall_clock = DateTime::parse_from_rfc3339("2026-09-08T16:00:00+08:00").unwrap();
        let first = EventEnvelope::new(
            later_wall_clock,
            40,
            source(),
            CaptureQuality::Window,
            SemanticPayload::ApplicationActivated,
        );
        let second = EventEnvelope::new(
            earlier_wall_clock,
            41,
            source(),
            CaptureQuality::Window,
            SemanticPayload::ApplicationActivated,
        );
        assert!(first.ordering_key() < second.ordering_key());
    }
}
