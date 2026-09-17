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
    /// What was open, with the attention each held, longest first. This is the only part of a
    /// segment that says what was being done rather than merely where: an application list answers
    /// "Ghostty" where a title answers which file, branch, or page. Empty when capture detail never
    /// reached window level, and defaulted so segments persisted before titles were carried still
    /// deserialize.
    #[serde(default)]
    pub titles: Vec<SegmentTitle>,
    /// Shared explicit project identity when available.
    pub project_id: Option<String>,
    /// Confidence reduced by incomplete semantic coverage.
    pub confidence: SegmentConfidence,
    /// Attention credited to this segment: the sum of how long each of its observations survived
    /// before the next one was recorded. Defaulted so segments persisted before attention was
    /// measured still deserialize.
    #[serde(default)]
    pub observed_seconds: i64,
    /// Set when this segment holds the newest observation on record, so its final dwell has no
    /// measured end yet. A consumer that knows the current wall clock may extend it; one that does
    /// not must leave it alone rather than invent an ending.
    #[serde(default)]
    pub open_ended: bool,
}

/// A window title and how long it held attention.
///
/// The seconds travel with the title because a merged view has to be able to rank titles it did not
/// measure itself; ordering alone cannot be summed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SegmentTitle {
    /// The window title as recorded, already bounded in length by capture.
    pub title: String,
    /// Attention credited to this title inside the segment.
    pub observed_seconds: i64,
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
    let credited = credited_instants(&ordered);

    let mut groups: Vec<Group> = Vec::new();
    let mut previous_index: Option<usize> = None;
    for (index, event) in ordered.iter().enumerate() {
        // Lifecycle markers report presence, not activity. They bound the observations around them
        // (see `dwell_seconds`) without ever becoming activity of their own.
        if is_boundary(&event.payload) {
            continue;
        }

        let broken_by_boundary = closed_since_previous_observation(&ordered, index);
        let starts_new = broken_by_boundary
            || previous_index.is_none_or(|previous| {
                credited[index].signed_duration_since(credited[previous]) > settings.idle_boundary
                    || continuity_score(&ordered[previous], event) < settings.continuity_threshold
                    || explicit_project_change(&ordered[previous], event)
            });

        let dwell = dwell_seconds(&ordered, &credited, index);
        if starts_new {
            groups.push(Group {
                events: vec![event.clone()],
                dwells: vec![dwell.unwrap_or(0)],
                observed_seconds: dwell.unwrap_or(0),
                open_ended: dwell.is_none(),
            });
        } else if let Some(current) = groups.last_mut() {
            current.events.push(event.clone());
            current.dwells.push(dwell.unwrap_or(0));
            current.observed_seconds += dwell.unwrap_or(0);
            current.open_ended = dwell.is_none();
        }
        previous_index = Some(index);
    }

    groups.iter().filter_map(Group::project).collect()
}

/// The instant each event in an ordered sequence is credited to, never earlier than one already
/// passed.
///
/// `normalize_event_order` clamps a timestamp that moves backwards so the sequence stays ordered,
/// and leaves the original timestamp on the event. Measuring an interval from the original then
/// measures the distance between two points the sequence itself disagrees about: a restarted tick
/// counter reads as a jump of several hours, and an observation that held for seconds is credited
/// with all of them. Attention is measured along the order, so the order supplies the clock.
fn credited_instants(ordered: &[EventEnvelope]) -> Vec<DateTime<FixedOffset>> {
    let mut latest: Option<DateTime<FixedOffset>> = None;
    ordered
        .iter()
        .map(|event| {
            let credited = latest.map_or(event.occurred_at, |passed| {
                if event.occurred_at > passed {
                    event.occurred_at
                } else {
                    passed
                }
            });
            latest = Some(credited);
            credited
        })
        .collect()
}

/// Events accumulated for one segment, alongside the attention measured for them.
struct Group {
    events: Vec<EventEnvelope>,
    /// Attention measured for each event, positionally aligned with `events`.
    dwells: Vec<i64>,
    observed_seconds: i64,
    open_ended: bool,
}

