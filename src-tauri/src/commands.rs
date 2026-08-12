use crate::platform::{find_for, Platform};
use crate::profile_store::{Profile, ProfileStore};
use crate::tray::AppState;
use serde::Serialize;
use std::path::Path;

#[derive(Serialize, Clone)]
pub struct ProfileView {
    pub id: String,
    pub label: String,
    pub path: String,
    pub is_default: bool,
    pub shares_account: bool,
}

pub fn to_views(store: &ProfileStore) -> Vec<ProfileView> {
    let dupes = crate::account::duplicate_uuids(store.list());
    store
        .list()
        .iter()
        .map(|p| ProfileView {
            id: p.id.clone(),
            label: p.label.clone(),
            path: p.path.display().to_string(),
            is_default: p.is_default,
            shares_account: p
                .last_known_account_uuid
                .as_deref()
                .map(|u| dupes.contains(u))
                .unwrap_or(false),
        })
        .collect()
}

pub(crate) fn refuse_if_running(platform: &dyn Platform, profile: &Profile) -> anyhow::Result<()> {
    let instances = platform.running_instances()?;
    if find_for(&instances, &profile.path, profile.is_default).is_some() {
        anyhow::bail!("quit this profile's Claude Desktop before deleting it");
    }
    Ok(())
}

pub(crate) fn directory_size(path: &Path) -> anyhow::Result<u64> {
    if !path.is_dir() {
        return Ok(0);
    }

    let mut total = 0;
    for entry in std::fs::read_dir(path)? {
        let entry = entry?;
        // `symlink_metadata` does NOT follow links. Following them would descend
        // into whatever a link points at — counting bytes that live outside this
        // profile, and recursing forever on a link that points back up its own tree.
        let metadata = entry.path().symlink_metadata()?;
        if metadata.is_dir() {
            total += directory_size(&entry.path())?;
        } else {
            total += metadata.len();
        }
    }
    Ok(total)
}

/// The frontend trims and refuses a blank label, but a Tauri command is the real
/// API boundary. A blank label renders as a nameless tray row, and a duplicate one
/// renders as two identical rows for two different accounts — both leave the user
/// unable to tell which profile they are about to launch.
pub(crate) fn validate_label(
    store: &ProfileStore,
    label: &str,
    exclude_id: &str,
) -> Result<String, String> {
    let label = label.trim();
    if label.is_empty() {
        return Err("a profile needs a label".into());
    }
    let taken = store
        .list()
        .iter()
        .any(|p| p.id != exclude_id && p.label.eq_ignore_ascii_case(label));
    if taken {
        return Err(format!("another profile is already called “{label}”"));
    }
    Ok(label.to_string())
}

fn register_renamed_identity(platform: &dyn Platform, renamed: &Profile) {
    let _ = platform.register_identity(renamed);
}

#[tauri::command]
pub fn list_profiles(state: tauri::State<AppState>) -> Result<Vec<ProfileView>, String> {
    let store = state.store.lock().map_err(|e| e.to_string())?;
    Ok(to_views(&store))
}

#[tauri::command]
pub fn add_profile(
    app: tauri::AppHandle,
    state: tauri::State<AppState>,
    label: String,
) -> Result<ProfileView, String> {
    let mut store = state.store.lock().map_err(|e| e.to_string())?;
    let label = validate_label(&store, &label, "")?;
    let created = store.add(&label, &state.paths).map_err(|e| e.to_string())?;
    store.save(&state.paths).map_err(|e| e.to_string())?;
    let view = to_views(&store)
        .into_iter()
        .find(|v| v.id == created.id)
        .ok_or_else(|| "profile vanished after creation".to_string())?;
    let _ = state.platform.register_identity(&created);
    drop(store);
    let _ = crate::tray::rebuild(&app);
    Ok(view)
}

#[tauri::command]
pub fn rename_profile(
    app: tauri::AppHandle,
    state: tauri::State<AppState>,
    id: String,
    label: String,
) -> Result<(), String> {
    let mut store = state.store.lock().map_err(|e| e.to_string())?;
    store
        .get(&id)
        .ok_or_else(|| format!("no profile with id {id}"))?;
    // Exclude this profile from the duplicate check, so re-saving its own label
    // (or only changing its capitalisation) is not reported as a collision.
    let label = validate_label(&store, &label, &id)?;
    store.rename(&id, &label).map_err(|e| e.to_string())?;
    store.save(&state.paths).map_err(|e| e.to_string())?;
    if let Some(renamed) = store.get(&id) {
        register_renamed_identity(&*state.platform, renamed);
    }
    drop(store);
    let _ = crate::tray::rebuild(&app);
    Ok(())
}

