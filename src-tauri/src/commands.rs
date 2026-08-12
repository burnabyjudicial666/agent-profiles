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
        let metadata = entry.metadata()?;
        if metadata.is_dir() {
            total += directory_size(&entry.path())?;
        } else {
            total += metadata.len();
        }
    }
    Ok(total)
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
    let previous = store
        .get(&id)
        .cloned()
        .ok_or_else(|| format!("no profile with id {id}"))?;
    let was_default = previous.is_default;
    store.rename(&id, &label).map_err(|e| e.to_string())?;
    store.save(&state.paths).map_err(|e| e.to_string())?;
    if let Some(renamed) = store.get(&id) {
        let _ = state.platform.register_identity(renamed);
    }
    if was_default {
        let _ = state.platform.register_identity(&previous);
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

#[tauri::command]
pub fn profile_size_bytes(state: tauri::State<AppState>, id: String) -> Result<u64, String> {
    let store = state.store.lock().map_err(|e| e.to_string())?;
    let profile = store
        .get(&id)
        .ok_or_else(|| format!("no profile with id {id}"))?;
    directory_size(&profile.path).map_err(|e| e.to_string())
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
    fn profile_size_includes_nested_files() {
        let d = tempfile::tempdir().unwrap();
        std::fs::write(d.path().join("root.bin"), b"123").unwrap();
        let nested = d.path().join("nested");
        std::fs::create_dir(&nested).unwrap();
        std::fs::write(nested.join("child.bin"), b"12345").unwrap();

        assert_eq!(directory_size(d.path()).unwrap(), 8);
    }
}
