mod account;
mod commands;
mod instance_manager;
mod paths;
mod platform;
mod profile_store;
mod shared_config;
mod tray;

use anyhow::{anyhow, Result};
use tauri::Manager;

fn profile(app: &tauri::AppHandle, id: &str) -> Result<profile_store::Profile> {
    let state = app
        .try_state::<tray::AppState>()
        .ok_or_else(|| anyhow!("Claude Profiles state is not available"))?;
    let store = state
        .store
        .lock()
        .map_err(|_| anyhow!("Claude Profiles profile store is unavailable"))?;
    store
        .get(id)
        .cloned()
        .ok_or_else(|| anyhow!("no profile with id {id}"))
}

fn handle_menu_event(app: &tauri::AppHandle, id: &str) -> Result<()> {
    let (action, profile_id) = match id.split_once(':') {
        Some((action, profile_id)) => (Some(action), Some(profile_id)),
        None => (None, None),
    };

    match (action, profile_id) {
        (Some("launch"), Some(id)) => {
            let state = app
                .try_state::<tray::AppState>()
                .ok_or_else(|| anyhow!("Claude Profiles state is not available"))?;
            let profile = profile(app, id)?;
            instance_manager::launch(&*state.platform, &profile, &state.paths)?;
            tray::rebuild(app)?;
        }
        (Some("focus"), Some(id)) => {
            let profile = profile(app, id)?;
            let state = app
                .try_state::<tray::AppState>()
                .ok_or_else(|| anyhow!("Claude Profiles state is not available"))?;
            let instances = state.platform.running_instances()?;
            let pid = crate::platform::find_for(&instances, &profile.path, profile.is_default)
                .ok_or_else(|| anyhow!("{} is no longer running", profile.label))?;
            match state.platform.focus(pid, &profile.id)? {
                platform::FocusOutcome::Focused => {
                    tray::rebuild(app)?;
                }
                platform::FocusOutcome::Unsupported(message) => {
                    let reason = format!("Could not focus {}: {message}", profile.label);
                    eprintln!("{reason}");
                    tray::rebuild_with_error(app, Some(&reason))?;
                }
            }
        }
        (Some("quit"), Some(id)) => {
            let profile = profile(app, id)?;
            let state = app
                .try_state::<tray::AppState>()
                .ok_or_else(|| anyhow!("Claude Profiles state is not available"))?;
            let instances = state.platform.running_instances()?;
            let pid = crate::platform::find_for(&instances, &profile.path, profile.is_default)
                .ok_or_else(|| anyhow!("{} is no longer running", profile.label))?;
            let app = app.clone();
            let worker_app = app.clone();
            let thread = std::thread::Builder::new()
                .name("claude-profiles-quit".into())
                .spawn(move || {
                    let result = (|| -> Result<()> {
                        let state = worker_app
                            .try_state::<tray::AppState>()
                            .ok_or_else(|| anyhow!("Claude Profiles state is not available"))?;
                        state.platform.quit(pid)?;
                        tray::rebuild(&worker_app)?;
                        Ok(())
                    })();
                    if let Err(error) = result {
                        eprintln!("tray quit action failed: {error}");
                        let reason = format!("Could not quit Claude Desktop: {error}");
                        if let Err(rebuild_error) =
                            tray::rebuild_with_error(&worker_app, Some(&reason))
                        {
                            eprintln!("tray rebuild failed: {rebuild_error}");
                        }
                    }
                });
            if let Err(error) = thread {
                let reason = format!("Could not start quit worker: {error}");
                eprintln!("{reason}");
                tray::rebuild_with_error(&app, Some(&reason))?;
            }
        }
        (None, None) if id == "manage" => {
            let window = app
                .get_webview_window("main")
                .ok_or_else(|| anyhow!("management window is not available"))?;
            window.show()?;
            window.set_focus()?;
        }
        (None, None) if id == "quit_app" => {
            app.exit(0);
        }
        _ => {}
    }

    Ok(())
}

/// A tray app outlives its windows. Closing the management window must hide it,
/// never destroy it: the webview is created once, and `get_webview_window` would
/// return `None` from then on, leaving "Manage Profiles…" permanently broken.
pub(crate) fn close_hides_window(label: &str) -> bool {
    label == "main"
}

/// `None` means a person closed the last window, which for a tray app is not a
/// request to quit — the tray is still there. `Some` only ever comes from our own
/// `app.exit()`, i.e. the "Quit Claude Profiles" row, which really must quit.
pub(crate) fn exit_should_be_prevented(code: Option<i32>) -> bool {
    code.is_none()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            commands::list_profiles,
            commands::add_profile,
            commands::rename_profile,
            commands::delete_profile,
            commands::profile_size_bytes,
        ])
        .on_menu_event(|app, event| {
            let id = event.id().as_ref();
            if let Err(error) = handle_menu_event(app, id) {
                let reason = format!("Tray action `{id}` failed: {error}");
                eprintln!("{reason}");
                if let Err(rebuild_error) = tray::rebuild_with_error(app, Some(&reason)) {
                    eprintln!("tray rebuild failed: {rebuild_error}");
                }
            }
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if close_hides_window(window.label()) {
                    api.prevent_close();
                    if let Err(error) = window.hide() {
                        eprintln!("could not hide the management window: {error}");
                    }
                }
            }
        })
        .on_tray_icon_event(|app, event| {
            if tray::should_rebuild_for_event(&event) {
                if let Err(error) = tray::rebuild(app) {
                    eprintln!("tray rebuild failed: {error}");
                }
            }
        })
        .setup(|app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            let platform = platform::current();
            let paths = paths::Paths::new(platform.data_root()?);
            let default_dir = platform.default_profile_dir()?;
            let store = profile_store::ProfileStore::load(&paths, &default_dir)?;
            app.manage(tray::AppState {
                platform,
                paths,
                store: std::sync::Mutex::new(store),
            });

            if let Some(state) = app.try_state::<tray::AppState>() {
                tray::sync_identities(&state);
            }
            tray::rebuild(app.handle())?;
            Ok(())
        })
        .build(tauri::generate_context!());

    match app {
        Ok(app) => app.run(|_app, event| {
            if let tauri::RunEvent::ExitRequested { api, code, .. } = &event {
                if exit_should_be_prevented(*code) {
                    api.prevent_exit();
                }
            }
        }),
        Err(error) => eprintln!("Claude Profiles failed to run: {error}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn closing_the_management_window_hides_it_rather_than_destroying_it() {
        assert!(close_hides_window("main"));
        assert!(!close_hides_window("some-future-window"));
    }

    #[test]
    fn only_our_own_quit_row_is_allowed_to_end_the_process() {
        // A person closing the last window reports no code; the tray lives on.
        assert!(exit_should_be_prevented(None));
        // `app.exit(0)` from "Quit Claude Profiles" reports one, and must win.
        assert!(!exit_should_be_prevented(Some(0)));
    }
}
