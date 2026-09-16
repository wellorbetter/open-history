//! Small, non-sensitive local UI preferences that don't belong in the
//! encrypted evidence database (e.g. cosmetic choices with no privacy
//! surface), persisted as a plain JSON file in the app data directory.

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

#[derive(Serialize, Deserialize)]
struct StoredPreferences {
    #[serde(default)]
    tray_icon_style: TrayIconStyle,
}

fn preferences_path(app: &AppHandle) -> tauri::Result<std::path::PathBuf> {
    let directory = app.path().app_data_dir()?;
    Ok(directory.join("preferences.json"))
}

/// Loads the persisted tray icon style, or the default when absent or unreadable.
pub fn load_tray_icon_style(app: &AppHandle) -> TrayIconStyle {
    let Ok(path) = preferences_path(app) else {
        return TrayIconStyle::default();
    };
    let Ok(contents) = std::fs::read_to_string(path) else {
        return TrayIconStyle::default();
    };
    serde_json::from_str::<StoredPreferences>(&contents)
        .map(|preferences| preferences.tray_icon_style)
        .unwrap_or_default()
}

/// Persists the tray icon style for future launches.
///
/// # Errors
///
/// Returns an error when the app data directory or preferences file cannot be written.
pub fn save_tray_icon_style(app: &AppHandle, style: TrayIconStyle) -> tauri::Result<()> {
    let path = preferences_path(app)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let contents = serde_json::to_string(&StoredPreferences {
        tray_icon_style: style,
    })
    .map_err(|error| tauri::Error::Io(std::io::Error::other(error)))?;
    std::fs::write(path, contents)?;
    Ok(())
}
