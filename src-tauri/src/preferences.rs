//! Small, non-sensitive local preferences that don't belong in the encrypted evidence database
//! (cosmetic choices, and a capture-detail selection that is a setting rather than evidence),
//! persisted as a plain JSON file in the app data directory.
//!
//! Every write goes through the whole document: saving one field must not silently drop another.

use openhistory_platform::CaptureDetail;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

/// How the macOS menu bar (status bar) icon is rendered.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrayIconStyle {
    /// A monochrome template icon that follows the system menu bar's
    /// light/dark appearance, matching native macOS status items.
    #[default]
    Monochrome,
    /// The brand-green glyph, rendered in full color.
    Color,
}

/// How much detail collection records per observation. Lower settings record strictly less: the
/// choice is about how much of the day is worth keeping, and it directly changes how much is
/// written to disk.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CaptureGranularity {
    /// Which application was in front, and nothing about what was open in it.
    Application,
    /// Application plus active-window metadata.
    #[default]
    Window,
    /// Adds bounded accessibility roles and semantic action categories.
    Semantic,
}

impl From<CaptureGranularity> for CaptureDetail {
    fn from(value: CaptureGranularity) -> Self {
        match value {
            CaptureGranularity::Application => Self::Application,
            CaptureGranularity::Window => Self::Window,
            CaptureGranularity::Semantic => Self::Semantic,
        }
    }
}

/// The fixed window the day timeline is grouped into.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TimelineBucket {
    /// Five-minute windows.
    FiveMinutes,
    /// Ten-minute windows.
    #[default]
    TenMinutes,
    /// Half-hour windows.
    ThirtyMinutes,
    /// Hourly windows.
    OneHour,
}

impl TimelineBucket {
    /// The window length this bucket groups by.
    #[must_use]
    pub fn duration(self) -> chrono::Duration {
        match self {
            Self::FiveMinutes => chrono::Duration::minutes(5),
            Self::TenMinutes => chrono::Duration::minutes(10),
            Self::ThirtyMinutes => chrono::Duration::minutes(30),
            Self::OneHour => chrono::Duration::hours(1),
        }
    }
}

/// The persisted preferences document.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredPreferences {
    /// Menu bar icon appearance.
    #[serde(default)]
    pub tray_icon_style: TrayIconStyle,
    /// How much detail collection records.
    #[serde(default)]
    pub capture_granularity: CaptureGranularity,
    /// How the day timeline is grouped.
    #[serde(default)]
    pub timeline_bucket: TimelineBucket,
    /// Whether the user had collection running. Restored at launch so that recording survives a
    /// restart; it records a choice already made, never a new one.
    #[serde(default)]
    pub collecting: bool,
}

fn preferences_path(app: &AppHandle) -> tauri::Result<std::path::PathBuf> {
    let directory = app.path().app_data_dir()?;
    Ok(directory.join("preferences.json"))
}

/// Loads the persisted preferences, falling back to defaults when absent or unreadable.
#[must_use]
pub fn load(app: &AppHandle) -> StoredPreferences {
    let Ok(path) = preferences_path(app) else {
        return StoredPreferences::default();
    };
    let Ok(contents) = std::fs::read_to_string(path) else {
        return StoredPreferences::default();
    };
    serde_json::from_str::<StoredPreferences>(&contents).unwrap_or_default()
}

/// Persists the full preferences document for future launches.
///
/// # Errors
///
/// Returns an error when the app data directory or preferences file cannot be written.
pub fn save(app: &AppHandle, preferences: StoredPreferences) -> tauri::Result<()> {
    let path = preferences_path(app)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let contents = serde_json::to_string(&preferences)
        .map_err(|error| tauri::Error::Io(std::io::Error::other(error)))?;
    std::fs::write(path, contents)?;
    Ok(())
}

/// Persists a new tray icon style, leaving every other preference untouched.
///
/// # Errors
///
/// Returns an error when the preferences file cannot be written.
pub fn save_tray_icon_style(app: &AppHandle, style: TrayIconStyle) -> tauri::Result<()> {
    save(
        app,
        StoredPreferences {
            tray_icon_style: style,
            ..load(app)
        },
    )
}

/// Persists a new capture granularity, leaving every other preference untouched.
///
/// # Errors
///
/// Returns an error when the preferences file cannot be written.
pub fn save_capture_granularity(
    app: &AppHandle,
    granularity: CaptureGranularity,
) -> tauri::Result<()> {
    save(
        app,
        StoredPreferences {
            capture_granularity: granularity,
            ..load(app)
        },
    )
}

/// Persists whether collection is running, leaving every other preference untouched.
///
/// # Errors
///
/// Returns an error when the preferences file cannot be written.
pub fn save_collecting(app: &AppHandle, collecting: bool) -> tauri::Result<()> {
    save(
        app,
        StoredPreferences {
            collecting,
            ..load(app)
        },
    )
}

/// Persists a new timeline bucket, leaving every other preference untouched.
///
/// # Errors
///
/// Returns an error when the preferences file cannot be written.
pub fn save_timeline_bucket(app: &AppHandle, bucket: TimelineBucket) -> tauri::Result<()> {
    save(
        app,
        StoredPreferences {
            timeline_bucket: bucket,
            ..load(app)
        },
    )
}
