//! Least-sensitive response types shared by the loopback API and MCP companion.

use chrono::{DateTime, FixedOffset};
use serde::{Deserialize, Serialize};

/// Maximum number of records returned by any V1 query.
pub const MAX_QUERY_LIMIT: u16 = 100;

/// Explicit bounded query shared by local clients.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HistoryQuery {
    /// Inclusive range start.
    pub from: DateTime<FixedOffset>,
    /// Exclusive range end.
    pub to: DateTime<FixedOffset>,
    /// Requested maximum, clamped to [`MAX_QUERY_LIMIT`].
    pub limit: u16,
}

impl HistoryQuery {
    /// Returns a copy with a safe record bound.
    #[must_use]
    pub fn bounded(mut self) -> Self {
        self.limit = self.limit.clamp(1, MAX_QUERY_LIMIT);
        self
    }
}

/// Derived summary returned without raw event content.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentTaskSummary {
    /// Stable derived segment identity.
    pub id: String,
    /// Grounded summary title.
    pub title: String,
    /// Grounded summary body.
    pub summary: String,
    /// Allowed contributing source labels.
    pub sources: Vec<String>,
    /// Every source-derived string is explicitly marked untrusted.
    pub untrusted_content: bool,
    /// Human-readable provenance category.
    pub provenance: String,
}

#[cfg(test)]
mod tests {
    use chrono::DateTime;

    use super::*;

    #[test]
    fn query_limit_is_never_unbounded() {
        let query = HistoryQuery {
            from: DateTime::parse_from_rfc3339("2026-09-08T00:00:00+08:00").unwrap(),
            to: DateTime::parse_from_rfc3339("2026-09-09T00:00:00+08:00").unwrap(),
            limit: u16::MAX,
        };
        assert_eq!(query.bounded().limit, MAX_QUERY_LIMIT);
    }
}

