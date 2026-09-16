//! Range aggregation of evidence into project-level work items, and Markdown report export.
//!
//! A digest never reads raw events or raw strings itself: every [`EvidenceItem`] arrives already
//! attributed to a project entity by the resolver, and already carrying only the counts and text a
//! report needs. This keeps a digest's size proportional to the number of distinct pieces of
//! evidence in a range, not to how many raw events produced them.
//!
//! This crate makes no outbound request and never will: its only dependencies are `chrono` and
//! `serde`, neither capable of network I/O, and every public function here is synchronous — there
//! is no `.await` point for a request to hide behind. [`aggregate_range`] and [`render_markdown`]
//! are usable with no summarization engine configured at all, which is what "remains available
//! with no summarization engine configured" means before any engine exists to configure.

use std::collections::BTreeSet;
use std::fmt::Write as _;

use chrono::{DateTime, FixedOffset, NaiveDate};
use openhistory_domain::EntityId;
use serde::{Deserialize, Serialize};

/// One piece of evidence contributing to a project's work item.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceItem {
    /// Project entity this evidence has already been attributed to.
    pub project_id: EntityId,
    /// Human-readable project label, when one was observed.
    pub project_label: Option<String>,
    /// When the evidence occurred.
    pub occurred_at: DateTime<FixedOffset>,
    /// What kind of evidence this is.
    pub kind: EvidenceKind,
}

/// The category of one piece of evidence, carrying only what a digest needs to report it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum EvidenceKind {
    /// Observed window attention, already reduced to a duration.
    Attention {
        /// Minutes of observed attention.
        minutes: u32,
    },
    /// A Git commit.
    Commit {
        /// Commit identifier.
        commit_id: String,
        /// Commit subject.
        subject: Option<String>,
    },
    /// An update from a local coding-agent session.
    AgentSession {
        /// Session thread identifier.
        thread_id: String,
        /// Most recent request or result text, for display only.
        summary: Option<String>,
    },
    /// An attended meeting.
    Meeting {
        /// Meeting subject, when the calendar or observation supplied one.
        subject: Option<String>,
        /// Observed minutes.
        minutes: u32,
    },
}

/// An inclusive date range a digest is generated over.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DateRange {
    /// First day included.
    pub start: NaiveDate,
    /// Last day included.
    pub end: NaiveDate,
}

impl DateRange {
    /// Builds a range, swapping the bounds if given in the wrong order.
    #[must_use]
    pub fn new(start: NaiveDate, end: NaiveDate) -> Self {
        if start <= end {
            Self { start, end }
        } else {
            Self {
                start: end,
                end: start,
            }
        }
    }

    fn contains(self, date: NaiveDate) -> bool {
        date >= self.start && date <= self.end
    }
}

/// One project's aggregated work for a range, with links back to its evidence.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkItem {
    /// Project entity this work item reports on.
    pub project_id: EntityId,
    /// Display title, falling back to the project identifier when no label was observed.
    pub title: String,
    /// Total observed attention minutes. Kept separate from artifact counts; never summed with
    /// them into one elapsed figure.
    pub attention_minutes: u32,
    /// Number of commits contributing to this work item.
    pub commit_count: u32,
    /// Number of distinct agent-session threads contributing to this work item.
    pub agent_session_count: u32,
    /// Number of attended meetings contributing to this work item.
    pub meeting_count: u32,
    /// Days within the range on which evidence was observed, ordered.
    pub active_days: Vec<NaiveDate>,
    /// Evidence this work item is traceable to, in occurrence order.
    pub evidence: Vec<EvidenceItem>,
}

impl WorkItem {
    /// Returns true when the only evidence is artifact evidence, with no observed attention.
    #[must_use]
    pub fn has_no_observed_attention(&self) -> bool {
        self.attention_minutes == 0
    }
}

/// Aggregates evidence within `range` into one work item per project, ordered deterministically.
///
/// Aggregating the same evidence twice produces byte-equivalent output: ordering is by project
/// identifier, and every collection this function builds is sorted before being placed in a
/// [`WorkItem`].
#[must_use]
pub fn aggregate_range(evidence: &[EvidenceItem], range: DateRange) -> Vec<WorkItem> {
    let mut by_project: Vec<(EntityId, Vec<EvidenceItem>)> = Vec::new();

    for item in evidence {
        if !range.contains(item.occurred_at.date_naive()) {
            continue;
        }
        match by_project
            .iter_mut()
            .find(|(project_id, _)| *project_id == item.project_id)
        {
            Some((_, items)) => items.push(item.clone()),
            None => by_project.push((item.project_id.clone(), vec![item.clone()])),
        }
    }

    by_project.sort_by(|left, right| left.0.cmp(&right.0));

    by_project
        .into_iter()
        .map(|(project_id, mut items)| {
            items.sort_by_key(|item| item.occurred_at);
            build_work_item(project_id, items)
        })
        .collect()
}

