//! Per-source collection status, kept separate from evidence content so it can be shown to the
//! user without exposing anything excluded content ever touched.
//!
//! The central distinction this crate exists to make explicit: a source that is enabled and
//! healthy but has recorded nothing recently ("no activity") is a different situation from a
//! source that is disabled, degraded, or lacks permission ("not collecting"). Collapsing the two
//! reads as "nothing is being recorded" either way, which is exactly the ambiguity that makes a
//! silently broken source invisible.

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

/// Whether a source is currently enabled to collect, and why not if it isn't.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SourceState {
    /// Enabled and functioning normally.
    Active,
    /// Enabled, but degraded — a parser could not recognize its input, a permission was revoked
    /// mid-session, or similar. Carries a category-only reason; never excluded content.
    Degraded {
        /// Non-identifying reason category, safe to display.
        reason: String,
    },
    /// Not enabled: the user has not opted in, or has turned it off.
    Disabled,
}

/// One source's status as shown to the user.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceHealth {
    /// Stable local source identifier, safe to display.
    pub source_id: String,
    /// Current enablement and health.
    pub state: SourceState,
    /// When this source last produced evidence, if ever.
    pub last_evidence_at: Option<DateTime<Utc>>,
}

/// How a source's current situation should be explained to the user.
///
/// This is the type that makes "no activity" and "not collecting" mutually exclusive at the type
/// level: a caller matching on this cannot accidentally render one as the other.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActivityDescription {
    /// Enabled, healthy, and has recorded evidence within the idle threshold.
    Active {
        /// Most recent evidence timestamp.
        last_evidence_at: DateTime<Utc>,
    },
    /// Enabled and healthy, but nothing has been recorded recently. This is a fact about the
    /// world (nothing happened), not about the source.
    NoActivity {
        /// Most recent evidence timestamp, if this source has ever produced any.
        last_evidence_at: Option<DateTime<Utc>>,
    },
    /// Not currently collecting, with a category-only reason. Silence here is explained by the
    /// source, not by an absence of activity.
    NotCollecting {
        /// Non-identifying reason category.
        reason: String,
    },
}

/// How long a healthy source can go without evidence before it reads as "no activity" rather than
/// "active" outright. Below this threshold, a momentary gap between events still reads as active.
const IDLE_THRESHOLD_MINUTES: i64 = 15;

/// Describes a source's current situation for display, given the current time.
///
/// Excluded content never reaches this function's inputs: `state`'s reason and `source_id` are
/// the only strings involved, and both are constrained to non-identifying categories by
/// [`SourceState`] and by callers that construct [`SourceHealth`].
#[must_use]
pub fn describe(health: &SourceHealth, now: DateTime<Utc>) -> ActivityDescription {
    match &health.state {
        SourceState::Disabled => ActivityDescription::NotCollecting {
            reason: "disabled".to_owned(),
        },
        SourceState::Degraded { reason } => ActivityDescription::NotCollecting {
            reason: reason.clone(),
        },
        SourceState::Active => match health.last_evidence_at {
            Some(last_evidence_at)
                if now - last_evidence_at <= Duration::minutes(IDLE_THRESHOLD_MINUTES) =>
            {
                ActivityDescription::Active { last_evidence_at }
            }
            last_evidence_at => ActivityDescription::NoActivity { last_evidence_at },
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn now() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-09-15T18:00:00Z")
            .unwrap()
            .with_timezone(&Utc)
    }

    #[test]
    fn a_recently_active_source_reads_as_active() {
        let health = SourceHealth {
            source_id: "git:open-history".into(),
            state: SourceState::Active,
            last_evidence_at: Some(now() - Duration::minutes(5)),
        };
        assert!(matches!(
            describe(&health, now()),
            ActivityDescription::Active { .. }
        ));
    }

    #[test]
    fn an_idle_period_is_no_activity_not_not_collecting() {
        let health = SourceHealth {
            source_id: "git:open-history".into(),
            state: SourceState::Active,
            last_evidence_at: Some(now() - Duration::hours(6)),
        };
        assert_eq!(
            describe(&health, now()),
            ActivityDescription::NoActivity {
                last_evidence_at: Some(now() - Duration::hours(6))
            }
        );
    }

    #[test]
    fn a_source_that_never_produced_evidence_is_still_no_activity_when_active() {
        let health = SourceHealth {
            source_id: "calendar".into(),
            state: SourceState::Active,
            last_evidence_at: None,
        };
        assert_eq!(
            describe(&health, now()),
            ActivityDescription::NoActivity {
                last_evidence_at: None
            }
        );
    }

    #[test]
    fn a_disabled_source_is_not_collecting_regardless_of_past_evidence() {
        let health = SourceHealth {
            source_id: "calendar".into(),
            state: SourceState::Disabled,
            last_evidence_at: Some(now() - Duration::minutes(1)),
        };
        assert_eq!(
            describe(&health, now()),
            ActivityDescription::NotCollecting {
                reason: "disabled".into()
            }
        );
    }

    #[test]
    fn a_permission_revoked_source_is_not_collecting_with_its_reason() {
        let health = SourceHealth {
            source_id: "macos-accessibility".into(),
            state: SourceState::Degraded {
                reason: "permission_revoked".into(),
            },
            last_evidence_at: Some(now() - Duration::minutes(1)),
        };
        assert_eq!(
            describe(&health, now()),
            ActivityDescription::NotCollecting {
                reason: "permission_revoked".into()
            }
        );
    }

    #[test]
    fn a_degraded_parser_is_not_collecting_while_other_sources_are_unaffected() {
        let degraded = SourceHealth {
            source_id: "codex-session".into(),
            state: SourceState::Degraded {
                reason: "unrecognized_format".into(),
            },
            last_evidence_at: None,
        };
        let active = SourceHealth {
            source_id: "git:open-history".into(),
            state: SourceState::Active,
            last_evidence_at: Some(now()),
        };

        assert_eq!(
            describe(&degraded, now()),
            ActivityDescription::NotCollecting {
                reason: "unrecognized_format".into()
            }
        );
        assert!(matches!(
            describe(&active, now()),
            ActivityDescription::Active { .. }
        ));
    }
}
