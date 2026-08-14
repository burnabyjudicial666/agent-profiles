//! Compiled on every platform so its tests keep running, but the code that
//! calls these helpers is the Windows `Platform` impl, which exists only there.
#![cfg_attr(not(target_os = "windows"), allow(dead_code))]

use crate::app_spec::{WinPath, WinRoot};
use anyhow::{anyhow, Result};
use std::path::{Path, PathBuf};

/// Turns an app's declared locations into real paths. Kept pure — the OS knows
/// what `%LOCALAPPDATA%` means, the app only knows what hangs off it.
pub fn expand(paths: &[WinPath], local: &Path, roaming: &Path) -> Vec<PathBuf> {
    paths
        .iter()
        .map(|path| match path.root {
            WinRoot::Local => local.join(path.rest),
            WinRoot::Roaming => roaming.join(path.rest),
        })
        .collect()
}

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

pub fn looked_in(candidates: &[PathBuf]) -> String {
    candidates
        .iter()
        .map(|candidate| candidate.display().to_string())
        .collect::<Vec<_>>()
        .join(", ")
}

fn env_path(var: &str) -> Result<PathBuf> {
    Ok(PathBuf::from(
        std::env::var(var).map_err(|_| anyhow!("{var} is not set"))?,
    ))
}

#[cfg(target_os = "windows")]
mod imp {
    use super::*;
    use crate::app_spec::Locations;
    use crate::platform::{
        win_proc, FocusHint, FocusOutcome, Platform, RunningProcess, ScanTarget, DATA_DIR_NAME,
    };

    pub struct Windows;

    fn roots() -> Result<(PathBuf, PathBuf)> {
        Ok((env_path("LOCALAPPDATA")?, env_path("APPDATA")?))
    }

    fn here<'a>(
        locations: &'a Locations,
        product: &str,
    ) -> Result<&'a crate::app_spec::WindowsLocation> {
        locations
            .windows
            .as_ref()
            .ok_or_else(|| anyhow!("{product} has not been declared for Windows"))
    }

    /// Whether Windows still has a process under this id.
    ///
    /// An unanswerable question is treated as "yes". Being unable to run
    /// `tasklist` means Windows itself is in a state we cannot reason about,
    /// and leaving an application that ignored the close request running
    /// forever is the worse of the two outcomes.
    fn still_running(pid: i32) -> bool {
        std::process::Command::new("tasklist")
            .args(["/FI", &format!("PID eq {pid}"), "/NH", "/FO", "CSV"])
            .output()
            .map(|out| win_proc::still_listed(&String::from_utf8_lossy(&out.stdout), pid))
            .unwrap_or(true)
    }

    impl Platform for Windows {
        fn declared_here(&self, locations: &Locations) -> bool {
            locations.windows.is_some()
        }

        fn data_root(&self) -> Result<PathBuf> {
            Ok(env_path("APPDATA")?.join(DATA_DIR_NAME))
        }

        fn default_profile_dir(&self, locations: &Locations) -> Result<PathBuf> {
            let (local, roaming) = roots()?;
            let candidates = expand(
                here(locations, "this app")?.default_profiles,
                &local,
                &roaming,
            );
            pick_default_profile(&candidates).ok_or_else(|| {
                anyhow!(
                    "the app's data directory was not found. Looked in: {}",
                    looked_in(&candidates)
                )
            })
        }

        fn binary(&self, locations: &Locations, product: &str) -> Result<PathBuf> {
            let (local, roaming) = roots()?;
            let candidates = expand(here(locations, product)?.binaries, &local, &roaming);
            pick_binary(&candidates).ok_or_else(|| {
                anyhow!(
                    "{product} was not found. Looked in: {}",
                    looked_in(&candidates)
                )
            })
        }

        fn process_marker(&self, locations: &Locations) -> String {
            locations
                .windows
                .as_ref()
                .map(|l| l.process_name.to_string())
                .unwrap_or_default()
        }

        fn scan(&self, targets: &[ScanTarget]) -> Result<Vec<RunningProcess>> {
            win_proc::scan(targets)
        }

        fn link(&self, source: &Path, target: &Path) -> Result<()> {
            std::fs::hard_link(source, target).map_err(|error| {
                anyhow!(
                    "could not link the shared config to {}: {error}. \
                     Both paths must be on the same drive.",
                    target.display()
                )
            })?;
            Ok(())
        }

        fn focus(&self, pid: i32, _hint: &FocusHint) -> Result<FocusOutcome> {
            // `BOOL` lives in windows-core as of the 0.61 reshuffle; only the
            // handle and message types stayed behind in Win32::Foundation.
            use windows::core::BOOL;
            use windows::Win32::Foundation::{HWND, LPARAM};
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
            // Check before insisting. Sleeping out the grace period and then
            // force-killing unconditionally aims `/F` at a pid that Windows may
            // already have handed to something else entirely — and Windows
            // reuses process ids briskly.
            if crate::platform::waited_for_exit(
                || still_running(pid),
                || std::thread::sleep(crate::platform::QUIT_POLL),
            ) {
                return Ok(());
            }
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
    use crate::app_spec;

    #[test]
    fn declared_locations_expand_against_the_right_environment_root() {
        let local = Path::new(r"C:\Users\h\AppData\Local");
        let roaming = Path::new(r"C:\Users\h\AppData\Roaming");
        let expanded = expand(
            app_spec::CLAUDE
                .locations
                .windows
                .as_ref()
                .unwrap()
                .default_profiles,
            local,
            roaming,
        );
        assert!(expanded[0].starts_with(local));
        assert!(expanded[1].starts_with(roaming));
    }

    #[test]
    fn an_app_declared_for_windows_says_where_to_look() {
        // An empty candidate list would report "not found. Looked in: " with
        // nothing after it, which tells the user precisely nothing. Apps not
        // declared for Windows are skipped: absent is not the same as empty.
        for spec in app_spec::all() {
            let Some(windows) = spec.locations.windows.as_ref() else {
                continue;
            };
            assert!(
                !windows.binaries.is_empty(),
                "{} declares no Windows binary",
                spec.id
            );
            assert!(
                !windows.default_profiles.is_empty(),
                "{} declares no Windows profile directory",
                spec.id
            );
        }
    }

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

    #[test]
    fn the_failure_message_lists_every_place_that_was_tried() {
        let looked = looked_in(&[PathBuf::from(r"C:\a"), PathBuf::from(r"C:\b")]);
        assert_eq!(looked, r"C:\a, C:\b");
    }
}
