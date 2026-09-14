//! Deterministic online and replayable task segmentation.

use std::collections::BTreeSet;

use chrono::{DateTime, Duration, FixedOffset};
use openhistory_domain::{
    CaptureQuality, EventEnvelope, LifecycleBoundary, SemanticPayload, normalize_event_order,
};
use serde::{Deserialize, Serialize};

/// Version of the deterministic segmentation policy.
pub const SEGMENTATION_POLICY_VERSION: u16 = 1;

/// Tunable deterministic segmentation settings.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SegmentationSettings {
    /// Maximum activity gap inside a task.
    pub idle_boundary: Duration,
    /// Minimum continuity score to keep an event in the current task.
    pub continuity_threshold: f32,
}

impl Default for SegmentationSettings {
    fn default() -> Self {
        Self {
            idle_boundary: Duration::minutes(5),
            continuity_threshold: 0.48,
        }
    }
}

/// A stable projection linked to immutable source events.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskSegment {
    /// Hash derived from ordered event identifiers and policy version.
    pub segment_id: String,
    /// First qualifying event time.
    pub started_at: DateTime<FixedOffset>,
    /// Last qualifying event time.
    pub ended_at: DateTime<FixedOffset>,
    /// Ordered source event identifiers.
    pub event_ids: Vec<String>,
    /// Unique source application labels in stable order.
    pub applications: Vec<String>,
    /// Shared explicit project identity when available.
    pub project_id: Option<String>,
    /// Confidence reduced by incomplete semantic coverage.
    pub confidence: SegmentConfidence,
}

/// Coarse confidence level shown to consumers instead of fabricated context.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SegmentConfidence {
    /// Semantic or enriched evidence is consistent.
    High,
    /// Window-level evidence supports continuity.
    Medium,
    /// Only application-level evidence is available.
    Low,
}

/// Replays events into byte-stable segment projections.
#[must_use]
pub fn segment_events(
    events: &[EventEnvelope],
    settings: SegmentationSettings,
) -> Vec<TaskSegment> {
    let ordered = normalize_event_order(events);

    let mut groups: Vec<Vec<EventEnvelope>> = Vec::new();
    for event in ordered {
        if is_closing_boundary(&event.payload) {
            continue;
        }

        let starts_new = groups.last().is_none_or(|current| {
            current.last().is_some_and(|previous| {
                event
                    .occurred_at
                    .signed_duration_since(previous.occurred_at)
                    > settings.idle_boundary
                    || continuity_score(previous, &event) < settings.continuity_threshold
                    || explicit_project_change(previous, &event)
            })
        });

        if starts_new {
            groups.push(vec![event]);
        } else if let Some(current) = groups.last_mut() {
            current.push(event);
        }
    }

    groups
        .iter()
        .filter_map(|group| project_group(group))
        .collect()
}

/// Calculates continuity using only allowed, explicit evidence.
#[must_use]
pub fn continuity_score(previous: &EventEnvelope, next: &EventEnvelope) -> f32 {
    let seconds = next
        .occurred_at
        .signed_duration_since(previous.occurred_at)
        .num_seconds()
        .unsigned_abs();
    let bounded_seconds = u16::try_from(seconds.min(300)).unwrap_or(300);
    let time_score = 1.0 - (f32::from(bounded_seconds) / 300.0);
    let same_application = (previous.source.application.platform_id.is_some()
        && previous.source.application.platform_id == next.source.application.platform_id)
        || (previous.source.application.display_name.is_some()
            && previous.source.application.display_name == next.source.application.display_name);
    let application_score = if same_application { 0.3 } else { 0.08 };
    let project_score = match (project_id(previous), project_id(next)) {
        (Some(left), Some(right)) if left == right => 0.42,
        (Some(_), Some(_)) => 0.0,
        _ => 0.12,
    };
    let quality_factor = match previous.quality.min(next.quality) {
        CaptureQuality::ApplicationOnly => 0.64,
        CaptureQuality::Window => 0.82,
        CaptureQuality::Semantic | CaptureQuality::Enriched => 1.0,
    };
    (time_score * 0.45 + application_score + project_score).min(1.0) * quality_factor
}

fn explicit_project_change(previous: &EventEnvelope, next: &EventEnvelope) -> bool {
    matches!((project_id(previous), project_id(next)), (Some(left), Some(right)) if left != right)
}

fn project_id(event: &EventEnvelope) -> Option<&str> {
    match &event.payload {
        SemanticPayload::WindowChanged { project_id, .. } => project_id.as_deref(),
        _ => None,
    }
}

