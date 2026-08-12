//! Compiled on every platform so its tests keep running, but the code that
//! calls these helpers is the Linux `Platform` impl, which exists only there.
#![cfg_attr(not(target_os = "linux"), allow(dead_code))]

use crate::profile_store::Profile;
use std::path::{Path, PathBuf};

pub const BINARY_NAME: &str = "claude-desktop";

pub fn wm_class(profile_id: &str) -> String {
    format!("claude-profiles-{profile_id}")
}

pub fn desktop_file_path(applications_dir: &Path, profile_id: &str) -> PathBuf {
    applications_dir.join(format!("{}.desktop", wm_class(profile_id)))
}

pub fn desktop_entry(profile: &Profile, exec: &str, icon: &str) -> String {
    format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Name=Claude — {label}\n\
         Comment=Claude Desktop, {label} profile\n\
         Exec={exec}\n\
         Icon={icon}\n\
         Terminal=false\n\
         Categories=Utility;\n\
         NoDisplay=true\n\
         StartupWMClass={class}\n",
        label = profile.label,
        class = wm_class(&profile.id)
    )
}

pub fn is_wayland(session_type: Option<&str>) -> bool {
    matches!(session_type, Some(session) if session.eq_ignore_ascii_case("wayland"))
}

pub fn data_root_from(xdg_config_home: Option<&str>, home: &str) -> PathBuf {
    match xdg_config_home {
        Some(path) if !path.is_empty() => PathBuf::from(path).join("claude-profiles"),
        _ => PathBuf::from(home).join(".config").join("claude-profiles"),
    }
}

#[cfg(target_os = "linux")]
mod imp {
    use super::*;
    use crate::platform::{unix_ps, FocusOutcome, Platform, RunningInstance};
    use anyhow::{anyhow, Result};
    use std::process::Command;

    const ICON_NAME: &str = "com.husniadil.claude-profiles";

    pub struct Linux;

    fn home() -> Result<PathBuf> {
        std::env::var_os("HOME")
            .map(PathBuf::from)
            .ok_or_else(|| anyhow!("HOME is not set"))
    }

    fn which(tool: &str) -> bool {
        Command::new("which")
            .arg(tool)
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    }

    fn applications_dir() -> Result<PathBuf> {
        Ok(home()?.join(".local").join("share").join("applications"))
    }

    fn write_desktop_entry(profile: &Profile, binary: &Path, icon: &Path) -> Result<()> {
        let directory = applications_dir()?;
        std::fs::create_dir_all(&directory)?;
        let exec = format!("{} --class={}", binary.display(), wm_class(&profile.id));
        std::fs::write(
            desktop_file_path(&directory, &profile.id),
            desktop_entry(profile, &exec, &icon.display().to_string()),
        )?;
        Ok(())
    }

