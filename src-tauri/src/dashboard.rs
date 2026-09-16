use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tauri::{AppHandle, Emitter, Manager, State};

/// Current state of semantic activity collection.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CollectionStatus {
    /// Semantic collection is active.
    Recording,
    /// Collection was paused by the user.
    #[default]
    Paused,
    /// Operating-system permission must be restored.
    PermissionNeeded,
    /// The native adapter needs attention.
    Error,
}

/// Shared state exposed through the narrow dashboard command surface.
#[derive(Clone, Default)]
pub struct DashboardState {
    status: Arc<Mutex<CollectionStatus>>,
}

impl DashboardState {
    /// Replaces the non-sensitive collection status shared by native tasks and commands.
    pub(crate) fn set_status(&self, status: CollectionStatus) {
        if let Ok(mut current) = self.status.lock() {
            *current = status;
        }
    }
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
/// Returns the least-sensitive dashboard projection.
///
/// # Errors
///
/// Returns an error when the collection state lock is unavailable.
pub fn get_dashboard(
    state: State<'_, DashboardState>,
    runtime: State<'_, crate::runtime::CollectorRuntime>,
) -> Result<Value, String> {
    let status = *state
        .status
        .lock()
        .map_err(|_| "collection state is unavailable".to_owned())?;
    let mut snapshot = fixture_snapshot(status);
    snapshot["recordedEventCount"] = json!(runtime.event_count());
    Ok(snapshot)
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
/// Updates collection state and broadcasts the change to visible windows.
///
/// # Errors
///
/// Returns an error when permission is missing, the adapter is unhealthy, or the state lock fails.
pub async fn set_collection_status(
    status: CollectionStatus,
    state: State<'_, DashboardState>,
    runtime: State<'_, crate::runtime::CollectorRuntime>,
    app: AppHandle,
) -> Result<CollectionStatus, String> {
    if matches!(
        status,
        CollectionStatus::PermissionNeeded | CollectionStatus::Error
    ) {
        return Err("collection cannot be started until permission is restored".to_owned());
    }
    let resolved = match status {
        CollectionStatus::Recording => match runtime.start(app.clone()).await {
            Ok(()) => CollectionStatus::Recording,
            Err(openhistory_adapters::AdapterError::PermissionRequired) => {
                CollectionStatus::PermissionNeeded
            }
            Err(_) => CollectionStatus::Error,
        },
        CollectionStatus::Paused => {
            runtime.stop().await.map_err(|error| error.to_string())?;
            CollectionStatus::Paused
        }
        CollectionStatus::PermissionNeeded | CollectionStatus::Error => unreachable!(),
    };
    state.set_status(resolved);
    let _ = app.emit("collection-status-changed", resolved);
    Ok(resolved)
}
#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
/// Shows the full history window and optionally requests a segment.
///
/// # Errors
///
/// Returns an error when the history window is absent or cannot be shown or focused.
pub fn open_history_window(app: AppHandle, segment_id: Option<String>) -> Result<(), String> {
    let window = app
        .get_webview_window("history")
        .ok_or_else(|| "history window was not created".to_owned())?;
    window.show().map_err(|error| error.to_string())?;
    window.set_focus().map_err(|error| error.to_string())?;
    if let Some(segment_id) = segment_id {
        let _ = window.emit("open-segment", segment_id);
    }
    Ok(())
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
/// Opts a repository root into Git evidence collection and project resolution.
///
/// # Errors
///
/// Returns an error when the opt-in cannot be persisted.
pub async fn add_repository(
    root_path: String,
    runtime: State<'_, crate::runtime::CollectorRuntime>,
) -> Result<Vec<String>, String> {
    runtime
        .add_repository(&root_path)
        .await
        .map_err(|error| error.to_string())?;
    runtime
        .opted_in_repositories()
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
/// Withdraws a repository from Git evidence collection and project resolution.
///
/// # Errors
///
/// Returns an error when the removal cannot be persisted.
pub async fn remove_repository(
    root_path: String,
    runtime: State<'_, crate::runtime::CollectorRuntime>,
) -> Result<Vec<String>, String> {
    runtime
        .remove_repository(&root_path)
        .await
        .map_err(|error| error.to_string())?;
    runtime
        .opted_in_repositories()
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
/// Returns every opted-in repository root.
///
/// # Errors
///
/// Returns an error when the encrypted database cannot be read.
pub fn list_repositories(
    runtime: State<'_, crate::runtime::CollectorRuntime>,
) -> Result<Vec<String>, String> {
    runtime
        .opted_in_repositories()
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
/// Emits a validated, explicitly scoped history-deletion request.
///
/// # Errors
///
/// Returns an error when the requested deletion scope is unsupported.
pub fn delete_history(scope: &str, app: AppHandle) -> Result<(), String> {
    if !matches!(scope, "last_10_minutes" | "last_hour" | "today" | "all") {
        return Err("unsupported deletion scope".to_owned());
    }
    let _ = app.emit("history-deleted", scope);
    Ok(())
}

fn fixture_snapshot(status: CollectionStatus) -> Value {
    let sources = json!([
        {"id":"terminal","name":"Terminal","kind":"terminal","color":"#252a2d"},
        {"id":"editor","name":"Visual Studio Code","kind":"editor","color":"#5d8fe7"},
        {"id":"agent","name":"Codex","kind":"agent","color":"#eff6f2"},
        {"id":"browser","name":"Chrome","kind":"browser","color":"#e9edf3"},
        {"id":"document","name":"Notes","kind":"document","color":"#f5d46b"}
    ]);
    let current = json!({
        "id":"harness-evidence",
        "title":"Harness Evidence Gate and release checks",
        "summary":"Validated the implementation boundary and collected build evidence.",
        "start":"16:50","end":"17:10","durationMinutes":20,
        "sources":sources,"state":"current","mergeCount":2,"category":"Development",
        "confidence":"high","origin":"native",
        "revisions":[{"id":"harness-r1","author":"deterministic","createdAt":"2026-09-08T16:50:00+08:00","title":"Harness Evidence Gate","summary":"Validated release evidence."}]
    });
    json!({
        "status": status,
        "selectedDate": "2026-09-08",
        "isToday": true,
        "current": current,
        "timeline": [current],
        "privacy": {
            "rawRetentionHours": 48,
            "excludedApplications": ["1Password", "Keychain Access"],
            "localAiEnabled": false,
            "localApiEnabled": false,
            "mcpEnabled": false
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixture_state_is_typed_and_local_first() {
        let snapshot = fixture_snapshot(CollectionStatus::Paused);
        assert_eq!(snapshot["status"], "paused");
        assert_eq!(snapshot["privacy"]["localAiEnabled"], false);
        assert_eq!(snapshot["privacy"]["rawRetentionHours"], 48);
    }
}