fn is_closing_boundary(payload: &SemanticPayload) -> bool {
    matches!(
        payload,
        SemanticPayload::Lifecycle(
            LifecycleBoundary::Idle
                | LifecycleBoundary::Sleep
                | LifecycleBoundary::Lock
                | LifecycleBoundary::Logout
                | LifecycleBoundary::Shutdown
        )
    )
}

fn project_group(events: &[EventEnvelope]) -> Option<TaskSegment> {
    let first = events.first()?;
    let last = events.last()?;
    let mut applications = BTreeSet::new();
    let mut event_ids = Vec::with_capacity(events.len());
    let mut quality = CaptureQuality::Enriched;
    let mut shared_project: Option<String> = None;

    for event in events {
        event_ids.push(event.event_id.to_string());
        quality = quality.min(event.quality);
        if let Some(name) = &event.source.application.display_name {
            applications.insert(name.clone());
        }
        if let Some(project) = project_id(event) {
            match &shared_project {
                None => shared_project = Some(project.to_owned()),
                Some(existing) if existing != project => shared_project = None,
                Some(_) => {}
            }
        }
    }

    let mut hasher = blake3::Hasher::new();
    hasher.update(&SEGMENTATION_POLICY_VERSION.to_le_bytes());
    for id in &event_ids {
        hasher.update(id.as_bytes());
    }

    let confidence = match quality {
        CaptureQuality::ApplicationOnly => SegmentConfidence::Low,
        CaptureQuality::Window => SegmentConfidence::Medium,
        CaptureQuality::Semantic | CaptureQuality::Enriched => SegmentConfidence::High,
    };

    Some(TaskSegment {
        segment_id: hasher.finalize().to_hex()[..20].to_owned(),
        started_at: first.occurred_at,
        ended_at: last.occurred_at,
        event_ids,
        applications: applications.into_iter().collect(),
        project_id: shared_project,
        confidence,
    })
}

#[cfg(test)]
mod tests {
    use chrono::DateTime;
    use openhistory_domain::{
        AdapterKind, ApplicationIdentity, EVENT_SCHEMA_VERSION, PrivacyClassification,
        RedactionFlag, SourceIdentity,
    };
    use uuid::Uuid;

    use super::*;

    fn event(id: u128, minute: u32, application: &str, project: Option<&str>) -> EventEnvelope {
        EventEnvelope {
            event_id: Uuid::from_u128(id),
            schema_version: EVENT_SCHEMA_VERSION,
            occurred_at: DateTime::parse_from_rfc3339(&format!(
                "2026-09-08T16:{minute:02}:00+08:00"
            ))
            .unwrap(),
            monotonic_ticks: u64::from(minute),
            source: SourceIdentity {
                source_id: application.into(),
                adapter: AdapterKind::Synthetic,
                application: ApplicationIdentity {
                    display_name: Some(application.into()),
                    platform_id: Some(application.into()),
                    process_id: None,
                },
            },
            privacy: PrivacyClassification::Allowed,
            quality: CaptureQuality::Semantic,
            redactions: Vec::<RedactionFlag>::new(),
            correlation_id: None,
            payload: SemanticPayload::WindowChanged {
                window_title: None,
                project_id: project.map(str::to_owned),
            },
        }
    }

    #[test]
    fn replay_is_deterministic_and_cross_application() {
        let events = vec![
            event(1, 0, "Editor", Some("open-history")),
            event(2, 1, "Terminal", Some("open-history")),
            event(3, 2, "Browser", Some("open-history")),
        ];
        let first = segment_events(&events, SegmentationSettings::default());
        let second = segment_events(&events, SegmentationSettings::default());
        assert_eq!(first, second);
        assert_eq!(first.len(), 1);
        assert_eq!(first[0].applications, ["Browser", "Editor", "Terminal"]);
    }

    #[test]
    fn project_change_and_idle_gap_split_tasks() {
        let events = vec![
            event(1, 0, "Editor", Some("alpha")),
            event(2, 1, "Editor", Some("beta")),
            event(3, 8, "Editor", Some("beta")),
        ];
        assert_eq!(
            segment_events(&events, SegmentationSettings::default()).len(),
            3
        );
    }

    #[test]
    fn low_quality_reduces_confidence() {
        let mut value = event(1, 0, "Unsupported", None);
        value.quality = CaptureQuality::ApplicationOnly;
        let segment = segment_events(&[value], SegmentationSettings::default());
        assert_eq!(segment[0].confidence, SegmentConfidence::Low);
    }
}
