use crate::app_spec::Identity;
use crate::profile_store::Profile;
use std::collections::{HashMap, HashSet};
use std::path::Path;

/// Reads whichever field this app records its signed-in account in.
///
/// Every failure is `None` on purpose: an unreadable, missing or unexpectedly
/// shaped file means "we cannot tell", and the only thing that rides on this is
/// a cosmetic warning about two profiles sharing an account.
pub fn read_account(profile_dir: &Path, identity: Option<&Identity>) -> Option<String> {
    let identity = identity?;
    let raw = std::fs::read_to_string(profile_dir.join(identity.file)).ok()?;
    let mut value: &serde_json::Value = &serde_json::from_str::<serde_json::Value>(&raw).ok()?;
    for key in identity.pointer {
        value = value.get(key)?;
    }
    value.as_str().map(str::to_string)
}

/// The accounts claimed by more than one profile of the same app.
///
/// Scoped to one app's profiles by its caller, and deliberately so: a Claude
/// account uuid and a ChatGPT account id share no namespace, and comparing
/// across apps could only ever produce a false warning.
pub fn duplicate_accounts(profiles: &[Profile]) -> HashSet<String> {
    let mut counts: HashMap<&str, usize> = HashMap::new();
    for account in profiles.iter().filter_map(|p| p.account.as_deref()) {
        *counts.entry(account).or_default() += 1;
    }
    counts
        .into_iter()
        .filter(|(_, count)| *count > 1)
        .map(|(account, _)| account.to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_spec;
    use std::path::PathBuf;

    fn profile(id: &str, account: Option<&str>) -> Profile {
        Profile {
            id: id.into(),
            label: id.into(),
            path: PathBuf::from("/tmp").join(id),
            is_default: false,
            account: account.map(str::to_string),
        }
    }

    #[test]
    fn a_top_level_field_is_read() {
        let d = tempfile::tempdir().unwrap();
        std::fs::write(
            d.path().join("config.json"),
            r#"{"darkMode":"true","lastKnownAccountUuid":"abc-123"}"#,
        )
        .unwrap();
        assert_eq!(
            read_account(d.path(), app_spec::CLAUDE.identity.as_ref()),
            Some("abc-123".to_string())
        );
    }

    #[test]
    fn a_nested_field_is_read_through_its_pointer() {
        // Codex keeps the account id one level down, inside `tokens`.
        let d = tempfile::tempdir().unwrap();
        std::fs::write(
            d.path().join("auth.json"),
            r#"{"auth_mode":"chatgpt","tokens":{"account_id":"acct-9","access_token":"secret"}}"#,
        )
        .unwrap();
        assert_eq!(
            read_account(d.path(), app_spec::CODEX.identity.as_ref()),
            Some("acct-9".to_string())
        );
    }

    #[test]
    fn an_app_that_records_no_identity_reports_none() {
        let d = tempfile::tempdir().unwrap();
        assert_eq!(read_account(d.path(), None), None);
    }

    #[test]
    fn a_missing_or_unreadable_file_yields_none() {
        let d = tempfile::tempdir().unwrap();
        let identity = app_spec::CLAUDE.identity.as_ref();
        assert_eq!(read_account(d.path(), identity), None);
        std::fs::write(d.path().join("config.json"), b"not json").unwrap();
        assert_eq!(read_account(d.path(), identity), None);
    }

    #[test]
    fn a_pointer_that_runs_off_the_end_yields_none_rather_than_panicking() {
        let d = tempfile::tempdir().unwrap();
        // `tokens` is present but holds a string, so descending into it fails.
        std::fs::write(d.path().join("auth.json"), r#"{"tokens":"nope"}"#).unwrap();
        assert_eq!(
            read_account(d.path(), app_spec::CODEX.identity.as_ref()),
            None
        );
    }

    #[test]
    fn duplicates_are_the_accounts_claimed_more_than_once() {
        let profiles = vec![
            profile("a", Some("same")),
            profile("b", Some("same")),
            profile("c", Some("other")),
            profile("d", None),
        ];
        let dupes = duplicate_accounts(&profiles);
        assert_eq!(dupes.len(), 1);
        assert!(dupes.contains("same"));
    }
}