impl Group {
    fn project(&self) -> Option<TaskSegment> {
        project_group(
            &self.events,
            &self.dwells,
            self.observed_seconds,
            self.open_ended,
        )
    }
}

/// How long the observation at `index` is credited with, or `None` when nothing has been observed
/// since — an open end the caller has to resolve against the current clock.
///
/// Events are only recorded when something changes, so the interval until the next observation is
/// exactly how long the observed state held. That interval is measured, not inferred: forty minutes
/// of watching a video produce one event and forty credited minutes, where a "no events means
/// nobody is here" timeout would have discarded most of it.
///
/// What the interval must never do is span time nobody was watching. A closing boundary ends it at
/// the boundary, and a `Resume` marker voids it entirely: resuming means collection had stopped, and
/// time that was never observed is not evidence of attention. It is measured between `credited`
/// instants rather than raw timestamps for the same reason — see `credited_instants`.
fn dwell_seconds(
    ordered: &[EventEnvelope],
    credited: &[DateTime<FixedOffset>],
    index: usize,
) -> Option<i64> {
    let current = *credited.get(index)?;
    let next = ordered.get(index + 1)?;
    if matches!(
        next.payload,
        SemanticPayload::Lifecycle(LifecycleBoundary::Resume)
    ) {
        return Some(0);
    }
    Some(
        credited
            .get(index + 1)?
            .signed_duration_since(current)
            .num_seconds()
            .max(0),
    )
}

