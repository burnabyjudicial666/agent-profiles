use crate::paths::Paths;
use crate::platform::{find_for, Platform, RunningInstance};
use crate::profile_store::ProfileStore;
use anyhow::{anyhow, Result};
use std::sync::Mutex;
use tauri::Manager;

pub struct AppState {
    pub platform: Box<dyn Platform>,
    pub paths: Paths,
    pub store: Mutex<ProfileStore>,
}

pub struct MenuRow {
    pub id: String,
    pub text: String,
    pub enabled: bool,
    pub pid: Option<i32>,
}

pub(crate) fn combine_error_messages(
    messages: impl IntoIterator<Item = Option<String>>,
) -> Option<String> {
    let mut messages = messages.into_iter().flatten();
    let first = messages.next()?;
    Some(
        std::iter::once(first)
            .chain(messages)
            .collect::<Vec<_>>()
            .join("; "),
    )
}

pub(crate) fn scan_instances(
    result: Result<Vec<RunningInstance>>,
) -> (Vec<RunningInstance>, Option<String>) {
    match result {
        Ok(instances) => (instances, None),
        Err(error) => (
            Vec::new(),
            Some(format!(
                "Could not scan running Claude Desktop instances: {error}"
            )),
        ),
    }
}

pub fn menu_rows(
    store: &ProfileStore,
    instances: &[RunningInstance],
    binary_error: Option<&str>,
) -> Vec<MenuRow> {
    let dupes = crate::account::duplicate_uuids(store.list());
    let mut rows: Vec<MenuRow> = store
        .list()
        .iter()
        .flat_map(|p| {
            let pid = find_for(instances, &p.path, p.is_default);
            let marker = if pid.is_some() { "●" } else { "○" };
            let shared_account = p
                .last_known_account_uuid
                .as_deref()
                .map(|u| dupes.contains(u))
                .unwrap_or(false);
            let suffix = if shared_account {
                "  (same account)"
            } else {
                ""
            };
            let action = if pid.is_some() { "focus" } else { "launch" };

            let mut out = vec![MenuRow {
                id: format!("{action}:{}", p.id),
                text: format!("{marker} {}{suffix}", p.label),
                enabled: binary_error.is_none(),
                pid,
            }];

            if pid.is_some() {
                out.push(MenuRow {
                    id: format!("quit:{}", p.id),
                    text: format!("      Quit {}", p.label),
                    enabled: binary_error.is_none(),
                    pid,
                });
            }
            out
        })
        .collect();

    if let Some(message) = binary_error {
        rows.push(MenuRow {
            id: "error".into(),
            text: message.to_string(),
            enabled: false,
            pid: None,
        });
    }
    rows
}

pub(crate) fn should_rebuild_for_event(event: &tauri::tray::TrayIconEvent) -> bool {
    matches!(event, tauri::tray::TrayIconEvent::Click { .. })
}

pub(crate) fn refresh_account_uuids(store: &mut ProfileStore) -> bool {
    let mut changed = false;
    for profile in store.list().to_vec() {
        let uuid = crate::account::read_account_uuid(&profile.path);
        if profile.last_known_account_uuid != uuid {
            store.set_account_uuid(&profile.id, uuid);
            changed = true;
        }
    }
    changed
}

pub fn rebuild(app: &tauri::AppHandle) -> Result<()> {
    rebuild_with_error(app, None)
}

