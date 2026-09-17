use std::{
    collections::{BTreeMap, btree_map::Entry},
    sync::{Arc, Mutex},
};

use chrono::{DateTime, FixedOffset, Local, NaiveDate};
use openhistory_segmentation::{SegmentConfidence, SegmentTitle, TaskSegment};
use openhistory_summaries::{Summarizer, SummaryRequest};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tauri::{AppHandle, Emitter, Manager, State};

use crate::preferences::TimelineBucket;

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

/// How much recent history a deletion request covers. Deliberately a closed set: a scope is a
/// promise about what disappears, so it is the type system that keeps it honest rather than a
/// string compared at the boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HistoryScope {
    /// The last ten minutes of activity.
    Last10Minutes,
    /// The last hour of activity.
    LastHour,
    /// Everything recorded since local midnight.
    Today,
    /// The entire retained history.
    All,
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
/// Returns the least-sensitive dashboard projection for one local day.
///
/// `date` is `YYYY-MM-DD` in the machine's own time zone, and defaults to today.
///
/// # Errors
///
/// Returns an error when `date` is not a date, or the collection state lock is unavailable.
pub fn get_dashboard(
    date: Option<String>,
    state: State<'_, DashboardState>,
    runtime: State<'_, crate::runtime::CollectorRuntime>,
    app: AppHandle,
) -> Result<Value, String> {
    let day = requested_day(date.as_deref())?;
    let status = *state
        .status
        .lock()
        .map_err(|_| "collection state is unavailable".to_owned())?;
    let segments = match runtime.segments_for(day) {
        Ok(segments) => segments,
        // Only "not open yet" is allowed to degrade, and it degrades into `storageReady: false`
        // below rather than into silence. A real storage failure is still an error the user sees.
        Err(openhistory_storage::StorageError::NotInitialized) => Vec::new(),
        Err(error) => return Err(error.to_string()),
    };
    let bucket = crate::preferences::load(&app).timeline_bucket;
    let mut snapshot = day_snapshot(status, &segments, bucket, day);
    // Storage opens on a background thread, so early frames can arrive before the key is
    // available — and if the credential store is asking for a password, that lasts as long as the
    // user takes to answer. Saying the day is empty would be a claim about the day; this says only
    // what is true, that nothing has been read yet.
    snapshot["storageReady"] = json!(runtime.storage_ready());
    snapshot["recordedEventCount"] = json!(runtime.event_count());
    snapshot["storageBytes"] = json!(runtime.storage_bytes());
    Ok(snapshot)
}

