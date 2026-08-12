mod account;
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let result = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
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
        .on_tray_icon_event(|app, event| {
            if tray::should_rebuild_for_event(&event) {
                if let Err(error) = tray::rebuild(app) {
                    eprintln!("tray rebuild failed: {error}");
                }
            }
        })
        .setup(|app| {
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
        .run(tauri::generate_context!());

    if let Err(error) = result {
        eprintln!("Claude Profiles failed to run: {error}");
    }
}
