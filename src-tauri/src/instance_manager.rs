use crate::paths::Paths;
use crate::platform::{find_for, Platform};
use crate::profile_store::Profile;
use anyhow::{anyhow, Result};

pub fn launch_args(profile: &Profile) -> Vec<String> {
    if profile.is_default {
        Vec::new()
    } else {
        vec![format!("--user-data-dir={}", profile.path.display())]
    }
}

pub fn prepare(platform: &dyn Platform, profile: &Profile, paths: &Paths) -> Result<()> {
    if !profile.path.is_dir() {
        return Err(anyhow!(
            "profile directory {} is missing",
            profile.path.display()
        ));
    }
    crate::shared_config::ensure_shared(platform, &profile.path, &paths.shared_config())
}

/// Refuses to spawn a second process against a user-data directory that already
/// has one. Claude Desktop has NO single-instance lock (verified 2026-08-12): two
/// processes on one profile both stay alive and corrupt its databases. The tray
/// already offers Focus instead of Launch for a live profile, but a menu built
/// seconds ago can be stale, so the check is repeated here, closest to the spawn.
pub fn launch(platform: &dyn Platform, profile: &Profile, paths: &Paths) -> Result<i32> {
    let running = platform.running_instances().unwrap_or_default();
    if let Some(pid) = find_for(&running, &profile.path, profile.is_default) {
        return Err(anyhow!(
            "{} is already running as pid {pid}; focus it instead of launching a second copy",
            profile.label
        ));
    }

    let binary = platform.claude_binary()?;
    prepare(platform, profile, paths)?;
    let mut args = platform.extra_launch_args(profile);
    args.extend(launch_args(profile)); // --user-data-dir stays last

    let child = std::process::Command::new(binary)
        .args(args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()?;
    Ok(child.id() as i32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile_store::Profile;
    use crate::shared_config::tests_support::FakePlatform;
    use std::path::PathBuf;

    fn profile(is_default: bool) -> Profile {
        Profile {
            id: "x".into(),
            label: "X".into(),
            path: PathBuf::from("/p/x"),
            is_default,
            last_known_account_uuid: None,
        }
    }

    #[test]
    fn a_normal_profile_launches_with_its_user_data_dir() {
        assert_eq!(launch_args(&profile(false)), vec!["--user-data-dir=/p/x"]);
    }

    #[test]
    fn the_default_profile_launches_with_no_arguments() {
        assert!(launch_args(&profile(true)).is_empty());
    }

    #[test]
    fn preparing_a_missing_profile_directory_is_an_error() {
        let d = tempfile::tempdir().unwrap();
        let paths = crate::paths::Paths::new(d.path());
        let p = Profile {
            path: d.path().join("gone"),
            ..profile(false)
        };
        assert!(prepare(&FakePlatform::default(), &p, &paths).is_err());
    }

    #[test]
    fn launching_a_profile_that_is_already_running_is_refused() {
        let d = tempfile::tempdir().unwrap();
        let paths = crate::paths::Paths::new(d.path());
        let p = Profile {
            path: d.path().join("live"),
            ..profile(false)
        };
        std::fs::create_dir_all(&p.path).unwrap();

        let platform = FakePlatform::with_running(vec![crate::platform::RunningInstance {
            pid: 4242,
            user_data_dir: Some(p.path.clone()),
        }]);

        let err = launch(&platform, &p, &paths).unwrap_err().to_string();
        assert!(err.contains("already running"), "got: {err}");
        assert!(
            err.contains("4242"),
            "the error must name the pid, got: {err}"
        );
    }
}
