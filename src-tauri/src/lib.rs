//! Native `OpenHistory` application shell.

mod dashboard;
mod tray;

use tauri::WindowEvent;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
/// Starts the `OpenHistory` desktop shell and registers its tray and IPC handlers.
///
/// # Panics
///
/// Panics when the native application runtime cannot start.
pub fn run() {
    let builder = tauri::Builder::default()
        .manage(dashboard::DashboardState::default())
        .invoke_handler(tauri::generate_handler![
            dashboard::get_dashboard,
            dashboard::set_collection_status,
            dashboard::open_history_window,
            dashboard::delete_history
        ])
        .setup(|app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            tray::setup(app)?;
            Ok(())
        })
        .on_tray_icon_event(tray::handle_event)
        .on_window_event(|window, event| {
            if window.label() == "compact" && matches!(event, WindowEvent::Focused(false)) {
                let _ = window.hide();
            }
            if window.label() == "compact" {
                if let WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        });

    builder
        .run(tauri::generate_context!())
        .expect("failed to run OpenHistory desktop application");
}
