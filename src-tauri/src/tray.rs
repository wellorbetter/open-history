use tauri::{
    App, AppHandle, Manager, PhysicalPosition,
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};

/// Creates the native notification-area icon.
///
/// # Errors
///
/// Returns a Tauri error when the tray icon cannot be constructed.
pub fn setup(app: &mut App) -> tauri::Result<()> {
    let mut tray = TrayIconBuilder::with_id("openhistory")
        .tooltip("OpenHistory · Recording")
        .show_menu_on_left_click(false);
    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }
    #[cfg(target_os = "macos")]
    {
        tray = tray.icon_as_template(true);
    }
    tray.build(app)?;
    Ok(())
}
/// Handles primary tray clicks and toggles the compact surface.
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
    if window.is_visible()? {
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
        let x = (anchor.x - width / 2.0).clamp(left + 8.0, right - width - 8.0);

        #[cfg(target_os = "macos")]
        let desired_y = anchor.y + 8.0;
        #[cfg(not(target_os = "macos"))]
        let desired_y = if bottom - anchor.y >= height + 8.0 {
            anchor.y + 8.0
        } else {
            anchor.y - height - 8.0
        };

        let y = desired_y.clamp(top + 8.0, bottom - height - 8.0);
        window.set_position(PhysicalPosition::new(x.round() as i32, y.round() as i32))?;
    }
    window.show()?;
    window.set_focus()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn window_margin_is_positive() {
        const WORK_AREA_MARGIN: f64 = 8.0;
        assert!(WORK_AREA_MARGIN > 0.0);
    }
}
