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
        .manage(tray::CompactPanel::default())
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
            if window.label() == "compact"
                && matches!(event, WindowEvent::Focused(false))
                && let Some(panel) = window.try_state::<tray::CompactPanel>()
                && !panel.consume_ignored_blur()
            {
                panel.note_hidden();
                let _ = window.hide();
            }
            if window.label() == "compact"
                && let WindowEvent::CloseRequested { api, .. } = event
            {
                api.prevent_close();
                if let Some(panel) = window.try_state::<tray::CompactPanel>() {
                    panel.note_hidden();
                }
                let _ = window.hide();
            }
            // Closing the history window hides it rather than destroying it, so the tray can open
            // it again, and hands the app back its accessory (menu-bar-only) policy.
            if window.label() == "history"
                && let WindowEvent::CloseRequested { api, .. } = event
            {
                api.prevent_close();
                let _ = window.hide();
                #[cfg(target_os = "macos")]
                let _ = window
                    .app_handle()
                    .set_activation_policy(tauri::ActivationPolicy::Accessory);
            }
        });

    builder
        .run(tauri::generate_context!())
        .expect("failed to run OpenHistory desktop application");
}
