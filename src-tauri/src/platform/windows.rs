//! Compiled on every platform so its tests keep running, but the code that
//! calls these helpers is the Windows `Platform` impl, which exists only there.
#![cfg_attr(not(target_os = "windows"), allow(dead_code))]

use anyhow::{anyhow, Result};
use std::path::PathBuf;

pub fn pick_default_profile(candidates: &[PathBuf]) -> Option<PathBuf> {
    candidates
        .iter()
        .find(|candidate| candidate.is_dir())
        .cloned()
}

pub fn pick_binary(candidates: &[PathBuf]) -> Option<PathBuf> {
    candidates
        .iter()
        .find(|candidate| candidate.is_file())
        .cloned()
}

fn env_path(var: &str) -> Result<PathBuf> {
    Ok(PathBuf::from(
        std::env::var(var).map_err(|_| anyhow!("{var} is not set"))?,
    ))
}

pub fn default_profile_candidates() -> Result<Vec<PathBuf>> {
    let local = env_path("LOCALAPPDATA")?;
    let roaming = env_path("APPDATA")?;
    Ok(vec![
        local
            .join("Packages")
            .join("Claude_pzs8sxrjxfjjc")
            .join("LocalCache")
            .join("Roaming")
            .join("Claude"),
        roaming.join("Claude"),
    ])
}

pub fn binary_candidates() -> Result<Vec<PathBuf>> {
    let local = env_path("LOCALAPPDATA")?;
    Ok(vec![
        local.join("AnthropicClaude").join("claude.exe"),
        local
            .join("Microsoft")
            .join("WindowsApps")
            .join("claude.exe"),
    ])
}

#[cfg(target_os = "windows")]
mod imp {
    use super::*;
    use crate::platform::{win_proc, FocusOutcome, Platform, RunningInstance};
    use std::path::Path;

    pub struct Windows;

    impl Platform for Windows {
        fn data_root(&self) -> Result<PathBuf> {
            Ok(env_path("APPDATA")?.join("Claude Profiles"))
        }

        fn default_profile_dir(&self) -> Result<PathBuf> {
            let candidates = default_profile_candidates()?;
            pick_default_profile(&candidates).ok_or_else(|| {
                anyhow!(
                    "Claude Desktop's data directory was not found. Looked in: {}",
                    candidates
                        .iter()
                        .map(|candidate| candidate.display().to_string())
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            })
        }

        fn claude_binary(&self) -> Result<PathBuf> {
            let candidates = binary_candidates()?;
            pick_binary(&candidates).ok_or_else(|| {
                anyhow!(
                    "Claude Desktop was not found. Looked in: {}",
                    candidates
                        .iter()
                        .map(|candidate| candidate.display().to_string())
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            })
        }

        fn running_instances(&self) -> Result<Vec<RunningInstance>> {
            win_proc::scan()
        }

        fn link_shared_config(&self, profile_dir: &Path, shared: &Path) -> Result<()> {
            std::fs::hard_link(shared, profile_dir.join(crate::paths::CONFIG_FILENAME)).map_err(
                |error| {
                    anyhow!(
                        "could not link the shared config into {}: {error}. \
                         Both paths must be on the same drive.",
                        profile_dir.display()
                    )
                },
            )?;
            Ok(())
        }

        fn focus(&self, pid: i32, _profile_id: &str) -> Result<FocusOutcome> {
            use windows::Win32::Foundation::{BOOL, HWND, LPARAM};
            use windows::Win32::UI::WindowsAndMessaging::{
                EnumWindows, GetWindowThreadProcessId, IsWindowVisible, SetForegroundWindow,
            };

            struct Search {
                pid: u32,
                found: Option<HWND>,
            }

            unsafe extern "system" fn callback(hwnd: HWND, lparam: LPARAM) -> BOOL {
                let search = &mut *(lparam.0 as *mut Search);
                let mut owner = 0u32;
                GetWindowThreadProcessId(hwnd, Some(&mut owner));
                if owner == search.pid && IsWindowVisible(hwnd).as_bool() {
                    search.found = Some(hwnd);
                    return BOOL(0);
                }
                BOOL(1)
            }

            let mut search = Search {
                pid: pid as u32,
                found: None,
            };
            unsafe {
                let _ = EnumWindows(Some(callback), LPARAM(&mut search as *mut _ as isize));
            }

            match search.found {
                Some(hwnd) => {
                    let raised = unsafe { SetForegroundWindow(hwnd) }.as_bool();
                    if raised {
                        Ok(FocusOutcome::Focused)
                    } else {
                        Ok(FocusOutcome::Unsupported(
                            "Windows refused to bring the window forward; \
                             click its taskbar entry instead"
                                .into(),
                        ))
                    }
                }
                None => Ok(FocusOutcome::Unsupported(
                    "no visible window was found for this instance".into(),
                )),
            }
        }

        fn quit(&self, pid: i32) -> Result<()> {
            std::process::Command::new("taskkill")
                .args(["/PID", &pid.to_string()])
                .status()?;
            std::thread::sleep(std::time::Duration::from_secs(10));
            let _ = std::process::Command::new("taskkill")
                .args(["/PID", &pid.to_string(), "/F"])
                .status();
            Ok(())
        }
    }
}

#[cfg(target_os = "windows")]
pub use imp::Windows;

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn the_first_existing_candidate_wins() {
        let d = tempfile::tempdir().unwrap();
        let msix = d.path().join("msix");
        let classic = d.path().join("classic");
        std::fs::create_dir_all(&classic).unwrap();

        assert_eq!(
            pick_default_profile(&[msix.clone(), classic.clone()]),
            Some(classic)
        );

        std::fs::create_dir_all(&msix).unwrap();
        assert_eq!(
            pick_default_profile(&[msix.clone(), d.path().join("classic")]),
            Some(msix)
        );
    }

    #[test]
    fn no_existing_candidate_yields_none() {
        assert_eq!(pick_default_profile(&[PathBuf::from("/nope")]), None);
        assert_eq!(pick_binary(&[PathBuf::from("/nope/claude.exe")]), None);
    }

    #[test]
    fn a_binary_candidate_must_be_a_file_not_a_directory() {
        let d = tempfile::tempdir().unwrap();
        let dir_named_like_exe = d.path().join("claude.exe");
        std::fs::create_dir_all(&dir_named_like_exe).unwrap();
        assert_eq!(pick_binary(&[dir_named_like_exe]), None);
    }
}
