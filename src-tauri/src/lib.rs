//! Native `OpenHistory` application shell.

mod agent;
mod dashboard;
mod preferences;
mod runtime;
mod tray;

use tauri::{Emitter, Manager, WindowEvent};

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
            dashboard::set_tray_icon_style,
            dashboard::get_capture_granularity,
            dashboard::set_capture_granularity,
            dashboard::get_timeline_bucket,
            dashboard::set_timeline_bucket,
            dashboard::interpret_activity
        ])
        .setup(move |app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            let data_directory = app.path().app_data_dir()?;
            std::fs::create_dir_all(&data_directory)?;
            let preferences = preferences::load(&app.handle().clone());
            let runtime = runtime::CollectorRuntime::new(
                data_directory.join("history.sqlite3"),
                dashboard.clone(),
                preferences.capture_granularity.into(),
            );
            app.manage(runtime);
            tray::setup(app, preferences.tray_icon_style)?;
            // Opening encrypted storage reads a key out of the login keychain, which may stop to
            // ask for a password. On the main thread that dialog freezes the app that raised it,
            // so the launch continues without waiting and storage catches up behind it.
            let handle = app.handle().clone();
            let collecting = preferences.collecting;
            let status = dashboard.clone();
            tauri::async_runtime::spawn(async move {
                let opened = handle
                    .state::<runtime::CollectorRuntime>()
                    .open_storage()
                    .await;
                if opened.is_err() {
                    status.set_status(dashboard::CollectionStatus::Error);
                    let _ = handle.emit(
                        "collection-status-changed",
                        dashboard::CollectionStatus::Error,
                    );
                    return;
                }
                if collecting {
                    handle
                        .state::<runtime::CollectorRuntime>()
                        .resume(handle.clone())
                        .await;
                }
            });
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
