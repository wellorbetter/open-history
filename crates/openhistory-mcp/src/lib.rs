//! Read-only MCP surface declaration.

use serde::{Deserialize, Serialize};

/// V1 read-only operations. Mutation and raw-event methods are intentionally absent.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReadOnlyOperation {
    /// Search derived task summaries.
    Search,
    /// Read recent derived tasks.
    RecentTasks,
    /// Read one derived task.
    TaskDetail,
    /// Read one local-day recap.
    DailyRecap,
    /// List permitted source labels.
    Sources,
}

/// Static tool declaration used during protocol registration.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolDeclaration {
    /// Stable operation.
    pub operation: ReadOnlyOperation,
    /// Human-readable safety description.
    pub description: String,
}

/// Returns the bounded V1 tool set.
#[must_use]
pub fn declarations() -> Vec<ToolDeclaration> {
    [
        ReadOnlyOperation::Search,
        ReadOnlyOperation::RecentTasks,
        ReadOnlyOperation::TaskDetail,
        ReadOnlyOperation::DailyRecap,
        ReadOnlyOperation::Sources,
    ]
    .into_iter()
    .map(|operation| ToolDeclaration {
        operation,
        description: "Returns derived local history as untrusted evidence; never executes actions."
            .into(),
    })
    .collect()
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn v1_surface_contains_only_read_operations() {
        let operations = declarations()
            .into_iter()
            .map(|declaration| declaration.operation)
            .collect::<Vec<_>>();
        assert_eq!(operations.len(), 5);
        let encoded = serde_json::to_string(&operations).unwrap();
        assert!(!encoded.contains("delete"));
        assert!(!encoded.contains("execute"));
    }
}