/// Whether collection closed between the previous observation and the one at `index`. A segment
/// that spanned a lock or a sleep would claim one uninterrupted stretch of attention across a gap
/// where there was demonstrably none.
fn closed_since_previous_observation(ordered: &[EventEnvelope], index: usize) -> bool {
    ordered[..index]
        .iter()
        .rev()
        .take_while(|event| is_boundary(&event.payload))
        .any(|event| is_closing_boundary(&event.payload))
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
        // Neither side resolved a project entity (no opted-in repository matched, or the
        // application has no known title convention): fall back to comparing window titles. This
        // is a weaker signal than confirmed project identity, so it scores below the matched case
        // even when titles agree exactly.
        (None, None) => match (window_title(previous), window_title(next)) {
            (Some(left), Some(right)) if left == right => 0.25,
            _ => 0.12,
        },
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

fn window_title(event: &EventEnvelope) -> Option<&str> {
    match &event.payload {
        SemanticPayload::WindowChanged { window_title, .. } => window_title.as_deref(),
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

/// Any presence marker, closing or resuming. Neither kind is activity, so neither belongs inside a
/// segment's evidence.
fn is_boundary(payload: &SemanticPayload) -> bool {
    matches!(payload, SemanticPayload::Lifecycle(_))
}

/// How many titles a segment reports. A window someone kept returning to all afternoon is worth
/// naming; the twentieth thing they glanced at is noise, and every one of them costs the reader's
/// attention to skip.
const MAX_TITLES: usize = 5;

/// The titles a segment's events recorded, ranked by the attention each held.
///
/// Titles are summed rather than listed in order of appearance, because the same window is usually
/// returned to several times and the total is what says whether it mattered. Ties keep the order
/// they were first seen in, so replaying the same events always produces the same segment.
fn ranked_titles(events: &[EventEnvelope], dwells: &[i64]) -> Vec<SegmentTitle> {
    let mut totals: Vec<(usize, String, i64)> = Vec::new();
    for (index, event) in events.iter().enumerate() {
        let SemanticPayload::WindowChanged {
            window_title: Some(title),
            ..
        } = &event.payload
        else {
            continue;
        };
        let held = dwells.get(index).copied().unwrap_or(0);
        if let Some(existing) = totals.iter_mut().find(|(_, seen, _)| seen == title) {
            existing.2 += held;
        } else {
            totals.push((totals.len(), title.clone(), held));
        }
    }
    totals.sort_by_key(|(first_seen, _, held)| (-*held, *first_seen));
    totals
        .into_iter()
        .take(MAX_TITLES)
        .map(|(_, title, observed_seconds)| SegmentTitle {
            title,
            observed_seconds,
        })
        .collect()
}

fn project_group(
    events: &[EventEnvelope],
    dwells: &[i64],
    observed_seconds: i64,
    open_ended: bool,
) -> Option<TaskSegment> {
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
        titles: ranked_titles(events, dwells),
        project_id: shared_project,
        confidence,
        observed_seconds,
        open_ended,
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

    fn marker(id: u128, minute: u32, boundary: LifecycleBoundary) -> EventEnvelope {
        let mut value = event(id, minute, "collector", None);
        value.payload = SemanticPayload::Lifecycle(boundary);
        value.source.application = ApplicationIdentity {
            display_name: None,
            platform_id: None,
            process_id: None,
        };
        value
    }

    #[test]
    fn a_lone_observation_is_credited_until_the_next_one() {
        // The regression this guards: attention was read as last-event minus first-event, so a
        // segment holding a single observation measured zero no matter how long it actually held.
        let events = vec![
            event(1, 0, "Ghostty", None),
            event(2, 6, "Chrome", None),
            event(3, 9, "Chrome", None),
        ];
        let segments = segment_events(&events, SegmentationSettings::default());

        assert_eq!(segments[0].applications, ["Ghostty"]);
        assert_eq!(segments[0].event_ids.len(), 1);
        assert_eq!(segments[0].observed_seconds, 6 * 60);
    }

    #[test]
    fn sustained_attention_on_one_unchanging_thing_is_kept_whole() {
        // Watching something for forty minutes changes nothing observable, so it records one event.
        // Reading that as "nobody was here" would delete the evidence of the longest thing the user
        // did all day; the interval until the next observation is measured, so it is credited.
        let events = vec![event(1, 0, "Player", None), event(2, 40, "Ghostty", None)];
        let segments = segment_events(&events, SegmentationSettings::default());

        assert_eq!(segments[0].applications, ["Player"]);
        assert_eq!(segments[0].observed_seconds, 40 * 60);
    }

    #[test]
    fn a_closing_boundary_ends_attention_where_collection_stopped() {
        let events = vec![
            event(1, 0, "Ghostty", None),
            marker(2, 5, LifecycleBoundary::Sleep),
            marker(3, 55, LifecycleBoundary::Resume),
            event(4, 56, "Ghostty", None),
        ];
        let segments = segment_events(&events, SegmentationSettings::default());

        // Five observed minutes before the machine went down, and nothing for the fifty it spent
        // asleep — not the fifty-six minutes the raw timestamps span.
        assert_eq!(segments[0].observed_seconds, 5 * 60);
        assert_eq!(segments.len(), 2, "a segment must not span a suspension");
        assert!(
            segments
                .iter()
                .all(|segment| segment.applications == ["Ghostty"]),
            "presence markers must never become activity of their own"
        );
    }

    #[test]
    fn time_nobody_watched_is_not_credited_as_attention() {
        // A collector that stopped without warning leaves no closing boundary; the next launch's
        // `Resume` is the only evidence, and it says the gap was unobserved rather than spent.
        let events = vec![
            event(1, 0, "Ghostty", None),
            marker(2, 45, LifecycleBoundary::Resume),
            event(3, 46, "Ghostty", None),
        ];
        let segments = segment_events(&events, SegmentationSettings::default());

        assert_eq!(segments[0].observed_seconds, 0);
    }

    /// Reassigns an event to a collector run, as the adapter does when it stamps its own identity.
    fn from_run(mut value: EventEnvelope, run: &str, tick: u64) -> EventEnvelope {
        value.source.source_id = run.into();
        value.monotonic_ticks = tick;
        value
    }

    #[test]
    fn a_restart_does_not_look_like_a_clock_that_went_backwards() {
        // A restart begins counting ticks from zero again. Ordering clamps a counter that moves
        // backwards inside one source, so the two runs have to be two sources: otherwise the second
        // run's first tick sorts against the first run's, and every event after it is dragged
        // forward to a timestamp it never had.
        let events = vec![
            from_run(event(1, 0, "Ghostty", None), "run-a", 1),
            from_run(event(2, 3, "Ghostty", None), "run-a", 2),
            from_run(marker(3, 50, LifecycleBoundary::Resume), "run-b", 1),
            from_run(event(4, 51, "Chrome", None), "run-b", 2),
        ];
        let segments = segment_events(&events, SegmentationSettings::default());

        // Three observed minutes, then nothing until the relaunch announced itself.
        assert_eq!(segments[0].observed_seconds, 3 * 60);
        assert_eq!(segments[0].applications, ["Ghostty"]);
    }

    #[test]
    fn a_scrambled_order_credits_no_more_attention_than_time_elapsed() {
        // What two runs sharing one source identity leave behind: ticks restart, so ordering reads
        // the restart as a clock rollback and clamps the older run's timestamps forward. The
        // sequence no longer agrees with the timestamps on it, and measuring between raw timestamps
        // credited a jump of the whole gap to every event on either side of one — which is how 308
        // segments came to claim thirteen days of attention inside a six hour day.
        let events = vec![
            from_run(event(1, 0, "Ghostty", None), "shared", 1),
            from_run(event(2, 3, "Ghostty", None), "shared", 2),
            from_run(event(3, 50, "Chrome", None), "shared", 1),
            from_run(event(4, 53, "Chrome", None), "shared", 2),
        ];
        let segments = segment_events(&events, SegmentationSettings::default());

        let credited: i64 = segments
            .iter()
            .map(|segment| segment.observed_seconds)
            .sum();
        assert!(
            credited <= 53 * 60,
            "credited {credited}s of attention inside 53 elapsed minutes"
        );
    }

    #[test]
    fn the_newest_observation_has_no_measured_ending() {
        let events = vec![event(1, 0, "Ghostty", None), event(2, 3, "Chrome", None)];
        let segments = segment_events(&events, SegmentationSettings::default());

        assert!(!segments[0].open_ended, "a replaced observation has an end");
        assert!(
            segments[1].open_ended,
            "nothing has replaced the last observation, so its end is unknown"
        );
        assert_eq!(segments[1].observed_seconds, 0);
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

    fn with_title(mut value: EventEnvelope, title: &str) -> EventEnvelope {
        if let SemanticPayload::WindowChanged { window_title, .. } = &mut value.payload {
            *window_title = Some(title.to_owned());
        }
        value
    }

    #[test]
    fn matching_project_identity_scores_above_the_title_fallback() {
        let same_project = continuity_score(
            &event(1, 0, "Editor", Some("project-a")),
            &event(2, 1, "Editor", Some("project-a")),
        );
        let unresolved_but_same_title = continuity_score(
            &with_title(event(1, 0, "Editor", None), "notes.txt"),
            &with_title(event(2, 1, "Editor", None), "notes.txt"),
        );
        let unresolved_and_different_title = continuity_score(
            &with_title(event(1, 0, "Editor", None), "notes.txt"),
            &with_title(event(2, 1, "Editor", None), "other.txt"),
        );

        // Real project identity beats a title match, which in turn beats no signal at all: an
        // unresolved observation is never treated as more confident than a resolved one.
        assert!(same_project > unresolved_but_same_title);
        assert!(unresolved_but_same_title > unresolved_and_different_title);
    }

    #[test]
    fn title_fallback_only_applies_when_neither_side_resolved_a_project() {
        let resolved_versus_unresolved_matching_title = continuity_score(
            &event(1, 0, "Editor", Some("project-a")),
            &with_title(event(2, 1, "Editor", None), "notes.txt"),
        );
        let both_unresolved_matching_title = continuity_score(
            &with_title(event(1, 0, "Editor", None), "notes.txt"),
            &with_title(event(2, 1, "Editor", None), "notes.txt"),
        );

        // A resolved project on only one side is not "neither side resolved a project": it must
        // not receive the title-fallback bonus even when titles happen to match, and so should
        // score no higher than the neutral, no-signal case.
        assert!(resolved_versus_unresolved_matching_title < both_unresolved_matching_title);
    }
}
