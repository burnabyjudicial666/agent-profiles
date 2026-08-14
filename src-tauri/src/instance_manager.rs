use crate::app_spec::AppSpec;
use crate::paths::Paths;
use crate::platform::{find_for, wm_class, Platform, ScanTarget};
use crate::profile_store::Profile;
use anyhow::{anyhow, Result};

/// What a scan must look for to recognise this app's processes.
pub fn scan_target(platform: &dyn Platform, spec: &'static AppSpec) -> Result<ScanTarget> {
    Ok(ScanTarget {
        app_id: spec.id,
        marker: platform.process_marker(&spec.locations)?,
        flag: spec.readback_flag(),
    })
}

/// The arguments a launch carries, in the order they must appear.
///
/// The stock profile is launched bare: it is defined as the one the app uses
/// when nothing designates a directory, so designating one would make it a
/// different profile and orphan everything the user already has.
pub fn launch_args(platform: &dyn Platform, spec: &AppSpec, profile: &Profile) -> Vec<String> {
    let mut args = platform.os_launch_args(&wm_class(spec.id, &profile.id));
    if !profile.is_default {
        args.extend(spec.launch_args(&profile.path));
    }
    args
}

pub fn launch_env(spec: &AppSpec, profile: &Profile) -> Vec<(&'static str, String)> {
    if profile.is_default {
        Vec::new()
    } else {
        spec.launch_env(&profile.path)
    }
}

pub fn prepare(
    platform: &dyn Platform,
    spec: &AppSpec,
    profile: &Profile,
    paths: &Paths,
) -> Result<()> {
    if !profile.path.is_dir() {
        return Err(anyhow!(
            "profile directory {} is missing",
            profile.path.display()
        ));
    }
    let Some(filename) = spec.shared_config else {
        return Ok(());
    };
    crate::shared_config::ensure_shared(
        platform,
        &profile.path,
        &paths.shared_config(filename),
        filename,
    )
}

