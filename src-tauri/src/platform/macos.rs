use crate::platform::{unix_ps, FocusOutcome, Platform, RunningInstance};
use anyhow::{anyhow, Result};
use std::path::{Path, PathBuf};

pub const BINARY: &str = "/Applications/Claude.app/Contents/MacOS/Claude";

pub struct MacOs;

fn home() -> Result<PathBuf> {
    Ok(PathBuf::from(
        std::env::var("HOME").map_err(|_| anyhow!("HOME is not set"))?,
    ))
}

fn data_root_in(home: &Path) -> PathBuf {
    home.join("Library")
        .join("Application Support")
        .join("Claude Profiles")
}

fn default_profile_in(home: &Path) -> PathBuf {
    home.join("Library")
        .join("Application Support")
        .join("Claude")
}

fn check_binary(bin: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let meta = std::fs::metadata(bin)
        .map_err(|_| anyhow!("Claude Desktop was not found at {}", bin.display()))?;
    if meta.permissions().mode() & 0o111 == 0 {
        return Err(anyhow!("{} is not executable", bin.display()));
    }
    Ok(())
}

impl Platform for MacOs {
    fn data_root(&self) -> Result<PathBuf> {
        Ok(data_root_in(&home()?))
    }

    fn default_profile_dir(&self) -> Result<PathBuf> {
        Ok(default_profile_in(&home()?))
    }

    fn claude_binary(&self) -> Result<PathBuf> {
        let bin = PathBuf::from(BINARY);
        check_binary(&bin)?;
        Ok(bin)
    }

    fn running_instances(&self) -> Result<Vec<RunningInstance>> {
        unix_ps::scan(&[BINARY])
    }

    fn link_shared_config(&self, profile_dir: &Path, shared: &Path) -> Result<()> {
        std::os::unix::fs::symlink(shared, profile_dir.join(crate::paths::CONFIG_FILENAME))?;
        Ok(())
    }

    fn focus(&self, pid: i32, _profile_id: &str) -> Result<FocusOutcome> {
        use objc2_app_kit::{NSApplicationActivationOptions, NSRunningApplication};
        let app = NSRunningApplication::runningApplicationWithProcessIdentifier(pid)
            .ok_or_else(|| anyhow!("no running application with pid {pid}"))?;
        app.activateWithOptions(NSApplicationActivationOptions::ActivateAllWindows);
        Ok(FocusOutcome::Focused)
    }

    fn quit(&self, pid: i32) -> Result<()> {
        crate::platform::unix_signal_quit(pid)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_hang_off_application_support() {
        let root = std::path::Path::new("/Users/h");
        assert_eq!(
            data_root_in(root),
            std::path::PathBuf::from("/Users/h/Library/Application Support/Claude Profiles")
        );
        assert_eq!(
            default_profile_in(root),
            std::path::PathBuf::from("/Users/h/Library/Application Support/Claude")
        );
    }

    #[test]
    fn a_missing_binary_is_rejected_by_name() {
        let err = check_binary(std::path::Path::new("/nope/Claude"))
            .unwrap_err()
            .to_string();
        assert!(
            err.contains("/nope/Claude"),
            "the error must name the path, got: {err}"
        );
    }

    #[test]
    fn a_present_but_non_executable_binary_is_rejected() {
        use std::os::unix::fs::PermissionsExt;
        let d = tempfile::tempdir().unwrap();
        let bin = d.path().join("Claude");
        std::fs::write(&bin, b"not really a binary").unwrap();
        std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert!(check_binary(&bin).is_err());

        std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert!(check_binary(&bin).is_ok());
    }
}