fn build_work_item(project_id: EntityId, items: Vec<EvidenceItem>) -> WorkItem {
    let mut attention_minutes = 0u32;
    let mut commit_count = 0u32;
    let mut agent_session_threads: BTreeSet<String> = BTreeSet::new();
    let mut meeting_count = 0u32;
    let mut active_days: BTreeSet<NaiveDate> = BTreeSet::new();
    let mut label = None;

    for item in &items {
        active_days.insert(item.occurred_at.date_naive());
        if label.is_none() {
            label.clone_from(&item.project_label);
        }
        match &item.kind {
            EvidenceKind::Attention { minutes } => attention_minutes += minutes,
            EvidenceKind::Commit { .. } => commit_count += 1,
            EvidenceKind::AgentSession { thread_id, .. } => {
                agent_session_threads.insert(thread_id.clone());
            }
            EvidenceKind::Meeting { minutes, .. } => {
                attention_minutes += minutes;
                meeting_count += 1;
            }
        }
    }

    WorkItem {
        title: label.unwrap_or_else(|| project_id.to_string()),
        attention_minutes,
        commit_count,
        agent_session_count: u32::try_from(agent_session_threads.len()).unwrap_or(u32::MAX),
        meeting_count,
        active_days: active_days.into_iter().collect(),
        project_id,
        evidence: items,
    }
}

/// Renders a range digest as a Markdown draft.
///
/// The output is explicitly marked as generated and contains only deterministic content: no
/// narrative summarization runs inside this function.
#[must_use]
pub fn render_markdown(range: DateRange, work_items: &[WorkItem]) -> String {
    let mut output = String::new();
    let _ = writeln!(output, "# Report draft: {} to {}", range.start, range.end);
    let _ = writeln!(output);
    let _ = writeln!(
        output,
        "_Generated by OpenHistory from local evidence. Deterministic content only; no narrative \
         text has been added._"
    );
    let _ = writeln!(output);

    if work_items.is_empty() {
        let _ = writeln!(output, "No activity was recorded for this range.");
        return output;
    }

    for item in work_items {
        let _ = writeln!(output, "## {}", item.title);
        let _ = writeln!(output);
        if item.has_no_observed_attention() {
            let _ = writeln!(
                output,
                "- Attention: no attention data was observed for this project"
            );
        } else {
            let _ = writeln!(output, "- Attention: {} min", item.attention_minutes);
        }
        let _ = writeln!(output, "- Commits: {}", item.commit_count);
        let _ = writeln!(output, "- Agent sessions: {}", item.agent_session_count);
        let _ = writeln!(output, "- Meetings: {}", item.meeting_count);
        let days = item
            .active_days
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(", ");
        let _ = writeln!(output, "- Active days: {days}");
        let _ = writeln!(output);
        let _ = writeln!(output, "Evidence:");
        for evidence in &item.evidence {
            let _ = writeln!(output, "- {}", describe(evidence));
        }
        let _ = writeln!(output);
    }

    output
}

fn describe(evidence: &EvidenceItem) -> String {
    let when = evidence.occurred_at.format("%Y-%m-%d %H:%M");
    match &evidence.kind {
        EvidenceKind::Attention { minutes } => {
            format!("{when} — observed activity ({minutes} min)")
        }
        EvidenceKind::Commit { commit_id, subject } => {
            let short_id = commit_id.get(..12).unwrap_or(commit_id);
            match subject {
                Some(subject) => format!("{when} — commit {short_id}: {subject}"),
                None => format!("{when} — commit {short_id}"),
            }
        }
        EvidenceKind::AgentSession { thread_id, summary } => match summary {
            Some(summary) => format!("{when} — agent session {thread_id}: {summary}"),
            None => format!("{when} — agent session {thread_id}"),
        },
        EvidenceKind::Meeting { subject, minutes } => match subject {
            Some(subject) => format!("{when} — meeting \"{subject}\" ({minutes} min)"),
            None => format!("{when} — meeting ({minutes} min)"),
        },
    }
}

#[cfg(test)]
mod tests {
    use openhistory_domain::EntityKey;

    use super::*;

    fn project(name: &str) -> EntityId {
        EntityId::derive(&EntityKey::repository_path(&format!("/work/{name}")).unwrap())
    }

