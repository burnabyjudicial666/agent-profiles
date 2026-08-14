use crate::app_spec::{AppSpec, Locations};
use crate::platform::{
    unix_ps, FocusHint, FocusOutcome, Platform, RunningProcess, ScanTarget, DATA_DIR_NAME,
};
use anyhow::{anyhow, Result};
use std::path::{Path, PathBuf};

pub struct MacOs;

fn home() -> Result<PathBuf> {
    Ok(PathBuf::from(
        std::env::var("HOME").map_err(|_| anyhow!("HOME is not set"))?,
    ))
}

fn data_root_in(home: &Path) -> PathBuf {
    home.join("Library")
        .join("Application Support")
        .join(DATA_DIR_NAME)
}

fn check_binary(bin: &Path, product: &str) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let meta = std::fs::metadata(bin)
        .map_err(|_| anyhow!("{product} was not found at {}", bin.display()))?;
    if meta.permissions().mode() & 0o111 == 0 {
        return Err(anyhow!("{} is not executable", bin.display()));
    }
    Ok(())
}

/// Resolving the row for this platform in one place, so every caller reports the
/// same thing when an app has not been declared here.
fn here<'a>(locations: &'a Locations, product: &str) -> Result<&'a crate::app_spec::MacLocation> {
    locations
        .macos
        .as_ref()
        .ok_or_else(|| anyhow!("{product} has not been declared for macOS"))
}

impl Platform for MacOs {
    fn declared_here(&self, locations: &Locations) -> bool {
        locations.macos.is_some()
    }

    fn data_root(&self) -> Result<PathBuf> {
        Ok(data_root_in(&home()?))
    }

    fn default_profile_dir(&self, locations: &Locations) -> Result<PathBuf> {
        Ok(home()?.join(here(locations, "this app")?.default_profile))
    }

    fn binary(&self, locations: &Locations, product: &str) -> Result<PathBuf> {
        let bin = PathBuf::from(here(locations, product)?.binary);
        check_binary(&bin, product)?;
        Ok(bin)
    }

    fn process_marker(&self, locations: &Locations) -> String {
        locations
            .macos
            .as_ref()
            .map(|l| l.binary.to_string())
            .unwrap_or_default()
    }

    fn scan(&self, targets: &[ScanTarget]) -> Result<Vec<RunningProcess>> {
        unix_ps::scan(targets)
    }

    fn link(&self, source: &Path, target: &Path) -> Result<()> {
        std::os::unix::fs::symlink(source, target)?;
        Ok(())
    }

    fn focus(&self, pid: i32, _hint: &FocusHint) -> Result<FocusOutcome> {
        use objc2_app_kit::{NSApplicationActivationOptions, NSRunningApplication};
        let app = NSRunningApplication::runningApplicationWithProcessIdentifier(pid)
            .ok_or_else(|| anyhow!("no running application with pid {pid}"))?;
        app.activateWithOptions(NSApplicationActivationOptions::ActivateAllWindows);
        Ok(FocusOutcome::Focused)
    }

    fn quit(&self, pid: i32) -> Result<()> {
        crate::platform::unix_signal_quit(pid)
    }

    fn register_identity(
        &self,
        _spec: &AppSpec,
        _profile_label: &str,
        _wm_class: &str,
    ) -> Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_spec;

    #[test]
    fn our_own_data_root_hangs_off_application_support() {
        assert_eq!(
            data_root_in(Path::new("/Users/h")),
            PathBuf::from("/Users/h/Library/Application Support/Agent Profiles")
        );
    }

    #[test]
    fn each_app_declares_where_its_own_stock_profile_lives() {
        // Claude keeps it under Application Support, Codex in a dotfile
        // directory. Resolving both through the home directory is what lets one
        // backend serve both without branching on the app.
        let home = Path::new("/Users/h");
        assert_eq!(
            home.join(
                app_spec::CLAUDE
                    .locations
                    .macos
                    .as_ref()
                    .unwrap()
                    .default_profile
            ),
            PathBuf::from("/Users/h/Library/Application Support/Claude")
        );
        assert_eq!(
            home.join(
                app_spec::CODEX
                    .locations
                    .macos
                    .as_ref()
                    .unwrap()
                    .default_profile
            ),
            PathBuf::from("/Users/h/.codex")
        );
    }

    #[test]
    fn a_missing_binary_is_rejected_by_name_and_product() {
        let err = check_binary(Path::new("/nope/Claude"), "Claude Desktop")
            .unwrap_err()
            .to_string();
        assert!(
            err.contains("/nope/Claude"),
            "must name the path, got: {err}"
        );
        assert!(
            err.contains("Claude Desktop"),
            "must name the product, got: {err}"
        );
    }

    #[test]
    fn a_present_but_non_executable_binary_is_rejected() {
        use std::os::unix::fs::PermissionsExt;
        let d = tempfile::tempdir().unwrap();
        let bin = d.path().join("Claude");
        std::fs::write(&bin, b"not really a binary").unwrap();
        std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert!(check_binary(&bin, "Claude Desktop").is_err());

        std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert!(check_binary(&bin, "Claude Desktop").is_ok());
    }
}