/// Refuses to spawn a second process against a profile directory that already
/// has one. Claude Desktop has NO single-instance lock (verified 2026-08-12):
/// two processes on one profile both stay alive and corrupt its databases.
/// ChatGPT does hold one, keyed on its user-data directory, so a duplicate exits
/// on its own — but silently, which looks to the user like a launch that did
/// nothing. The tray already offers Focus instead of Launch for a live profile,
/// but a menu built seconds ago can be stale, so the check is repeated here,
/// closest to the spawn.
pub fn launch(
    platform: &dyn Platform,
    spec: &'static AppSpec,
    profile: &Profile,
    paths: &Paths,
) -> Result<i32> {
    // Fail CLOSED. If the process scan itself fails we do not know whether this
    // profile is live, and `unwrap_or_default()` would turn "I cannot tell" into
    // "nothing is running" — launching straight into the corruption this guard
    // exists to prevent. Refusing costs the user one retry; guessing costs them
    // their profile.
    let running = platform
        .scan(&[scan_target(platform, spec)?])
        .map_err(|error| {
            anyhow!(
                "could not check whether {} is already running ({error}); \
                 refusing to launch, because a second copy would corrupt the profile",
                profile.label
            )
        })?;
    if let Some(pid) = find_for(&running, spec.id, &profile.path, profile.is_default) {
        return Err(anyhow!(
            "{} is already running as pid {pid}; focus it instead of launching a second copy",
            profile.label
        ));
    }

    let binary = platform.binary(&spec.locations, spec.product)?;
    prepare(platform, spec, profile, paths)?;

    let child = std::process::Command::new(binary)
        .args(launch_args(platform, spec, profile))
        .envs(launch_env(spec, profile))
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()?;
    Ok(child.id() as i32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_spec;
    use crate::platform::RunningProcess;
    use crate::shared_config::tests_support::FakePlatform;
    use std::path::PathBuf;

    fn profile(is_default: bool) -> Profile {
        Profile {
            id: "x".into(),
            label: "X".into(),
            path: PathBuf::from("/p/x"),
            is_default,
            account: None,
        }
    }

    #[test]
    fn an_app_designated_by_argument_launches_with_it() {
        let args = launch_args(&FakePlatform::default(), &app_spec::CLAUDE, &profile(false));
        assert_eq!(args, vec!["--user-data-dir=/p/x"]);
    }

    #[test]
    fn an_app_designated_by_both_channels_gets_both() {
        let p = profile(false);
        let args = launch_args(&FakePlatform::default(), &app_spec::CODEX, &p);
        assert_eq!(args, vec!["--user-data-dir=/p/x"]);
        assert_eq!(
            launch_env(&app_spec::CODEX, &p),
            vec![("CODEX_HOME", "/p/x".into())]
        );
    }

    #[test]
    fn the_stock_profile_is_launched_with_no_designation_at_all() {
        // Not just "no argument": an app designated by environment would still be
        // moved off its stock directory by a stray variable.
        let p = profile(true);
        for spec in app_spec::all() {
            assert!(
                launch_args(&FakePlatform::default(), spec, &p).is_empty(),
                "{} designates the stock profile by argument",
                spec.id
            );
            assert!(
                launch_env(spec, &p).is_empty(),
                "{} designates the stock profile by environment",
                spec.id
            );
        }
    }

    #[test]
    fn a_scan_target_asks_for_the_flag_the_app_is_read_back_from() {
        let target = scan_target(&FakePlatform::default(), &app_spec::CODEX).unwrap();
        assert_eq!(target.app_id, "codex");
        assert_eq!(target.flag, "--user-data-dir=");
    }

    #[test]
    fn preparing_a_missing_profile_directory_is_an_error() {
        let d = tempfile::tempdir().unwrap();
        let paths = Paths::new(d.path());
        let p = Profile {
            path: d.path().join("gone"),
            ..profile(false)
        };
        assert!(prepare(&FakePlatform::default(), &app_spec::CLAUDE, &p, &paths).is_err());
    }

    #[test]
    fn preparing_links_the_app_s_own_shared_file() {
        let d = tempfile::tempdir().unwrap();
        let paths = Paths::new(d.path());
        let p = Profile {
            path: d.path().join("live"),
            ..profile(false)
        };
        std::fs::create_dir_all(&p.path).unwrap();

        prepare(&FakePlatform::default(), &app_spec::CODEX, &p, &paths).unwrap();

        // Codex shares config.toml, not Claude's JSON file.
        assert!(paths.shared_config("config.toml").exists());
        assert!(p.path.join("config.toml").exists());
    }

    #[test]
    fn launching_a_profile_that_is_already_running_is_refused() {
        let d = tempfile::tempdir().unwrap();
        let paths = Paths::new(d.path());
        let p = Profile {
            path: d.path().join("live"),
            ..profile(false)
        };
        std::fs::create_dir_all(&p.path).unwrap();

        let platform = FakePlatform::with_running(vec![RunningProcess {
            app_id: "claude",
            pid: 4242,
            profile_dir: Some(p.path.clone()),
        }]);

        let err = launch(&platform, &app_spec::CLAUDE, &p, &paths)
            .unwrap_err()
            .to_string();
        assert!(err.contains("already running"), "got: {err}");
        assert!(
            err.contains("4242"),
            "the error must name the pid, got: {err}"
        );
    }

    #[test]
    fn another_app_running_the_same_directory_does_not_block_a_launch() {
        // The guard is per app. A ChatGPT process must never be mistaken for a
        // live Claude profile, or launching Claude would be refused forever.
        let d = tempfile::tempdir().unwrap();
        let paths = Paths::new(d.path());
        let p = Profile {
            path: d.path().join("live"),
            ..profile(false)
        };
        std::fs::create_dir_all(&p.path).unwrap();

        let platform = FakePlatform::with_running(vec![RunningProcess {
            app_id: "codex",
            pid: 4242,
            profile_dir: Some(p.path.clone()),
        }]);

        // It gets past the guard and fails later, at the spawn of a fake binary.
        let err = launch(&platform, &app_spec::CLAUDE, &p, &paths)
            .unwrap_err()
            .to_string();
        assert!(!err.contains("already running"), "got: {err}");
    }

    #[test]
    fn launching_is_refused_when_liveness_cannot_be_determined() {
        let d = tempfile::tempdir().unwrap();
        let paths = Paths::new(d.path());
        let p = Profile {
            path: d.path().join("live"),
            ..profile(false)
        };
        std::fs::create_dir_all(&p.path).unwrap();

        let err = launch(&FakePlatform::failing_scan(), &app_spec::CLAUDE, &p, &paths)
            .unwrap_err()
            .to_string();
        assert!(err.contains("could not check"), "got: {err}");
    }
}