#[tauri::command]
pub fn delete_profile(
    app: tauri::AppHandle,
    state: tauri::State<AppState>,
    id: String,
) -> Result<(), String> {
    let mut store = state.store.lock().map_err(|e| e.to_string())?;
    let profile = store
        .get(&id)
        .cloned()
        .ok_or_else(|| format!("no profile with id {id}"))?;
    refuse_if_running(&*state.platform, &profile).map_err(|e| e.to_string())?;
    let _ = state.platform.unregister_identity(&profile);
    store.remove(&id, &state.paths).map_err(|e| e.to_string())?;
    store.save(&state.paths).map_err(|e| e.to_string())?;
    drop(store);
    let _ = crate::tray::rebuild(&app);
    Ok(())
}

#[derive(Serialize)]
pub struct AutostartState {
    /// False in development builds, where registering a login item would point at
    /// a binary that moves. The UI hides the control rather than offering a lie.
    pub offered: bool,
    pub enabled: bool,
}

/// The operating system is the single source of truth. Deliberately not mirrored
/// into `profiles.json`: a person can turn the login item off in System Settings
/// without telling this app, and a stored copy would then be confidently wrong.
#[tauri::command]
pub fn autostart_state(app: tauri::AppHandle) -> Result<AutostartState, String> {
    use tauri_plugin_autostart::ManagerExt;
    if !crate::autostart_is_offered() {
        return Ok(AutostartState {
            offered: false,
            enabled: false,
        });
    }
    let enabled = app.autolaunch().is_enabled().map_err(|e| e.to_string())?;
    Ok(AutostartState {
        offered: true,
        enabled,
    })
}

#[tauri::command]
pub fn set_autostart(app: tauri::AppHandle, enabled: bool) -> Result<(), String> {
    use tauri_plugin_autostart::ManagerExt;
    if !crate::autostart_is_offered() {
        return Err("launching at login is only available in an installed build".into());
    }
    let manager = app.autolaunch();
    if enabled {
        manager.enable().map_err(|e| e.to_string())
    } else {
        manager.disable().map_err(|e| e.to_string())
    }
}

