use crate::profile_store::Profile;
use std::collections::{HashMap, HashSet};
use std::path::Path;

pub fn read_account_uuid(profile_dir: &Path) -> Option<String> {
    let raw = std::fs::read_to_string(profile_dir.join("config.json")).ok()?;
    let value: serde_json::Value = serde_json::from_str(&raw).ok()?;
    value
        .get("lastKnownAccountUuid")?
        .as_str()
        .map(str::to_string)
}

pub fn duplicate_uuids(profiles: &[Profile]) -> HashSet<String> {
    let mut counts: HashMap<&str, usize> = HashMap::new();
    for uuid in profiles
        .iter()
        .filter_map(|profile| profile.last_known_account_uuid.as_deref())
    {
        *counts.entry(uuid).or_default() += 1;
    }
    counts
        .into_iter()
        .filter(|(_, count)| *count > 1)
        .map(|(uuid, _)| uuid.to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile_store::Profile;
    use std::path::PathBuf;

    fn profile(id: &str, uuid: Option<&str>) -> Profile {
        Profile {
            id: id.into(),
            label: id.into(),
            path: PathBuf::from("/tmp").join(id),
            is_default: false,
            last_known_account_uuid: uuid.map(str::to_string),
        }
    }

    #[test]
    fn reads_the_account_uuid_from_config_json() {
        let d = tempfile::tempdir().unwrap();
        std::fs::write(
            d.path().join("config.json"),
            r#"{"darkMode":"true","lastKnownAccountUuid":"abc-123"}"#,
        )
        .unwrap();
        assert_eq!(read_account_uuid(d.path()), Some("abc-123".to_string()));
    }

    #[test]
    fn a_missing_or_unreadable_config_yields_none() {
        let d = tempfile::tempdir().unwrap();
        assert_eq!(read_account_uuid(d.path()), None);
        std::fs::write(d.path().join("config.json"), b"not json").unwrap();
        assert_eq!(read_account_uuid(d.path()), None);
    }

    #[test]
    fn duplicates_are_the_uuids_claimed_more_than_once() {
        let profiles = vec![
            profile("a", Some("same")),
            profile("b", Some("same")),
            profile("c", Some("other")),
            profile("d", None),
        ];
        let dupes = duplicate_uuids(&profiles);
        assert_eq!(dupes.len(), 1);
        assert!(dupes.contains("same"));
    }
}
