use std::sync::{Arc, Mutex};

use chrono::Local;
use openhistory_segmentation::{SegmentConfidence, TaskSegment};
use openhistory_summaries::deterministic_summary;
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
    let segments = runtime
        .today_segments()
        .map_err(|error| error.to_string())?;
    let mut snapshot = today_snapshot(status, &segments);
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
/// The app runs as a macOS accessory (menu bar only), which its floating compact panel is happy
/// with but a regular window is not: an accessory app never becomes active, so the history
/// window renders while silently ignoring every click. Becoming a regular app for as long as
/// that window is open is what makes it accept input; `crate::windows` restores the accessory
/// policy once it closes.
///
/// # Errors
///
/// Returns an error when the history window is absent or cannot be shown or focused.
pub fn open_history_window(app: AppHandle, segment_id: Option<String>) -> Result<(), String> {
    let window = app
        .get_webview_window("history")
        .ok_or_else(|| "history window was not created".to_owned())?;
    #[cfg(target_os = "macos")]
    app.set_activation_policy(tauri::ActivationPolicy::Regular)
        .map_err(|error| error.to_string())?;
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

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
/// Returns the persisted menu bar icon style.
pub fn get_tray_icon_style(app: AppHandle) -> crate::preferences::TrayIconStyle {
    crate::preferences::load_tray_icon_style(&app)
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
/// Persists a new menu bar icon style and applies it to the live tray icon.
///
/// # Errors
///
/// Returns an error when the preference cannot be saved or the tray icon cannot be updated.
pub fn set_tray_icon_style(
    style: crate::preferences::TrayIconStyle,
    app: AppHandle,
) -> Result<crate::preferences::TrayIconStyle, String> {
    crate::preferences::save_tray_icon_style(&app, style).map_err(|error| error.to_string())?;
    crate::tray::apply_style(&app, style).map_err(|error| error.to_string())?;
    Ok(style)
}

/// Builds today's dashboard projection from deterministically segmented, already-persisted
/// events. Segments carry only what was actually observed: an unresolved project stays
/// "General" rather than guessing, and there is no generated text anywhere in this path.
fn today_snapshot(status: CollectionStatus, segments: &[TaskSegment]) -> Value {
    let now = Local::now();
    let ongoing_boundary = chrono::Duration::minutes(5);
    let is_ongoing = |segment: &TaskSegment| {
        status == CollectionStatus::Recording
            && now.signed_duration_since(segment.ended_at) <= ongoing_boundary
    };

    let mut timeline: Vec<Value> = segments
        .iter()
        .map(|segment| activity_segment_json(segment, is_ongoing(segment)))
        .collect();
    timeline.reverse();

    let current = segments
        .last()
        .filter(|segment| is_ongoing(segment))
        .map(|segment| activity_segment_json(segment, true));

    json!({
        "status": status,
        "selectedDate": now.format("%Y-%m-%d").to_string(),
        "isToday": true,
        "current": current,
        "timeline": timeline,
        "privacy": {
            "rawRetentionHours": 48,
            "excludedApplications": ["1Password", "Keychain Access"],
            "localAiEnabled": false,
            "localApiEnabled": false,
            "mcpEnabled": false
        }
    })
}

fn activity_segment_json(segment: &TaskSegment, is_ongoing: bool) -> Value {
    let summary = deterministic_summary(segment);
    let duration_minutes = summary.duration_seconds / 60;
    let confidence = match segment.confidence {
        SegmentConfidence::High => "high",
        SegmentConfidence::Medium => "medium",
        SegmentConfidence::Low => "low",
    };
    let category = if segment.project_id.is_some() {
        "Development"
    } else {
        "General"
    };
    let sources: Vec<Value> = segment
        .applications
        .iter()
        .map(|name| source_for_application(name))
        .collect();

    json!({
        "id": segment.segment_id,
        "title": summary.title,
        "summary": summary.outline,
        "start": segment.started_at.format("%H:%M").to_string(),
        "end": segment.ended_at.format("%H:%M").to_string(),
        "durationMinutes": duration_minutes,
        "sources": sources,
        "state": if is_ongoing { "current" } else { "complete" },
        "category": category,
        "confidence": confidence,
        "origin": "native",
        "revisions": [{
            "id": format!("{}-r1", segment.segment_id),
            "author": "deterministic",
            "createdAt": segment.started_at.to_rfc3339(),
            "title": summary.title,
            "summary": summary.outline
        }]
    })
}

/// Classifies a captured application display name into a source kind and accent color for
/// presentation. Falls back to a neutral "system" kind rather than guessing at an unknown
/// application's purpose.
fn source_for_application(display_name: &str) -> Value {
    let lower = display_name.to_lowercase();
    let (kind, color) = if lower.contains("code")
        || lower.contains("xcode")
        || lower.contains("intellij")
        || lower.contains("vim")
    {
        ("editor", "#5d8fe7")
    } else if lower.contains("terminal") || lower.contains("iterm") || lower.contains("ghostty") {
        ("terminal", "#252a2d")
    } else if lower.contains("codex") || lower.contains("claude") || lower.contains("copilot") {
        ("agent", "#eff6f2")
    } else if lower.contains("chrome")
        || lower.contains("safari")
        || lower.contains("firefox")
        || lower.contains("arc")
    {
        ("browser", "#e9edf3")
    } else if lower.contains("notes") || lower.contains("notion") || lower.contains("word") {
        ("document", "#f5d46b")
    } else {
        ("system", "#c8cdd6")
    };

    json!({
        "id": slugify(display_name),
        "name": display_name,
        "kind": kind,
        "color": color
    })
}

fn slugify(value: &str) -> String {
    value
        .to_lowercase()
        .chars()
        .map(|character| {
            if character.is_alphanumeric() {
                character
            } else {
                '-'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use openhistory_domain::{
        AdapterKind, ApplicationIdentity, CaptureQuality, EventEnvelope, SemanticPayload,
        SourceIdentity,
    };
    use openhistory_segmentation::{SegmentationSettings, segment_events};

    fn window_event(minutes_offset: i64, project_id: Option<&str>) -> EventEnvelope {
        let occurred_at = Local::now().fixed_offset() + chrono::Duration::minutes(minutes_offset);
        EventEnvelope::new(
            occurred_at,
            u64::try_from(minutes_offset + 1000).unwrap_or_default(),
            SourceIdentity {
                source_id: "macos-accessibility".to_owned(),
                adapter: AdapterKind::MacOsAccessibility,
                application: ApplicationIdentity {
                    display_name: Some("Visual Studio Code".to_owned()),
                    platform_id: Some("com.microsoft.VSCode".to_owned()),
                    process_id: None,
                },
            },
            CaptureQuality::Window,
            SemanticPayload::WindowChanged {
                window_title: Some("main.rs".to_owned()),
                project_id: project_id.map(ToOwned::to_owned),
            },
        )
    }

    #[test]
    fn empty_segments_produce_no_activity_and_a_real_date() {
        let snapshot = today_snapshot(CollectionStatus::Paused, &[]);
        assert_eq!(snapshot["status"], "paused");
        assert_eq!(snapshot["timeline"], json!([]));
        assert!(snapshot["current"].is_null());
        assert_eq!(
            snapshot["selectedDate"],
            Local::now().format("%Y-%m-%d").to_string()
        );
    }

    #[test]
    fn a_segment_with_a_resolved_project_is_grounded_in_its_own_evidence() {
        let events = vec![window_event(0, Some("open-history"))];
        let segments = segment_events(&events, SegmentationSettings::default());
        let snapshot = today_snapshot(CollectionStatus::Paused, &segments);
        let timeline = snapshot["timeline"].as_array().expect("timeline array");
        assert_eq!(timeline.len(), 1);
        assert_eq!(timeline[0]["category"], "Development");
        assert_eq!(timeline[0]["title"], "open-history");
        assert_eq!(timeline[0]["sources"][0]["name"], "Visual Studio Code");
        assert_eq!(timeline[0]["sources"][0]["kind"], "editor");
        assert_eq!(timeline[0]["state"], "complete");
    }

    #[test]
    fn a_recent_segment_is_current_only_while_recording() {
        let events = vec![window_event(0, None)];
        let segments = segment_events(&events, SegmentationSettings::default());

        let paused = today_snapshot(CollectionStatus::Paused, &segments);
        assert!(paused["current"].is_null());
        assert_eq!(paused["timeline"][0]["category"], "General");

        let recording = today_snapshot(CollectionStatus::Recording, &segments);
        assert_eq!(recording["current"]["state"], "current");
        assert_eq!(recording["timeline"][0]["state"], "current");
    }
}
