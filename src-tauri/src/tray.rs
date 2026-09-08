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
    #[test]
    fn coordinate_is_clamped_inside_work_area() {
        assert_eq!(clamp_coordinate(900.0, 8.0, 812.0), 812.0);
        assert_eq!(clamp_coordinate(-20.0, 8.0, 812.0), 8.0);
    }
}
