use std::sync::atomic::{AtomicBool, Ordering};

use tauri::{
    App, AppHandle, Manager, PhysicalPosition,
    image::Image,
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};

use crate::preferences::TrayIconStyle;

/// Authoritative visibility state for the compact panel, plus a one-shot blur suppressor.
///
/// `WebviewWindow::is_visible` cannot drive the tray toggle: a borderless, transparent macOS
/// window that was ordered out still reports itself visible in some states, which strands the
/// toggle in its hide branch and leaves the panel permanently unopenable. Every show and hide
/// goes through this state instead, so the toggle always knows what it last did.
///
/// `ignore_next_blur` covers the other half of the same interaction: clicking the tray icon
/// while the panel has focus makes macOS resign its key-window status, and that blur would
/// otherwise race the click's own toggle. A blur arriving right after a tray click is a side
/// effect of that click, not a genuine click-elsewhere dismissal, so it is dropped once.
#[derive(Default)]
pub struct CompactPanel {
    shown: AtomicBool,
    ignore_next_blur: AtomicBool,
}

impl CompactPanel {
    fn is_shown(&self) -> bool {
        self.shown.load(Ordering::Acquire)
    }

    fn set_shown(&self, shown: bool) {
        self.shown.store(shown, Ordering::Release);
    }

    /// Records that a blur immediately following a tray click should not auto-hide the panel.
    fn arm_ignore_next_blur(&self) {
        self.ignore_next_blur.store(true, Ordering::Release);
    }

    fn disarm_ignore_next_blur(&self) {
        self.ignore_next_blur.store(false, Ordering::Release);
    }

    /// Consumes the flag: returns whether a blur happening right now should be ignored.
    pub fn consume_ignored_blur(&self) -> bool {
        self.ignore_next_blur.swap(false, Ordering::AcqRel)
    }

    /// Records that the panel was hidden by something other than the tray toggle.
    pub fn note_hidden(&self) {
        self.set_shown(false);
    }
}

/// A monochrome sparkle glyph on a transparent background, distinct from the
/// full-color, fully-opaque app icon: macOS template mode discards color and
/// keeps only alpha as a mask, so using the app icon there renders as a
/// featureless solid block instead of a glyph.
const MONOCHROME_ICON_BYTES: &[u8] = include_bytes!("../icons/tray-icon.png");

/// The app's own brand mark (light green rounded badge, dark sparkle), for
/// users who prefer a colored icon matching the logo over the system-template
/// convention. Not a template image: macOS would otherwise discard its color
/// the same way it does above.
const COLOR_ICON_BYTES: &[u8] = include_bytes!("../icons/64x64.png");

fn icon_for_style(app: &AppHandle, style: TrayIconStyle) -> Image<'static> {
    let bytes = match style {
        TrayIconStyle::Monochrome => MONOCHROME_ICON_BYTES,
        TrayIconStyle::Color => COLOR_ICON_BYTES,
    };
    Image::from_bytes(bytes).unwrap_or_else(|_| {
        app.default_window_icon()
            .cloned()
            .map_or_else(fallback_icon, Image::to_owned)
    })
}

fn fallback_icon() -> Image<'static> {
    Image::new_owned(vec![0, 0, 0, 0], 1, 1)
}

/// A template image is only meaningful for the monochrome style: it is what
/// makes macOS follow the system menu bar's light/dark appearance. Coloring
/// the glyph requires opting out of that so its color isn't discarded.
fn is_template(style: TrayIconStyle) -> bool {
    matches!(style, TrayIconStyle::Monochrome)
}

/// Creates the native notification-area icon.
///
/// # Errors
///
/// Returns a Tauri error when the tray icon cannot be constructed.
pub fn setup(app: &mut App, style: TrayIconStyle) -> tauri::Result<()> {
    let mut tray = TrayIconBuilder::with_id("openhistory")
        .tooltip("OpenHistory · Recording")
        .show_menu_on_left_click(false)
        .icon(icon_for_style(&app.handle().clone(), style));
    #[cfg(target_os = "macos")]
    {
        tray = tray.icon_as_template(is_template(style));
    }
    tray.build(app)?;
    Ok(())
}

/// Updates the live tray icon to match a newly chosen style.
///
/// # Errors
///
/// Returns a Tauri error when the tray icon cannot be found or updated.
pub fn apply_style(app: &AppHandle, style: TrayIconStyle) -> tauri::Result<()> {
    let Some(tray) = app.tray_by_id("openhistory") else {
        return Ok(());
    };
    tray.set_icon_with_as_template(Some(icon_for_style(app, style)), is_template(style))
}
/// Handles primary tray clicks and toggles the compact surface.
#[allow(clippy::needless_pass_by_value)]
pub fn handle_event(app: &AppHandle, event: TrayIconEvent) {
    if let TrayIconEvent::Click {
        position,
        button: MouseButton::Left,
        button_state: MouseButtonState::Up,
        ..
    } = event
    {
        let _ = toggle_compact(app, position);
    }
}

fn toggle_compact(app: &AppHandle, anchor: PhysicalPosition<f64>) -> tauri::Result<()> {
    let Some(window) = app.get_webview_window("compact") else {
        return Ok(());
    };
    let Some(panel) = app.try_state::<CompactPanel>() else {
        return Ok(());
    };
    panel.arm_ignore_next_blur();
    if panel.is_shown() {
        panel.set_shown(false);
        window.hide()?;
        return Ok(());
    }

    let size = window.outer_size()?;
    let monitor = app
        .monitor_from_point(anchor.x, anchor.y)?
        .or(app.primary_monitor()?);
    if let Some(monitor) = monitor {
        let area = monitor.work_area();
        let left = f64::from(area.position.x);
        let top = f64::from(area.position.y);
        let right = left + f64::from(area.size.width);
        let bottom = top + f64::from(area.size.height);
        let width = f64::from(size.width);
        let height = f64::from(size.height);
        let x = clamp_coordinate(anchor.x - width / 2.0, left + 8.0, right - width - 8.0);

        #[cfg(target_os = "macos")]
        let desired_y = anchor.y + 8.0;
        #[cfg(not(target_os = "macos"))]
        let desired_y = if bottom - anchor.y >= height + 8.0 {
            anchor.y + 8.0
        } else {
            anchor.y - height - 8.0
        };

        let y = clamp_coordinate(desired_y, top + 8.0, bottom - height - 8.0);
        window.set_position(PhysicalPosition::new(
            rounded_physical(x),
            rounded_physical(y),
        ))?;
    }
    window.show()?;
    window.set_focus()?;
    panel.set_shown(true);
    panel.disarm_ignore_next_blur();
    Ok(())
}

fn clamp_coordinate(value: f64, minimum: f64, maximum: f64) -> f64 {
    value.clamp(minimum, maximum.max(minimum))
}

#[allow(clippy::cast_possible_truncation)]
fn rounded_physical(value: f64) -> i32 {
    value
        .round()
        .clamp(f64::from(i32::MIN), f64::from(i32::MAX)) as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coordinate_is_clamped_inside_work_area() {
        assert!((clamp_coordinate(900.0, 8.0, 812.0) - 812.0).abs() < f64::EPSILON);
        assert!((clamp_coordinate(-20.0, 8.0, 812.0) - 8.0).abs() < f64::EPSILON);
    }
}
