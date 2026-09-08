//! Storage contracts and a deterministic in-memory backend for core and UI tests.
//!
//! The production `SQLCipher` backend is introduced behind this contract so no unencrypted fallback
//! can be selected accidentally.

use std::collections::BTreeMap;

use chrono::{DateTime, Duration, FixedOffset};
use openhistory_domain::EventEnvelope;
use openhistory_segmentation::TaskSegment;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Storage operation failure without key or content disclosure.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum StorageError {
    /// Requested encrypted backend is not initialized.
    #[error("encrypted storage is not initialized")]
    NotInitialized,
    /// Transaction was aborted and all changes rolled back.
    #[error("transaction was rolled back")]
    RolledBack,
}
/// Supported raw-event retention settings.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RetentionPolicy {
    /// Delete raw events immediately after segment finalization.
    NoRawHistory,
    /// Retain raw events for a bounded number of hours.
    Hours(u16),
}

impl Default for RetentionPolicy {
    fn default() -> Self {
        Self::Hours(48)
    }
}

/// Deletion report safe to display in the UI.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DeletionReport {
    /// Number of raw events deleted.
    pub raw_events: usize,
    /// Number of derived segments deleted.
    pub segments: usize,
}

/// Minimal repository operations used by the deterministic core.
pub trait HistoryRepository {
    /// Persists an already allowed and minimized event.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError`] when encrypted storage is unavailable or the transaction fails.
    fn insert_event(&mut self, event: EventEnvelope) -> Result<(), StorageError>;
    /// Replaces a derived projection while preserving raw events.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError`] when encrypted storage is unavailable or the transaction fails.
    fn upsert_segment(&mut self, segment: TaskSegment) -> Result<(), StorageError>;
    /// Returns allowed events ordered by stable event key.
    fn events(&self) -> Vec<EventEnvelope>;
    /// Deletes expired raw events and returns counts only.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError`] when cleanup cannot complete transactionally.
    fn sweep_retention(
        &mut self,
        now: DateTime<FixedOffset>,
        policy: RetentionPolicy,
    ) -> Result<DeletionReport, StorageError>;
}

/// In-memory backend for synthetic journeys. It is never used as a production fallback.
#[derive(Default)]
pub struct MemoryHistoryRepository {
    events: BTreeMap<String, EventEnvelope>,
    segments: BTreeMap<String, TaskSegment>,
}

impl HistoryRepository for MemoryHistoryRepository {
    fn insert_event(&mut self, event: EventEnvelope) -> Result<(), StorageError> {
        self.events.insert(event.event_id.to_string(), event);
        Ok(())
    }

    fn upsert_segment(&mut self, segment: TaskSegment) -> Result<(), StorageError> {
        self.segments.insert(segment.segment_id.clone(), segment);
        Ok(())
    }

    fn events(&self) -> Vec<EventEnvelope> {
        let mut values = self.events.values().cloned().collect::<Vec<_>>();
        values.sort_by_key(EventEnvelope::ordering_key);
        values
    }

    fn sweep_retention(
        &mut self,
        now: DateTime<FixedOffset>,
        policy: RetentionPolicy,
    ) -> Result<DeletionReport, StorageError> {
        let cutoff = match policy {
            RetentionPolicy::NoRawHistory => now,
            RetentionPolicy::Hours(hours) => now - Duration::hours(i64::from(hours)),
        };
        let before = self.events.len();
        self.events.retain(|_, event| event.occurred_at >= cutoff);
        Ok(DeletionReport {
            raw_events: before - self.events.len(),
            segments: 0,
        })
    }
}

#[cfg(test)]
mod tests {
    use chrono::DateTime;
    use openhistory_domain::{
        AdapterKind, ApplicationIdentity, CaptureQuality, EventEnvelope, SemanticPayload,
        SourceIdentity,
    };

    use super::*;

    fn event(timestamp: &str, ticks: u64) -> EventEnvelope {
        EventEnvelope::new(
            DateTime::parse_from_rfc3339(timestamp).unwrap(),
            ticks,
            SourceIdentity {
                source_id: "fixture".into(),
                adapter: AdapterKind::Synthetic,
                application: ApplicationIdentity {
                    display_name: Some("Editor".into()),
                    platform_id: None,
                    process_id: None,
                },
            },
            CaptureQuality::Window,
            SemanticPayload::ApplicationActivated,
        )
    }

    #[test]
    fn default_retention_deletes_only_older_raw_events() {
        let mut repository = MemoryHistoryRepository::default();
        repository
            .insert_event(event("2026-09-06T15:59:59+08:00", 1))
            .unwrap();
        repository
            .insert_event(event("2026-09-06T16:00:00+08:00", 2))
            .unwrap();
        let now = DateTime::parse_from_rfc3339("2026-09-08T16:00:00+08:00").unwrap();
        let report = repository
            .sweep_retention(now, RetentionPolicy::default())
            .unwrap();
        assert_eq!(report.raw_events, 1);
        assert_eq!(repository.events().len(), 1);
    }
}