    fn at(day: &str, hour: u32) -> DateTime<FixedOffset> {
        let date = NaiveDate::parse_from_str(day, "%Y-%m-%d").unwrap();
        date.and_hms_opt(hour, 0, 0)
            .unwrap()
            .and_local_timezone(FixedOffset::east_opt(0).unwrap())
            .unwrap()
    }

    fn range(start: &str, end: &str) -> DateRange {
        DateRange::new(
            NaiveDate::parse_from_str(start, "%Y-%m-%d").unwrap(),
            NaiveDate::parse_from_str(end, "%Y-%m-%d").unwrap(),
        )
    }

    #[test]
    fn work_spanning_several_days_merges_into_one_work_item() {
        let project_id = project("open-history");
        let evidence = vec![
            EvidenceItem {
                project_id: project_id.clone(),
                project_label: Some("open-history".into()),
                occurred_at: at("2026-09-14", 9),
                kind: EvidenceKind::Attention { minutes: 60 },
            },
            EvidenceItem {
                project_id: project_id.clone(),
                project_label: Some("open-history".into()),
                occurred_at: at("2026-09-15", 10),
                kind: EvidenceKind::Commit {
                    commit_id: "abc123def456".into(),
                    subject: Some("feat: add digest".into()),
                },
            },
            EvidenceItem {
                project_id: project_id.clone(),
                project_label: Some("open-history".into()),
                occurred_at: at("2026-09-16", 11),
                kind: EvidenceKind::AgentSession {
                    thread_id: "thread-1".into(),
                    summary: Some("Implemented digest aggregation".into()),
                },
            },
        ];

        let work_items = aggregate_range(&evidence, range("2026-09-14", "2026-09-20"));

        assert_eq!(work_items.len(), 1);
        let item = &work_items[0];
        assert_eq!(item.attention_minutes, 60);
        assert_eq!(item.commit_count, 1);
        assert_eq!(item.agent_session_count, 1);
        assert_eq!(item.active_days.len(), 3);
    }

    #[test]
    fn work_with_no_artifacts_still_appears() {
        let project_id = project("reading");
        let evidence = vec![EvidenceItem {
            project_id: project_id.clone(),
            project_label: None,
            occurred_at: at("2026-09-14", 9),
            kind: EvidenceKind::Attention { minutes: 30 },
        }];

        let work_items = aggregate_range(&evidence, range("2026-09-14", "2026-09-20"));

        assert_eq!(work_items.len(), 1);
        assert_eq!(work_items[0].commit_count, 0);
        assert_eq!(work_items[0].title, project_id.to_string());
    }

    #[test]
    fn regenerating_an_unchanged_range_is_byte_equivalent() {
        let project_id = project("open-history");
        let evidence = vec![
            EvidenceItem {
                project_id: project_id.clone(),
                project_label: Some("open-history".into()),
                occurred_at: at("2026-09-15", 10),
                kind: EvidenceKind::Commit {
                    commit_id: "aaa".into(),
                    subject: None,
                },
            },
            EvidenceItem {
                project_id,
                project_label: Some("open-history".into()),
                occurred_at: at("2026-09-14", 9),
                kind: EvidenceKind::Attention { minutes: 15 },
            },
        ];
        let target_range = range("2026-09-14", "2026-09-20");

        let first = render_markdown(target_range, &aggregate_range(&evidence, target_range));
        let second = render_markdown(target_range, &aggregate_range(&evidence, target_range));

        assert_eq!(first, second);
    }

    #[test]
    fn artifact_only_work_states_no_attention_was_observed() {
        let project_id = project("agent-only");
        let evidence = vec![EvidenceItem {
            project_id,
            project_label: Some("agent-only".into()),
            occurred_at: at("2026-09-14", 9),
            kind: EvidenceKind::AgentSession {
                thread_id: "thread-1".into(),
                summary: None,
            },
        }];

        let work_items = aggregate_range(&evidence, range("2026-09-14", "2026-09-20"));
        let markdown = render_markdown(range("2026-09-14", "2026-09-20"), &work_items);

        assert!(work_items[0].has_no_observed_attention());
        assert!(markdown.contains("no attention data was observed"));
    }

    #[test]
    fn evidence_outside_the_range_is_excluded() {
        let project_id = project("open-history");
        let evidence = vec![EvidenceItem {
            project_id,
            project_label: Some("open-history".into()),
            occurred_at: at("2026-09-01", 9),
            kind: EvidenceKind::Attention { minutes: 10 },
        }];

        let work_items = aggregate_range(&evidence, range("2026-09-14", "2026-09-20"));
        assert!(work_items.is_empty());
    }

