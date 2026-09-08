use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tauri::{AppHandle, Emitter, Manager, State};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CollectionStatus {
    #[default]
    Recording,
    Paused,
    PermissionNeeded,
    Error,
}

#[derive(Default)]
pub struct DashboardState {
    status: Mutex<CollectionStatus>,
}

#[tauri::command]
pub fn get_dashboard(state: State<'_, DashboardState>) -> Result<Value, String> {
    let status = *state
        .status
        .lock()
        .map_err(|_| "collection state is unavailable".to_owned())?;
    Ok(fixture_snapshot(status))
}

#[tauri::command]
pub fn set_collection_status(
    status: CollectionStatus,
    state: State<'_, DashboardState>,
    app: AppHandle,
) -> Result<CollectionStatus, String> {
    if matches!(
        status,
        CollectionStatus::PermissionNeeded | CollectionStatus::Error
    ) {
        return Err("collection cannot be started until permission is restored".to_owned());
    }
    *state
        .status
        .lock()
        .map_err(|_| "collection state is unavailable".to_owned())? = status;
    let _ = app.emit("collection-status-changed", status);
    Ok(status)
}
#[tauri::command]
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
            "externalAiEnabled": false,
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
        assert_eq!(snapshot["privacy"]["externalAiEnabled"], false);
        assert_eq!(snapshot["privacy"]["rawRetentionHours"], 48);
    }
}
