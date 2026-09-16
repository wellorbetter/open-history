//! Native `OpenHistory` application shell.

mod dashboard;
mod preferences;
mod runtime;
mod tray;

use tauri::{Manager, WindowEvent};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
/// Starts the `OpenHistory` desktop shell and registers its tray and IPC handlers.
///
/// # Panics
///
/// Panics when the native application runtime cannot start.
pub fn run() {
    let dashboard = dashboard::DashboardState::default();
    let builder = tauri::Builder::default()
        .manage(dashboard.clone())
        .invoke_handler(tauri::generate_handler![
            dashboard::get_dashboard,
            dashboard::set_collection_status,
            dashboard::open_history_window,
            dashboard::delete_history,
            dashboard::add_repository,
            dashboard::remove_repository,
            dashboard::list_repositories,
            dashboard::get_tray_icon_style,
            dashboard::set_tray_icon_style
        ])
        .setup(move |app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            let data_directory = app.path().app_data_dir()?;
            std::fs::create_dir_all(&data_directory)?;
            let runtime = runtime::CollectorRuntime::initialize(
                &data_directory.join("history.sqlite3"),
                dashboard.clone(),
            )
            .map_err(|error| std::io::Error::other(error.to_string()))?;
            app.manage(runtime);
            let tray_icon_style = preferences::load_tray_icon_style(&app.handle().clone());
            tray::setup(app, tray_icon_style)?;
            Ok(())
        })
        .on_tray_icon_event(tray::handle_event)
        .on_window_event(|window, event| {
            if window.label() == "compact" && matches!(event, WindowEvent::Focused(false)) {
                let _ = window.hide();
            }
            if window.label() == "compact"
                && let WindowEvent::CloseRequested { api, .. } = event
            {
                api.prevent_close();
                let _ = window.hide();
            }
        });

    builder
        .run(tauri::generate_context!())
        .expect("failed to run OpenHistory desktop application");
}