pub(crate) fn rebuild_with_error(
    app: &tauri::AppHandle,
    runtime_error: Option<&str>,
) -> Result<()> {
    let state = app
        .try_state::<AppState>()
        .ok_or_else(|| anyhow!("Claude Profiles state is not available"))?;

    let rows = {
        let mut store = state
            .store
            .lock()
            .map_err(|_| anyhow!("Claude Profiles profile store is unavailable"))?;

        if refresh_account_uuids(&mut store) {
            let _ = store.save(&state.paths);
        }

        let (instances, scan_error) = scan_instances(state.platform.running_instances());
        let binary_error = state
            .platform
            .claude_binary()
            .err()
            .map(|error| error.to_string());
        let menu_error =
            combine_error_messages([runtime_error.map(str::to_string), scan_error, binary_error]);
        menu_rows(&store, &instances, menu_error.as_deref())
    };

    let menu = tauri::menu::Menu::new(app)?;
    for row in rows {
        let item =
            tauri::menu::MenuItem::with_id(app, &row.id, &row.text, row.enabled, None::<&str>)?;
        menu.append(&item)?;
    }
    menu.append(&tauri::menu::PredefinedMenuItem::separator(app)?)?;
    menu.append(&tauri::menu::MenuItem::with_id(
        app,
        "manage",
        "Manage Profiles…",
        true,
        None::<&str>,
    )?)?;
    menu.append(&tauri::menu::MenuItem::with_id(
        app,
        "quit_app",
        "Quit Claude Profiles",
        true,
        None::<&str>,
    )?)?;

    if let Some(tray) = app.tray_by_id("main") {
        tray.set_menu(Some(menu))?;
    } else {
        let icon = tauri::image::Image::from_bytes(include_bytes!("../icons/tray/tray-icon.png"))?;
        let mut tray = tauri::tray::TrayIconBuilder::with_id("main")
            .icon(icon)
            .menu(&menu)
            .tooltip("Claude Profiles");
        #[cfg(target_os = "macos")]
        {
            tray = tray.icon_as_template(true);
        }
        tray.build(app)?;
    }

    Ok(())
}