#[tauri::command]
pub fn profile_size_bytes(state: tauri::State<AppState>, id: String) -> Result<u64, String> {
    // Take the path and let the lock go. Walking a profile directory is seconds of
    // I/O on a large account, and the tray rebuild wants this same mutex from the
    // main thread on every hover — holding it across the walk freezes the whole app.
    let path = {
        let store = state.store.lock().map_err(|e| e.to_string())?;
        store
            .get(&id)
            .map(|profile| profile.path.clone())
            .ok_or_else(|| format!("no profile with id {id}"))?
    };
    directory_size(&path).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paths::Paths;
    use crate::profile_store::ProfileStore;

    #[test]
    fn profiles_sharing_an_account_are_flagged_in_the_view() {
        let d = tempfile::tempdir().unwrap();
        let paths = Paths::new(d.path().join("root"));
        let def = d.path().join("stock");
        std::fs::create_dir_all(&def).unwrap();
        let mut store = ProfileStore::load(&paths, &def).unwrap();
        let a = store.add("A", &paths).unwrap();
        let b = store.add("B", &paths).unwrap();
        store.set_account_uuid(&a.id, Some("same".into()));
        store.set_account_uuid(&b.id, Some("same".into()));

        let views = to_views(&store);

        assert!(views.iter().find(|v| v.id == a.id).unwrap().shares_account);
        assert!(views.iter().find(|v| v.id == b.id).unwrap().shares_account);
        assert!(!views[0].shares_account); // Default has no uuid
    }

    #[test]
    fn renaming_a_default_profile_registers_only_the_new_identity() {
        use crate::platform::{FocusOutcome, Platform, RunningInstance};
        use std::path::{Path, PathBuf};
        use std::sync::{Arc, Mutex};

        struct RecordingPlatform {
            registrations: Arc<Mutex<Vec<Profile>>>,
        }

        impl Platform for RecordingPlatform {
            fn data_root(&self) -> anyhow::Result<PathBuf> {
                Ok(PathBuf::new())
            }

            fn default_profile_dir(&self) -> anyhow::Result<PathBuf> {
                Ok(PathBuf::new())
            }

            fn claude_binary(&self) -> anyhow::Result<PathBuf> {
                Ok(PathBuf::new())
            }

            fn running_instances(&self) -> anyhow::Result<Vec<RunningInstance>> {
                Ok(Vec::new())
            }

            fn link_shared_config(
                &self,
                _profile_dir: &Path,
                _shared: &Path,
            ) -> anyhow::Result<()> {
                Ok(())
            }

            fn focus(&self, _pid: i32, _profile_id: &str) -> anyhow::Result<FocusOutcome> {
                Ok(FocusOutcome::Focused)
            }

            fn quit(&self, _pid: i32) -> anyhow::Result<()> {
                Ok(())
            }

            fn register_identity(&self, profile: &Profile) -> anyhow::Result<()> {
                self.registrations.lock().unwrap().push(profile.clone());
                Ok(())
            }
        }

        let d = tempfile::tempdir().unwrap();
        let paths = Paths::new(d.path().join("root"));
        let default_dir = d.path().join("stock");
        std::fs::create_dir_all(&default_dir).unwrap();
        let mut store = ProfileStore::load(&paths, &default_dir).unwrap();
        store.rename("default", "Personal").unwrap();
        let renamed = store.get("default").cloned().unwrap();
        let registrations = Arc::new(Mutex::new(Vec::new()));
        let platform = RecordingPlatform {
            registrations: Arc::clone(&registrations),
        };

        register_renamed_identity(&platform, &renamed);

        let registrations = registrations.lock().unwrap();
        assert_eq!(registrations.len(), 1);
        assert_eq!(registrations[0].id, "default");
        assert_eq!(registrations[0].label, "Personal");
    }

    #[test]
    fn deletion_is_refused_when_the_profile_has_a_running_instance() {
        use crate::platform::RunningInstance;
        use crate::shared_config::tests_support::FakePlatform;
        use std::path::PathBuf;

        let profile = crate::profile_store::Profile {
            id: "work".into(),
            label: "Work".into(),
            path: PathBuf::from("/profiles/work"),
            is_default: false,
            last_known_account_uuid: None,
        };
        let platform = FakePlatform::with_running(vec![RunningInstance {
            pid: 4242,
            user_data_dir: Some(profile.path.clone()),
        }]);

        let error = refuse_if_running(&platform, &profile)
            .unwrap_err()
            .to_string();
        assert_eq!(
            error,
            "quit this profile's Claude Desktop before deleting it"
        );
    }

    #[test]
    fn a_blank_label_is_refused() {
        let d = tempfile::tempdir().unwrap();
        let paths = Paths::new(d.path().join("root"));
        let def = d.path().join("stock");
        std::fs::create_dir_all(&def).unwrap();
        let store = ProfileStore::load(&paths, &def).unwrap();

        assert!(validate_label(&store, "   ", "").is_err());
        assert_eq!(validate_label(&store, "  Kerja  ", "").unwrap(), "Kerja");
    }

    #[test]
    fn a_label_already_taken_by_another_profile_is_refused() {
        let d = tempfile::tempdir().unwrap();
        let paths = Paths::new(d.path().join("root"));
        let def = d.path().join("stock");
        std::fs::create_dir_all(&def).unwrap();
        let mut store = ProfileStore::load(&paths, &def).unwrap();
        let kerja = store.add("Kerja", &paths).unwrap();

        // Case differences still collide: two tray rows a person cannot tell apart.
        assert!(validate_label(&store, "kerja", "").is_err());
        // But a profile may keep, or re-case, its own label.
        assert_eq!(validate_label(&store, "KERJA", &kerja.id).unwrap(), "KERJA");
    }

    #[test]
    fn a_symlinked_directory_is_not_followed_when_measuring_a_profile() {
        let d = tempfile::tempdir().unwrap();
        let profile = d.path().join("profile");
        std::fs::create_dir(&profile).unwrap();
        std::fs::write(profile.join("real.bin"), b"1234").unwrap();

        let outside = d.path().join("outside");
        std::fs::create_dir(&outside).unwrap();
        std::fs::write(outside.join("huge.bin"), vec![0u8; 4096]).unwrap();

        #[cfg(unix)]
        std::os::unix::fs::symlink(&outside, profile.join("link")).unwrap();

        // Only the profile's own 4 bytes count, plus the link entry itself — never
        // the 4096 bytes living outside the profile.
        assert!(directory_size(&profile).unwrap() < 100);
    }

    #[test]
    fn profile_size_includes_nested_files() {
        let d = tempfile::tempdir().unwrap();
        std::fs::write(d.path().join("root.bin"), b"123").unwrap();
        let nested = d.path().join("nested");
        std::fs::create_dir(&nested).unwrap();
        std::fs::write(nested.join("child.bin"), b"12345").unwrap();

        assert_eq!(directory_size(d.path()).unwrap(), 8);
    }
}