    fn remove_desktop_entry(profile_id: &str) -> Result<()> {
        let path = desktop_file_path(&applications_dir()?, profile_id);
        match std::fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error.into()),
        }
    }

    impl Platform for Linux {
        fn data_root(&self) -> Result<PathBuf> {
            let home = home()?;
            let xdg_config_home = std::env::var_os("XDG_CONFIG_HOME");
            Ok(data_root_from(
                xdg_config_home.as_deref().and_then(|path| path.to_str()),
                &home.display().to_string(),
            ))
        }

        fn default_profile_dir(&self) -> Result<PathBuf> {
            Ok(home()?.join(".config").join("Claude"))
        }

        fn claude_binary(&self) -> Result<PathBuf> {
            let output = Command::new("which").arg(BINARY_NAME).output()?;
            if !output.status.success() {
                return Err(anyhow!(
                    "Claude Desktop was not found on PATH as `{BINARY_NAME}`. \
                     Install it with: sudo apt install claude-desktop"
                ));
            }
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if path.is_empty() {
                return Err(anyhow!(
                    "Claude Desktop was not found on PATH as `{BINARY_NAME}`. \
                     Install it with: sudo apt install claude-desktop"
                ));
            }
            Ok(PathBuf::from(path))
        }

        fn running_instances(&self) -> Result<Vec<RunningInstance>> {
            unix_ps::scan(&[BINARY_NAME])
        }

        fn link_shared_config(&self, profile_dir: &Path, shared: &Path) -> Result<()> {
            std::os::unix::fs::symlink(shared, profile_dir.join(crate::paths::CONFIG_FILENAME))?;
            Ok(())
        }

        fn focus(&self, _pid: i32, profile_id: &str) -> Result<FocusOutcome> {
            if is_wayland(std::env::var("XDG_SESSION_TYPE").ok().as_deref()) {
                return Ok(FocusOutcome::Unsupported(
                    "Wayland does not let one app raise another's window — use this profile's \
                     own entry in your taskbar or alt-tab"
                        .into(),
                ));
            }
            if !which("xdotool") {
                return Ok(FocusOutcome::Unsupported(
                    "install xdotool to focus from here, or use this profile's taskbar entry"
                        .into(),
                ));
            }
            let class = wm_class(profile_id);
            let status = Command::new("xdotool")
                .args(["search", "--class", &class, "windowactivate"])
                .status()?;
            if status.success() {
                Ok(FocusOutcome::Focused)
            } else {
                Ok(FocusOutcome::Unsupported(format!(
                    "xdotool found no window with class {class}"
                )))
            }
        }

        fn quit(&self, pid: i32) -> Result<()> {
            crate::platform::unix_signal_quit(pid)
        }

        fn extra_launch_args(&self, profile: &Profile) -> Vec<String> {
            vec![format!("--class={}", wm_class(&profile.id))]
        }

        fn register_identity(&self, profile: &Profile) -> Result<()> {
            let binary = self.claude_binary()?;
            write_desktop_entry(profile, &binary, Path::new(ICON_NAME))
        }

        fn unregister_identity(&self, profile: &Profile) -> Result<()> {
            remove_desktop_entry(&profile.id)
        }
    }
}

#[cfg(target_os = "linux")]
pub use imp::Linux;

#[cfg(test)]
mod tests {
    use super::*;

    fn profile(id: &str, label: &str) -> Profile {
        Profile {
            id: id.into(),
            label: label.into(),
            path: PathBuf::from("/p").join(id),
            is_default: false,
            last_known_account_uuid: None,
        }
    }

    #[test]
    fn the_class_is_keyed_on_the_profile_id() {
        assert_eq!(wm_class("a1b2"), "claude-profiles-a1b2");
    }

    #[test]
    fn labels_that_would_slug_identically_still_get_distinct_identities() {
        let a = profile("id-one", "Work A");
        let b = profile("id-two", "work  a");
        assert_ne!(wm_class(&a.id), wm_class(&b.id));

        let dir = Path::new("/apps");
        assert_ne!(desktop_file_path(dir, &a.id), desktop_file_path(dir, &b.id));
    }

    #[test]
    fn the_desktop_entry_declares_a_matching_startup_wm_class() {
        let p = profile("a1b2", "Kerja");
        let entry = desktop_entry(&p, "/usr/bin/claude-desktop --class=x", "/i/icon.png");
        assert!(entry.contains("Name=Claude — Kerja"));
        assert!(entry.contains("StartupWMClass=claude-profiles-a1b2"));
        assert!(entry.contains("NoDisplay=true"));
        assert!(entry.contains("Icon=/i/icon.png"));
        assert!(entry.starts_with("[Desktop Entry]"));
    }

    #[test]
    fn the_desktop_file_lands_in_the_applications_directory() {
        assert_eq!(
            desktop_file_path(Path::new("/home/h/.local/share/applications"), "a1b2"),
            PathBuf::from("/home/h/.local/share/applications/claude-profiles-a1b2.desktop")
        );
    }

    #[test]
    fn wayland_is_detected_from_the_session_type() {
        assert!(is_wayland(Some("wayland")));
        assert!(!is_wayland(Some("x11")));
        assert!(!is_wayland(None));
    }

    #[test]
    fn the_config_root_honours_xdg_config_home() {
        assert_eq!(
            data_root_from(Some("/xdg"), "/home/h"),
            PathBuf::from("/xdg/claude-profiles")
        );
        assert_eq!(
            data_root_from(None, "/home/h"),
            PathBuf::from("/home/h/.config/claude-profiles")
        );
    }
}