/// Re-asserts every profile's desktop identity. A no-op everywhere but Linux.
pub fn sync_identities(state: &AppState) {
    let Ok(store) = state.store.lock() else {
        return;
    };
    for p in store.list() {
        let _ = state.platform.register_identity(p);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paths::Paths;
    use crate::platform::RunningInstance;
    use crate::profile_store::ProfileStore;

    fn store_with_one_extra() -> (tempfile::TempDir, ProfileStore) {
        let d = tempfile::tempdir().unwrap();
        let paths = Paths::new(d.path().join("root"));
        let def = d.path().join("stock");
        std::fs::create_dir_all(&def).unwrap();
        let mut store = ProfileStore::load(&paths, &def).unwrap();
        store.add("Kerja", &paths).unwrap();
        (d, store)
    }

    #[test]
    fn a_running_profile_gets_a_marker_and_a_pid() {
        let (_d, store) = store_with_one_extra();
        let kerja = store.list()[1].clone();
        let instances = vec![RunningInstance {
            pid: 777,
            user_data_dir: Some(kerja.path.clone()),
        }];
        let rows = menu_rows(&store, &instances, None);
        let row = rows
            .iter()
            .find(|r| r.id == format!("focus:{}", kerja.id))
            .unwrap();
        assert_eq!(row.pid, Some(777));
        assert!(row.text.starts_with("● "));
        assert!(row.enabled);
    }

    #[test]
    fn a_running_profile_also_offers_a_quit_row_right_after_it() {
        let (_d, store) = store_with_one_extra();
        let kerja = store.list()[1].clone();
        let instances = vec![RunningInstance {
            pid: 777,
            user_data_dir: Some(kerja.path.clone()),
        }];
        let rows = menu_rows(&store, &instances, None);

        let focus_at = rows
            .iter()
            .position(|r| r.id == format!("focus:{}", kerja.id))
            .unwrap();
        let quit = &rows[focus_at + 1];
        assert_eq!(quit.id, format!("quit:{}", kerja.id));
        assert_eq!(quit.pid, Some(777));
        assert!(quit.text.contains("Quit"));
    }

    #[test]
    fn a_stopped_profile_offers_launch_and_no_quit_row() {
        let (_d, store) = store_with_one_extra();
        let kerja = store.list()[1].clone();
        let rows = menu_rows(&store, &[], None);
        let row = rows
            .iter()
            .find(|r| r.id == format!("launch:{}", kerja.id))
            .unwrap();
        assert_eq!(row.pid, None);
        assert!(row.text.starts_with("○ "));
        assert!(!rows.iter().any(|r| r.id.starts_with("quit:")));
    }

    #[test]
    fn a_missing_binary_disables_every_row_and_adds_an_explanation() {
        let (_d, store) = store_with_one_extra();
        let rows = menu_rows(&store, &[], Some("Claude Desktop was not found at /x"));
        assert!(rows.iter().filter(|r| r.id != "error").all(|r| !r.enabled));
        assert!(rows.iter().any(|r| r.text.contains("not found at /x")));
    }

    #[test]
    fn profiles_sharing_an_account_are_marked() {
        let (d, mut store) = store_with_one_extra();
        let _ = d;
        let a = store.list()[0].id.clone();
        let b = store.list()[1].id.clone();
        store.set_account_uuid(&a, Some("same".into()));
        store.set_account_uuid(&b, Some("same".into()));
        let rows = menu_rows(&store, &[], None);
        assert_eq!(
            rows.iter()
                .filter(|r| r.text.contains("same account"))
                .count(),
            2
        );
    }

    #[test]
    fn a_liveness_scan_failure_keeps_the_empty_fallback_and_exposes_its_reason() {
        let (instances, reason) = scan_instances(Err(anyhow::anyhow!("process list unavailable")));
        assert!(instances.is_empty());
        assert_eq!(
            reason.as_deref(),
            Some("Could not scan running Claude Desktop instances: process list unavailable")
        );
    }

    #[test]
    fn only_click_events_request_a_rebuild() {
        use tauri::tray::{MouseButton, MouseButtonState, TrayIconEvent};
        use tauri::{PhysicalPosition, Rect};

        let id: tauri::tray::TrayIconId = "main".into();
        let position = PhysicalPosition::new(0.0, 0.0);
        let click = TrayIconEvent::Click {
            id: id.clone(),
            position,
            rect: Rect::default(),
            button: MouseButton::Left,
            button_state: MouseButtonState::Up,
        };
        assert!(should_rebuild_for_event(&click));

        let other_events = [
            TrayIconEvent::DoubleClick {
                id: id.clone(),
                position,
                rect: Rect::default(),
                button: MouseButton::Left,
            },
            TrayIconEvent::Enter {
                id: id.clone(),
                position,
                rect: Rect::default(),
            },
            TrayIconEvent::Move {
                id: id.clone(),
                position,
                rect: Rect::default(),
            },
            TrayIconEvent::Leave {
                id,
                position,
                rect: Rect::default(),
            },
        ];
        assert!(other_events
            .iter()
            .all(|event| !should_rebuild_for_event(event)));
    }

    #[test]
    fn refreshing_account_uuids_reports_only_actual_changes() {
        let d = tempfile::tempdir().unwrap();
        let paths = Paths::new(d.path().join("root"));
        let def = d.path().join("stock");
        std::fs::create_dir_all(&def).unwrap();
        let mut store = ProfileStore::load(&paths, &def).unwrap();
        store.add("Kerja", &paths).unwrap();

        assert!(!refresh_account_uuids(&mut store));

        let profile = store.list()[1].clone();
        std::fs::write(
            profile.path.join("config.json"),
            r#"{"lastKnownAccountUuid":"abc-123"}"#,
        )
        .unwrap();
        assert!(refresh_account_uuids(&mut store));
        assert_eq!(
            store
                .get(&profile.id)
                .unwrap()
                .last_known_account_uuid
                .as_deref(),
            Some("abc-123")
        );
        assert!(!refresh_account_uuids(&mut store));
    }

    #[test]
    fn runtime_and_scan_errors_are_combined_into_one_visible_menu_reason() {
        let reason = combine_error_messages([
            Some("launch failed".to_string()),
            Some("scan failed".to_string()),
            None,
        ]);
        assert_eq!(reason.as_deref(), Some("launch failed; scan failed"));
    }
}
