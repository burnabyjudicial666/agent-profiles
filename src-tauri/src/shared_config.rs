use crate::platform::Platform;
use anyhow::Result;
use std::path::Path;

#[derive(Debug, PartialEq)]
pub enum LinkState {
    AlreadyLinked,
    AdoptFile(String),
    CreateFresh,
}

pub fn inspect(link: &Path, _shared: &Path, is_linked: bool) -> Result<LinkState> {
    if is_linked {
        return Ok(LinkState::AlreadyLinked);
    }
    match std::fs::read_to_string(link) {
        Ok(contents) => Ok(LinkState::AdoptFile(contents)),
        Err(_) => Ok(LinkState::CreateFresh),
    }
}

pub fn ensure_shared(platform: &dyn Platform, profile_dir: &Path, shared: &Path) -> Result<()> {
    let link = profile_dir.join(crate::paths::CONFIG_FILENAME);
    let is_linked = is_same_file(&link, shared);

    match inspect(&link, shared, is_linked)? {
        LinkState::AlreadyLinked => {
            write_shared_if_absent(shared)?;
            return Ok(());
        }
        LinkState::AdoptFile(contents) => {
            create_parent(shared)?;
            std::fs::write(shared, contents)?;
        }
        LinkState::CreateFresh => write_shared_if_absent(shared)?,
    }

    if std::fs::symlink_metadata(&link).is_ok() {
        std::fs::remove_file(&link)?;
    }
    platform.link_shared_config(profile_dir, shared)
}

fn write_shared_if_absent(shared: &Path) -> Result<()> {
    create_parent(shared)?;
    if !shared.exists() {
        std::fs::write(shared, "{}")?;
    }
    Ok(())
}

fn create_parent(shared: &Path) -> Result<()> {
    if let Some(parent) = shared.parent() {
        std::fs::create_dir_all(parent)?;
    }
    Ok(())
}

/// True when `link` and `shared` are the same underlying file — covering both the
/// symlink case (macOS, Linux) and the hardlink case (Windows).
fn is_same_file(link: &Path, shared: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        match (std::fs::metadata(link), std::fs::metadata(shared)) {
            (Ok(a), Ok(b)) => a.dev() == b.dev() && a.ino() == b.ino(),
            _ => false,
        }
    }
    #[cfg(windows)]
    {
        // Two hardlinks to one file report the same length and creation time; the
        // authoritative check needs GetFileInformationByHandle. Compare the cheap
        // signals and let a false negative simply re-link, which is harmless.
        match (std::fs::metadata(link), std::fs::metadata(shared)) {
            (Ok(a), Ok(b)) => a.len() == b.len() && a.created().ok() == b.created().ok(),
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared_config::tests_support::FakePlatform;

    fn tmp() -> tempfile::TempDir {
        tempfile::tempdir().unwrap()
    }

    #[test]
    fn an_already_linked_config_needs_no_work() {
        let d = tmp();
        let link = d.path().join("cfg.json");
        std::fs::write(&link, "{}").unwrap();
        let shared = d.path().join("shared.json");
        assert_eq!(
            inspect(&link, &shared, true).unwrap(),
            LinkState::AlreadyLinked
        );
    }

    #[test]
    fn an_existing_regular_file_is_adopted_with_its_contents() {
        let d = tmp();
        let link = d.path().join("cfg.json");
        std::fs::write(&link, r#"{"mcpServers":{"a":{}}}"#).unwrap();
        let shared = d.path().join("shared.json");
        assert_eq!(
            inspect(&link, &shared, false).unwrap(),
            LinkState::AdoptFile(r#"{"mcpServers":{"a":{}}}"#.to_string())
        );
    }

    #[test]
    fn a_missing_config_means_create_fresh() {
        let d = tmp();
        let link = d.path().join("nothing.json");
        let shared = d.path().join("shared.json");
        assert_eq!(
            inspect(&link, &shared, false).unwrap(),
            LinkState::CreateFresh
        );
    }

    #[test]
    fn ensure_shared_seeds_an_empty_object_when_there_is_nothing_to_adopt() {
        let d = tmp();
        let profile = d.path().join("profile");
        std::fs::create_dir_all(&profile).unwrap();
        let shared = d.path().join("shared").join(crate::paths::CONFIG_FILENAME);

        ensure_shared(&FakePlatform::default(), &profile, &shared).unwrap();

        assert_eq!(std::fs::read_to_string(&shared).unwrap().trim(), "{}");
    }

    #[test]
    fn ensure_shared_copies_an_existing_config_into_shared_before_linking() {
        let d = tmp();
        let profile = d.path().join("profile");
        std::fs::create_dir_all(&profile).unwrap();
        std::fs::write(
            profile.join(crate::paths::CONFIG_FILENAME),
            r#"{"mcpServers":{"keep":{}}}"#,
        )
        .unwrap();
        let shared = d.path().join("shared").join(crate::paths::CONFIG_FILENAME);

        ensure_shared(&FakePlatform::default(), &profile, &shared).unwrap();

        assert!(std::fs::read_to_string(&shared).unwrap().contains("keep"));
    }
}

#[cfg(test)]
pub mod tests_support {
    use crate::platform::{FocusOutcome, Platform, RunningInstance};
    use std::path::{Path, PathBuf};

    /// Minimal Platform stand-in: real linking is a per-backend concern, so this
    /// one just copies. `running` lets a test stage a live instance.
    #[derive(Default)]
    pub struct FakePlatform {
        running: Vec<RunningInstance>,
    }

    impl FakePlatform {
        pub fn with_running(running: Vec<RunningInstance>) -> Self {
            Self { running }
        }
    }

    impl Platform for FakePlatform {
        fn data_root(&self) -> anyhow::Result<PathBuf> {
            unimplemented!()
        }
        fn default_profile_dir(&self) -> anyhow::Result<PathBuf> {
            unimplemented!()
        }
        fn claude_binary(&self) -> anyhow::Result<PathBuf> {
            Ok(PathBuf::from("/fake/claude"))
        }
        fn running_instances(&self) -> anyhow::Result<Vec<RunningInstance>> {
            Ok(self.running.clone())
        }
        fn link_shared_config(&self, profile_dir: &Path, shared: &Path) -> anyhow::Result<()> {
            std::fs::copy(shared, profile_dir.join(crate::paths::CONFIG_FILENAME))?;
            Ok(())
        }
        fn focus(&self, _pid: i32, _profile_id: &str) -> anyhow::Result<FocusOutcome> {
            unimplemented!()
        }
        fn quit(&self, _pid: i32) -> anyhow::Result<()> {
            unimplemented!()
        }
    }
}