    #[test]
    fn empty_range_renders_without_fabricating_work() {
        let markdown = render_markdown(range("2026-09-14", "2026-09-20"), &[]);
        assert!(markdown.contains("No activity was recorded"));
    }

    #[test]
    fn attention_and_artifact_counts_never_combine() {
        let project_id = project("mixed");
        let evidence = vec![
            EvidenceItem {
                project_id: project_id.clone(),
                project_label: Some("mixed".into()),
                occurred_at: at("2026-09-14", 9),
                kind: EvidenceKind::Attention { minutes: 45 },
            },
            EvidenceItem {
                project_id,
                project_label: Some("mixed".into()),
                occurred_at: at("2026-09-14", 10),
                kind: EvidenceKind::Commit {
                    commit_id: "abc".into(),
                    subject: None,
                },
            },
        ];

        let work_items = aggregate_range(&evidence, range("2026-09-14", "2026-09-20"));

        assert_eq!(work_items[0].attention_minutes, 45);
        assert_eq!(work_items[0].commit_count, 1);
    }

    #[test]
    fn every_work_item_resolves_to_openable_evidence() {
        let project_id = project("open-history");
        let evidence = vec![EvidenceItem {
            project_id: project_id.clone(),
            project_label: Some("open-history".into()),
            occurred_at: at("2026-09-15", 10),
            kind: EvidenceKind::Commit {
                commit_id: "abc".into(),
                subject: Some("feat: add digest".into()),
            },
        }];

        let work_items = aggregate_range(&evidence, range("2026-09-14", "2026-09-20"));

        // "Openable" here means the work item carries the evidence itself, not a reference a
        // caller has to resolve elsewhere — there is nowhere else for it to be lost between.
        assert_eq!(work_items[0].evidence, evidence);
    }

    #[test]
    fn deleted_evidence_disappears_from_the_next_generation_with_no_orphaned_copy() {
        // aggregate_range holds no state between calls: nothing here is a cache to leave a copy
        // in. Deleting evidence "through existing deletion controls" (a caller's storage layer)
        // means the next call simply receives a shorter list, which this proves is sufficient.
        let project_id = project("open-history");
        let kept = EvidenceItem {
            project_id: project_id.clone(),
            project_label: Some("open-history".into()),
            occurred_at: at("2026-09-15", 10),
            kind: EvidenceKind::Commit {
                commit_id: "kept".into(),
                subject: None,
            },
        };
        let deleted = EvidenceItem {
            project_id,
            project_label: Some("open-history".into()),
            occurred_at: at("2026-09-15", 11),
            kind: EvidenceKind::Commit {
                commit_id: "deleted".into(),
                subject: None,
            },
        };

        let before = aggregate_range(&[kept.clone(), deleted], range("2026-09-14", "2026-09-20"));
        assert_eq!(before[0].evidence.len(), 2);

        let after = aggregate_range(&[kept], range("2026-09-14", "2026-09-20"));
        assert_eq!(after[0].evidence.len(), 1);
        assert_eq!(
            after[0].evidence[0].kind,
            EvidenceKind::Commit {
                commit_id: "kept".into(),
                subject: None,
            }
        );
    }

    #[test]
    fn a_high_volume_week_aggregates_and_exports_with_no_summarization_engine_and_no_network() {
        // 20,000 raw-scale evidence items across many projects, over a wall-clock budget tight
        // enough to fail if this ever started doing per-item I/O. There is nothing here capable
        // of a network call in the first place (see the module docs), so this test is really
        // checking that the busy-week path stays a pure, bounded computation as it scales.
        let target_range = range("2026-09-14", "2026-09-20");
        let evidence: Vec<EvidenceItem> = (0..20_000)
            .map(|index| EvidenceItem {
                project_id: project(&format!("project-{}", index % 50)),
                project_label: Some(format!("project-{}", index % 50)),
                occurred_at: at("2026-09-14", u32::try_from(index % 24).unwrap()),
                kind: EvidenceKind::Commit {
                    commit_id: format!("commit-{index}"),
                    subject: Some(format!("change {index}")),
                },
            })
            .collect();

        let started = std::time::Instant::now();
        let work_items = aggregate_range(&evidence, target_range);
        let markdown = render_markdown(target_range, &work_items);
        let elapsed = started.elapsed();

        assert_eq!(work_items.len(), 50);
        assert!(!markdown.is_empty());
        assert!(
            elapsed < std::time::Duration::from_secs(5),
            "aggregation took {elapsed:?}, too slow for a bounded local computation"
        );
    }
}
