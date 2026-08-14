//! Compiled on every platform so its tests keep running, but the code that
//! calls these helpers is the Linux `Platform` impl, which exists only there.
#![cfg_attr(not(target_os = "linux"), allow(dead_code))]

use crate::platform::DATA_DIR_SLUG;
use std::path::{Path, PathBuf};

pub fn desktop_file_path(applications_dir: &Path, wm_class: &str) -> PathBuf {
    applications_dir.join(format!("{wm_class}.desktop"))
}

pub fn desktop_entry(
    app_label: &str,
    profile_label: &str,
    exec: &str,
    icon: &str,
    wm_class: &str,
) -> String {
    format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Name={app_label} — {profile_label}\n\
         Comment={app_label}, {profile_label} profile\n\
         Exec={exec}\n\
         Icon={icon}\n\
         Terminal=false\n\
         Categories=Utility;\n\
         NoDisplay=true\n\
         StartupWMClass={wm_class}\n",
    )
}

pub fn is_wayland(session_type: Option<&str>) -> bool {
    matches!(session_type, Some(session) if session.eq_ignore_ascii_case("wayland"))
}

pub fn data_root_from(xdg_config_home: Option<&str>, home: &str) -> PathBuf {
    match xdg_config_home {
        Some(path) if !path.is_empty() => PathBuf::from(path).join(DATA_DIR_SLUG),
        _ => PathBuf::from(home).join(".config").join(DATA_DIR_SLUG),
    }
}

#[cfg(target_os = "linux")]
mod imp {
    use super::*;
    use crate::app_spec::{AppSpec, Locations};
    use crate::platform::{unix_ps, FocusHint, FocusOutcome, Platform, RunningProcess, ScanTarget};
    use anyhow::{anyhow, Result};
    use std::process::Command;

    const ICON_NAME: &str = "com.husniadil.agent-profiles";

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

    fn here<'a>(
        locations: &'a Locations,
        product: &str,
    ) -> Result<&'a crate::app_spec::LinuxLocation> {
        locations
            .linux
            .as_ref()
            .ok_or_else(|| anyhow!("{product} has not been declared for Linux"))
    }

    impl Platform for Linux {
        fn declared_here(&self, locations: &Locations) -> bool {
            locations.linux.is_some()
        }

        fn data_root(&self) -> Result<PathBuf> {
            let home = home()?;
            let xdg_config_home = std::env::var_os("XDG_CONFIG_HOME");
            Ok(data_root_from(
                xdg_config_home.as_deref().and_then(|path| path.to_str()),
                &home.display().to_string(),
            ))
        }

        fn default_profile_dir(&self, locations: &Locations) -> Result<PathBuf> {
            Ok(home()?.join(here(locations, "this app")?.default_profile))
        }

        fn binary(&self, locations: &Locations, product: &str) -> Result<PathBuf> {
            let declared = here(locations, product)?;
            let command = declared.command;
            let hint = declared.install_hint;
            let output = Command::new("which").arg(command).output()?;
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !output.status.success() || path.is_empty() {
                return Err(anyhow!(
                    "{product} was not found on PATH as `{command}`. {hint}"
                ));
            }
            Ok(PathBuf::from(path))
        }

        fn process_marker(&self, locations: &Locations) -> String {
            locations
                .linux
                .as_ref()
                .map(|l| l.command.to_string())
                .unwrap_or_default()
        }

        fn scan(&self, targets: &[ScanTarget]) -> Result<Vec<RunningProcess>> {
            unix_ps::scan(targets)
        }

        fn link(&self, source: &Path, target: &Path) -> Result<()> {
            std::os::unix::fs::symlink(source, target)?;
            Ok(())
        }

        fn focus(&self, _pid: i32, hint: &FocusHint) -> Result<FocusOutcome> {
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
            let status = Command::new("xdotool")
                .args(["search", "--class", hint.wm_class, "windowactivate"])
                .status()?;
            if status.success() {
                Ok(FocusOutcome::Focused)
            } else {
                Ok(FocusOutcome::Unsupported(format!(
                    "xdotool found no window with class {}",
                    hint.wm_class
                )))
            }
        }

        fn quit(&self, pid: i32) -> Result<()> {
            crate::platform::unix_signal_quit(pid)
        }

        fn os_launch_args(&self, wm_class: &str) -> Vec<String> {
            vec![format!("--class={wm_class}")]
        }

        fn register_identity(
            &self,
            spec: &AppSpec,
            profile_label: &str,
            wm_class: &str,
        ) -> Result<()> {
            let binary = self.binary(&spec.locations, spec.product)?;
            let directory = applications_dir()?;
            std::fs::create_dir_all(&directory)?;
            let exec = format!("{} --class={wm_class}", binary.display());
            std::fs::write(
                desktop_file_path(&directory, wm_class),
                desktop_entry(spec.label, profile_label, &exec, ICON_NAME, wm_class),
            )?;
            Ok(())
        }

        fn unregister_identity(&self, wm_class: &str) -> Result<()> {
            let path = desktop_file_path(&applications_dir()?, wm_class);
            match std::fs::remove_file(path) {
                Ok(()) => Ok(()),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Err(error) => Err(error.into()),
            }
        }
    }
}

#[cfg(target_os = "linux")]
pub use imp::Linux;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::wm_class;

    #[test]
    fn profiles_that_would_slug_identically_still_get_distinct_identities() {
        let a = wm_class("claude", "id-one");
        let b = wm_class("claude", "id-two");
        assert_ne!(a, b);

        let dir = Path::new("/apps");
        assert_ne!(desktop_file_path(dir, &a), desktop_file_path(dir, &b));
    }

    #[test]
    fn the_same_profile_id_under_two_apps_gets_two_desktop_files() {
        let dir = Path::new("/apps");
        assert_ne!(
            desktop_file_path(dir, &wm_class("claude", "abc")),
            desktop_file_path(dir, &wm_class("codex", "abc"))
        );
    }

    #[test]
    fn the_desktop_entry_declares_a_matching_startup_wm_class() {
        let class = wm_class("claude", "a1b2");
        let entry = desktop_entry(
            "Claude",
            "Kerja",
            "/usr/bin/claude-desktop --class=x",
            "/i/icon.png",
            &class,
        );
        assert!(entry.contains("Name=Claude — Kerja"));
        assert!(entry.contains("StartupWMClass=agent-profiles-claude-a1b2"));
        assert!(entry.contains("NoDisplay=true"));
        assert!(entry.contains("Icon=/i/icon.png"));
        assert!(entry.starts_with("[Desktop Entry]"));
    }

    #[test]
    fn the_entry_is_named_after_whichever_app_it_belongs_to() {
        let entry = desktop_entry("ChatGPT", "Kerja", "/usr/bin/chatgpt", "icon", "c");
        assert!(entry.contains("Name=ChatGPT — Kerja"));
    }

    #[test]
    fn the_desktop_file_lands_in_the_applications_directory() {
        assert_eq!(
            desktop_file_path(
                Path::new("/home/h/.local/share/applications"),
                "agent-profiles-claude-a1b2"
            ),
            PathBuf::from("/home/h/.local/share/applications/agent-profiles-claude-a1b2.desktop")
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
            PathBuf::from("/xdg/agent-profiles")
        );
        assert_eq!(
            data_root_from(None, "/home/h"),
            PathBuf::from("/home/h/.config/agent-profiles")
        );
    }
}