/// Resolves which local day a caller asked for.
///
/// An unparseable date is refused rather than quietly answered with today's history under the
/// requested day's heading, which would caption one day's work with another's date.
fn requested_day(date: Option<&str>) -> Result<NaiveDate, String> {
    match date {
        None => Ok(Local::now().date_naive()),
        Some(value) => NaiveDate::parse_from_str(value, "%Y-%m-%d")
            .map_err(|_| format!("{value} is not a date")),
    }
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
        // Collecting into storage that is not open yet would drop every event on the floor and
        // report Recording while doing it. Refusing says what is actually happening.
        CollectionStatus::Recording if !runtime.storage_ready() => {
            return Err("encrypted storage is still unlocking".to_owned());
        }
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
    // Remember the choice, so a restart resumes it rather than silently recording nothing. Only a
    // settled state is worth restoring: coming back up in PermissionNeeded or Error would arm
    // collection against a machine that already refused it.
    if matches!(
        resolved,
        CollectionStatus::Recording | CollectionStatus::Paused
    ) {
        let _ = crate::preferences::save_collecting(
            &app,
            matches!(resolved, CollectionStatus::Recording),
        );
    }
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
/// Deletes the requested span of history from encrypted storage and returns how many raw events
/// were removed. The surfaces are told afterwards so they can drop what they were showing.
///
/// # Errors
///
/// Returns an error when the deletion cannot complete.
pub fn delete_history(
    scope: HistoryScope,
    runtime: State<'_, crate::runtime::CollectorRuntime>,
    app: AppHandle,
) -> Result<usize, String> {
    let deleted = runtime
        .delete_history(scope)
        .map_err(|error| error.to_string())?;
    let _ = app.emit("history-deleted", scope);
    Ok(deleted)
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
/// Returns the persisted menu bar icon style.
pub fn get_tray_icon_style(app: AppHandle) -> crate::preferences::TrayIconStyle {
    crate::preferences::load(&app).tray_icon_style
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

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
/// Returns the persisted capture granularity.
pub fn get_capture_granularity(app: AppHandle) -> crate::preferences::CaptureGranularity {
    crate::preferences::load(&app).capture_granularity
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
/// Persists a new capture granularity and applies it to collection that is already running.
///
/// # Errors
///
/// Returns an error when the preference cannot be saved or collection cannot be restarted.
pub async fn set_capture_granularity(
    granularity: crate::preferences::CaptureGranularity,
    runtime: State<'_, crate::runtime::CollectorRuntime>,
    app: AppHandle,
) -> Result<crate::preferences::CaptureGranularity, String> {
    crate::preferences::save_capture_granularity(&app, granularity)
        .map_err(|error| error.to_string())?;
    runtime
        .set_capture_detail(granularity.into(), &app)
        .await
        .map_err(|error| error.to_string())?;
    Ok(granularity)
}

/// What a local coding agent made of one window's evidence, and who was asked.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Interpretation {
    /// The command that answered, so the answer is attributable.
    pub agent: String,
    /// The agent's title for the window.
    pub title: String,
    /// The agent's account of what was going on.
    pub summary: String,
}

/// Assembles exactly what a local agent is allowed to see about one window.
///
/// Nothing here is inferred: every line states an observation and the attention it held. The agent
/// is given the same evidence the user is already looking at and nothing more, so its answer can be
/// checked against what is on screen.
fn interpretation_request(entry: &BucketedActivity, id: &str) -> SummaryRequest {
    let mut evidence = vec![format!(
        "Observed from {} to {}, holding attention for {} seconds in total.",
        entry.started_at.format("%H:%M"),
        entry.ended_at.format("%H:%M"),
        entry.observed_seconds.max(0)
    )];
    for held in &entry.titles {
        evidence.push(format!(
            "Window titled \"{}\" held attention for {} seconds.",
            held.title,
            held.observed_seconds.max(0)
        ));
    }
    for application in &entry.applications {
        evidence.push(format!("Application \"{application}\" was in front."));
    }
    if let Some(project) = &entry.project_id {
        evidence.push(format!("Work resolved to the project \"{project}\"."));
    }
    SummaryRequest {
        segment_revision: format!("{id}-r1"),
        fallback_title: entry
            .titles
            .first()
            .map_or_else(|| "Desktop activity".to_owned(), |held| held.title.clone()),
        applications: entry.applications.clone(),
        evidence,
    }
}

#[tauri::command]
/// Asks a coding agent installed on this machine what one timeline row was about.
///
/// Runs only when the user asks. The agent it finds is a CLI the user already installed and signed
/// in, which means the evidence leaves this machine for that agent's vendor — the surface that
/// offers this has to say so before it is clicked. The answer is checked for grounding before it
/// comes back, so an agent that names something the evidence never showed is refused rather than
/// displayed.
///
/// # Errors
///
/// Returns an error when no agent is installed, `date` is not a date, the row is no longer in that
/// day's history, the agent cannot be run or times out, or its answer is not grounded in the
/// evidence it was given.
pub async fn interpret_activity(
    segment_id: String,
    date: Option<String>,
    runtime: State<'_, crate::runtime::CollectorRuntime>,
    app: AppHandle,
) -> Result<Interpretation, String> {
    let agent = crate::agent::LocalAgent::detect()
        .ok_or_else(|| "no coding agent command was found on this Mac".to_owned())?;
    let day = requested_day(date.as_deref())?;
    let segments = runtime
        .segments_for(day)
        .map_err(|error| error.to_string())?;
    let bucket = crate::preferences::load(&app).timeline_bucket;
    let entry = bucketed(&segments, bucket, None)
        .into_iter()
        .find(|entry| window_id(entry) == segment_id)
        .ok_or_else(|| "that stretch is no longer in this day's history".to_owned())?;
    let request = interpretation_request(&entry, &segment_id);
    let output = agent
        .summarize(&request)
        .await
        .map_err(|error| error.to_string())?;
    Ok(Interpretation {
        agent: agent.name(),
        title: output.title,
        summary: output.summary,
    })
}

/// Builds one day's dashboard projection from deterministically segmented, already-persisted
/// events. Segments carry only what was actually observed: an unresolved project stays
/// "General" rather than guessing, and there is no generated text anywhere in this path.
fn day_snapshot(
    status: CollectionStatus,
    segments: &[TaskSegment],
    bucket: TimelineBucket,
    day: NaiveDate,
) -> Value {
    let now = Local::now();
    let ongoing_boundary = chrono::Duration::minutes(5);
    let is_today = day == now.date_naive();
    // Only a recording collector looking at today can honestly extend its newest observation to the
    // present moment. Doing it on a past day would grow a finished stretch by every hour since.
    let open_end = if status == CollectionStatus::Recording && is_today {
        Some(now.fixed_offset())
    } else {
        None
    };
    let buckets = bucketed(segments, bucket, open_end);
    let is_ongoing = |entry: &BucketedActivity| {
        status == CollectionStatus::Recording
            && is_today
            && now.signed_duration_since(entry.ended_at) <= ongoing_boundary
    };

    let mut timeline: Vec<Value> = buckets
        .iter()
        .map(|entry| activity_segment_json(entry, is_ongoing(entry)))
        .collect();
    timeline.reverse();

    let current = buckets
        .last()
        .filter(|entry| is_ongoing(entry))
        .map(|entry| activity_segment_json(entry, true));

    json!({
        "status": status,
        "selectedDate": day.format("%Y-%m-%d").to_string(),
        "isToday": is_today,
        "current": current,
        "timeline": timeline,
        "privacy": {
            // No retention window is reported because none is enforced: nothing in this app runs a
            // retention sweep, so a number here would be a promise the code does not keep.
            //
            // The excluded list is read from the policy the collector actually installs, not
            // restated here. Three flags — a local model, a loopback API and an MCP companion —
            // also used to be reported, hardcoded false, to feed switches that persisted nothing
            // and features that were never built. The switches are gone, so the flags are too.
            "excludedApplications": crate::runtime::EXCLUDED_APPLICATIONS,
        }
    })
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
/// Returns the persisted timeline grouping window.
pub fn get_timeline_bucket(app: AppHandle) -> TimelineBucket {
    crate::preferences::load(&app).timeline_bucket
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
/// Persists a new timeline grouping window.
///
/// # Errors
///
/// Returns an error when the preference cannot be saved.
pub fn set_timeline_bucket(
    bucket: TimelineBucket,
    app: AppHandle,
) -> Result<TimelineBucket, String> {
    crate::preferences::save_timeline_bucket(&app, bucket).map_err(|error| error.to_string())?;
    Ok(bucket)
}

/// One timeline row: every segment that fell inside the same fixed window, merged.
///
/// Continuity segmentation alone produces a row per uninterrupted stretch of one context, which
/// on a real desktop means dozens of few-second rows an hour — accurate, unreadable, and mostly
/// zero-minute. Collapsing them into a window gives the timeline a stable rhythm while keeping
/// every number measured: attention is the summed observed time, never the width of the window.
struct BucketedActivity {
    window_start: DateTime<FixedOffset>,
    started_at: DateTime<FixedOffset>,
    ended_at: DateTime<FixedOffset>,
    observed_seconds: i64,
    applications: Vec<String>,
    titles: Vec<SegmentTitle>,
    project_id: Option<String>,
    confidence: SegmentConfidence,
}

/// Folds another segment's titles into a window's, summing the attention each title held and
/// keeping the strongest first.
///
/// The same window is normally seen in several segments of one bucket, so the totals have to add up
/// rather than the lists concatenate — otherwise the thing someone spent an hour on ranks below
/// whatever they opened first.
fn merge_titles(into: &mut Vec<SegmentTitle>, from: &[SegmentTitle]) {
    for incoming in from {
        if let Some(existing) = into.iter_mut().find(|held| held.title == incoming.title) {
            existing.observed_seconds += incoming.observed_seconds;
        } else {
            into.push(incoming.clone());
        }
    }
    into.sort_by_key(|held| -held.observed_seconds);
    into.truncate(5);
}

/// A merged window is only as trustworthy as its least certain contributor. `SegmentConfidence`
/// deliberately has no ordering of its own — deriving one would rank it by declaration order,
/// which reads backwards — so the comparison is spelled out here.
fn weaker_confidence(left: SegmentConfidence, right: SegmentConfidence) -> SegmentConfidence {
    let rank = |value: SegmentConfidence| match value {
        SegmentConfidence::Low => 0_u8,
        SegmentConfidence::Medium => 1,
        SegmentConfidence::High => 2,
    };
    if rank(right) < rank(left) {
        right
    } else {
        left
    }
}

fn bucketed(
    segments: &[TaskSegment],
    bucket: TimelineBucket,
    open_end: Option<DateTime<FixedOffset>>,
) -> Vec<BucketedActivity> {
    let window = bucket.duration();
    let window_seconds = window.num_seconds().max(1);
    let mut buckets: BTreeMap<i64, BucketedActivity> = BTreeMap::new();

    for segment in segments {
        let key = segment.started_at.timestamp().div_euclid(window_seconds);
        // Segmentation measured how long each observation held. The only part it cannot know is the
        // newest observation's ending, because nothing has replaced it yet — so a still-recording
        // collector extends that one to now, and a stopped one leaves it where the evidence ends.
        let mut observed = segment.observed_seconds.max(0);
        if segment.open_ended
            && let Some(now) = open_end
        {
            observed += now
                .signed_duration_since(segment.ended_at)
                .num_seconds()
                .max(0);
        }
        let observed = observed;
        match buckets.entry(key) {
            Entry::Vacant(slot) => {
                let window_start = DateTime::from_timestamp(key * window_seconds, 0)
                    .map_or(segment.started_at, |value| {
                        value.with_timezone(&segment.started_at.timezone())
                    });
                slot.insert(BucketedActivity {
                    window_start,
                    started_at: segment.started_at,
                    ended_at: segment.ended_at,
                    observed_seconds: observed,
                    applications: segment.applications.clone(),
                    titles: segment.titles.clone(),
                    project_id: segment.project_id.clone(),
                    confidence: segment.confidence,
                });
            }
            Entry::Occupied(mut slot) => {
                let entry = slot.get_mut();
                entry.started_at = entry.started_at.min(segment.started_at);
                entry.ended_at = entry.ended_at.max(segment.ended_at);
                entry.observed_seconds += observed;
                for application in &segment.applications {
                    if !entry.applications.contains(application) {
                        entry.applications.push(application.clone());
                    }
                }
                merge_titles(&mut entry.titles, &segment.titles);
                // A window that spans two projects belongs to neither: say so rather than
                // attributing the whole window to whichever one happened to be first.
                if entry.project_id != segment.project_id {
                    entry.project_id = None;
                }
                entry.confidence = weaker_confidence(entry.confidence, segment.confidence);
            }
        }
    }

    buckets.into_values().collect()
}

/// Names a timeline row after the window it covers, so the same row keeps the same identity
/// between snapshots and can be asked about later.
fn window_id(entry: &BucketedActivity) -> String {
    format!("window-{}", entry.window_start.timestamp())
}

fn activity_segment_json(entry: &BucketedActivity, is_ongoing: bool) -> Value {
    let duration_minutes = entry.observed_seconds / 60;
    let confidence = match entry.confidence {
        SegmentConfidence::High => "high",
        SegmentConfidence::Medium => "medium",
        SegmentConfidence::Low => "low",
    };
    let category = if entry.project_id.is_some() {
        "Development"
    } else {
        "General"
    };
    let sources: Vec<Value> = entry
        .applications
        .iter()
        .map(|name| source_for_application(name))
        .collect();
    // Name the window after the most specific thing actually observed in it: the window someone
    // spent the most time in, then the project when one resolved, then the applications themselves.
    // A title says which file or page; an application list only says which program was in front.
    let title = entry
        .titles
        .first()
        .map(|held| held.title.clone())
        .or_else(|| entry.project_id.clone())
        .unwrap_or_else(|| match entry.applications.as_slice() {
            [] => "Unidentified activity".to_owned(),
            [single] => single.clone(),
            [first, rest @ ..] => format!("{first} +{}", rest.len()),
        });
    let places = match entry.applications.as_slice() {
        [] => "Activity was recorded without identifying application detail.".to_owned(),
        [single] => format!("Worked in {single}."),
        values => format!("Worked across {}.", values.join(", ")),
    };
    // What was open, with the time each held, is the part a reader can actually act on. It is listed
    // rather than interpreted: naming an intent behind these windows would be a guess, and the
    // summary is the one place that has to stay strictly what was seen.
    let summary = if entry.titles.is_empty() {
        places
    } else {
        let opened = entry
            .titles
            .iter()
            .map(|held| {
                let minutes = held.observed_seconds / 60;
                if minutes > 0 {
                    format!("{} ({minutes} min)", held.title)
                } else {
                    held.title.clone()
                }
            })
            .collect::<Vec<_>>()
            .join(" · ");
        format!("{places} Open: {opened}.")
    };
    let id = window_id(entry);

    json!({
        "id": id,
        "title": title,
        "summary": summary,
        "start": entry.started_at.format("%H:%M").to_string(),
        "end": entry.ended_at.format("%H:%M").to_string(),
        "durationMinutes": duration_minutes,
        // The raw seconds travel alongside the minutes so the UI can distinguish "nothing was
        // observed" from "less than a minute was observed". Truncating to minutes in the backend
        // and rendering the result verbatim reported both as a flat "0 min".
        "observedSeconds": entry.observed_seconds,
        "sources": sources,
        "state": if is_ongoing { "current" } else { "complete" },
        "category": category,
        "confidence": confidence,
        "origin": "native",
        "revisions": [{
            "id": format!("{id}-r1"),
            "author": "deterministic",
            "createdAt": entry.started_at.to_rfc3339(),
            "title": title,
            "summary": summary
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

    /// Most of these tests are about today, which is what the surface opens on.
    fn today_snapshot(
        status: CollectionStatus,
        segments: &[TaskSegment],
        bucket: TimelineBucket,
    ) -> Value {
        day_snapshot(status, segments, bucket, Local::now().date_naive())
    }

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

    /// The projection may only report privacy facts the code enforces.
    ///
    /// It used to also send `localAiEnabled`, `localApiEnabled` and `mcpEnabled`, all hardcoded
    /// false, feeding three settings switches that persisted nothing and stood for features that do
    /// not exist — no HTTP listener is built, and the MCP binary prints one line and exits. The
    /// excluded list was a second hardcoded copy of the collector's policy, and had already drifted
    /// one entry behind it.
    #[test]
    fn the_privacy_projection_reports_only_what_is_enforced() {
        let snapshot = today_snapshot(CollectionStatus::Paused, &[], TimelineBucket::default());
        let privacy = snapshot["privacy"].as_object().expect("privacy object");

        assert_eq!(
            privacy["excludedApplications"],
            json!(crate::runtime::EXCLUDED_APPLICATIONS),
            "the card names what the collector excludes, so it has to read the same list"
        );
        assert_eq!(
            privacy.len(),
            1,
            "no unbacked flag may reappear here: {privacy:?}"
        );
    }

    #[test]
    fn empty_segments_produce_no_activity_and_a_real_date() {
        let snapshot = today_snapshot(CollectionStatus::Paused, &[], TimelineBucket::default());
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
        let snapshot = today_snapshot(
            CollectionStatus::Paused,
            &segments,
            TimelineBucket::default(),
        );
        let timeline = snapshot["timeline"].as_array().expect("timeline array");
        assert_eq!(timeline.len(), 1);
        assert_eq!(timeline[0]["category"], "Development");
        // The window title outranks the project name: both are grounded in the same event, and only
        // one of them says what was being worked on.
        assert_eq!(timeline[0]["title"], "main.rs");
        assert!(
            timeline[0]["summary"]
                .as_str()
                .expect("summary")
                .contains("main.rs"),
            "the summary has to name what was open, not only where"
        );
        assert_eq!(timeline[0]["sources"][0]["name"], "Visual Studio Code");
        assert_eq!(timeline[0]["sources"][0]["kind"], "editor");
        assert_eq!(timeline[0]["state"], "complete");
    }

    /// A day that has ended has nothing in progress in it. Recording says something about now, and
    /// carrying that into a past day would report a finished stretch as still running and keep
    /// growing it by every hour since.
    #[test]
    fn a_past_day_is_never_recording_in_progress() {
        let events = vec![window_event(0, None)];
        let segments = segment_events(&events, SegmentationSettings::default());
        let yesterday = Local::now()
            .date_naive()
            .pred_opt()
            .expect("yesterday exists");

        let snapshot = day_snapshot(
            CollectionStatus::Recording,
            &segments,
            TimelineBucket::default(),
            yesterday,
        );
        assert_eq!(snapshot["isToday"], false);
        assert_eq!(snapshot["selectedDate"], yesterday.to_string());
        assert!(
            snapshot["current"].is_null(),
            "a past day cannot have something in progress"
        );
    }

    /// Answering an unparseable date with today's history under that date's heading would caption
    /// one day's work with another's.
    #[test]
    fn a_date_that_is_not_a_date_is_refused() {
        assert_eq!(
            requested_day(None).expect("today always parses"),
            Local::now().date_naive()
        );
        assert_eq!(
            requested_day(Some("2026-09-16")).expect("an ISO date parses"),
            NaiveDate::from_ymd_opt(2026, 9, 16).expect("a real date")
        );
        assert!(requested_day(Some("last tuesday")).is_err());
    }

    #[test]
    fn a_recent_segment_is_current_only_while_recording() {
        let events = vec![window_event(0, None)];
        let segments = segment_events(&events, SegmentationSettings::default());

        let paused = today_snapshot(
            CollectionStatus::Paused,
            &segments,
            TimelineBucket::default(),
        );
        assert!(paused["current"].is_null());
        assert_eq!(paused["timeline"][0]["category"], "General");

        let recording = today_snapshot(
            CollectionStatus::Recording,
            &segments,
            TimelineBucket::default(),
        );
        assert_eq!(recording["current"]["state"], "current");
        assert_eq!(recording["timeline"][0]["state"], "current");
    }

    fn segment(
        started_minutes: i64,
        ended_minutes: i64,
        applications: &[&str],
        project_id: Option<&str>,
    ) -> TaskSegment {
        let base = DateTime::parse_from_rfc3339("2026-09-17T09:00:00+08:00").unwrap();
        TaskSegment {
            segment_id: format!("segment-{started_minutes}"),
            started_at: base + chrono::Duration::minutes(started_minutes),
            ended_at: base + chrono::Duration::minutes(ended_minutes),
            event_ids: Vec::new(),
            applications: applications
                .iter()
                .map(|value| (*value).to_owned())
                .collect(),
            titles: Vec::new(),
            project_id: project_id.map(ToOwned::to_owned),
            confidence: SegmentConfidence::Medium,
            // These tests are about how windows merge, not about how dwell is measured, so the
            // segment is credited with its own span. Measuring dwell is covered in segmentation.
            observed_seconds: (ended_minutes - started_minutes) * 60,
            open_ended: false,
        }
    }

    #[test]
    fn segments_inside_one_window_collapse_into_a_single_row() {
        // Three short stretches inside the same ten minutes, which would otherwise be three rows
        // reading "0 min" each.
        let segments = vec![
            segment(0, 2, &["Ghostty"], None),
            segment(3, 6, &["Chrome"], None),
            segment(7, 9, &["Ghostty"], None),
        ];

        let snapshot = today_snapshot(
            CollectionStatus::Paused,
            &segments,
            TimelineBucket::TenMinutes,
        );
        let timeline = snapshot["timeline"].as_array().expect("timeline");
        assert_eq!(timeline.len(), 1, "one window is one row");
        let row = &timeline[0];
        assert_eq!(row["start"], "09:00");
        assert_eq!(row["end"], "09:09");
        // Attention is the summed observed time (2 + 3 + 2), never the width of the window.
        assert_eq!(row["durationMinutes"], 7);
        assert_eq!(row["sources"].as_array().unwrap().len(), 2);
        assert_eq!(row["title"], "Ghostty +1");
    }

    #[test]
    fn a_narrower_window_keeps_the_same_activity_in_separate_rows() {
        let segments = vec![
            segment(0, 2, &["Ghostty"], None),
            segment(7, 9, &["Ghostty"], None),
        ];

        let wide = today_snapshot(
            CollectionStatus::Paused,
            &segments,
            TimelineBucket::TenMinutes,
        );
        assert_eq!(wide["timeline"].as_array().unwrap().len(), 1);

        let narrow = today_snapshot(
            CollectionStatus::Paused,
            &segments,
            TimelineBucket::FiveMinutes,
        );
        assert_eq!(narrow["timeline"].as_array().unwrap().len(), 2);
        // Newest first, matching how the surfaces read.
        assert_eq!(narrow["timeline"][0]["start"], "09:07");
    }

    #[test]
    fn a_window_spanning_two_projects_is_attributed_to_neither() {
        let segments = vec![
            segment(0, 3, &["Code"], Some("open-history")),
            segment(4, 8, &["Code"], Some("timetrace")),
        ];

        let snapshot = today_snapshot(
            CollectionStatus::Paused,
            &segments,
            TimelineBucket::TenMinutes,
        );
        let row = &snapshot["timeline"][0];
        assert_eq!(
            row["category"], "General",
            "no single project owns the window"
        );
        assert_eq!(row["title"], "Code");
    }
}
