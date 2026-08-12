use anyhow::Result;
use std::path::{Path, PathBuf};

pub mod unix_ps;
pub mod win_proc;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(any(target_os = "windows", test))]
mod windows;

#[derive(Debug, Clone, PartialEq)]
pub struct RunningInstance {
    pub pid: i32,
    pub user_data_dir: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum FocusOutcome {
    Focused,
    Unsupported(String),
}

pub trait Platform: Send + Sync {
    fn data_root(&self) -> Result<PathBuf>;
    fn default_profile_dir(&self) -> Result<PathBuf>;
    fn claude_binary(&self) -> Result<PathBuf>;
    fn running_instances(&self) -> Result<Vec<RunningInstance>>;
    fn link_shared_config(&self, profile_dir: &Path, shared: &Path) -> Result<()>;
    /// `profile_id` is carried alongside `pid` because Linux locates the window
    /// by the `--class` it launched with, not by pid. Other backends ignore it.
    fn focus(&self, pid: i32, profile_id: &str) -> Result<FocusOutcome>;
    fn quit(&self, pid: i32) -> Result<()>;
}

pub fn find_for(
    instances: &[RunningInstance],
    profile_dir: &Path,
    is_default: bool,
) -> Option<i32> {
    instances
        .iter()
        .find(|i| match (&i.user_data_dir, is_default) {
            (None, true) => true,
            (Some(dir), false) => dir == profile_dir,
            _ => false,
        })
        .map(|i| i.pid)
}

pub fn current() -> Box<dyn Platform> {
    #[cfg(target_os = "macos")]
    return Box::new(macos::MacOs);
    #[cfg(target_os = "linux")]
    return Box::new(linux::Linux);
    #[cfg(target_os = "windows")]
    return Box::new(windows::Windows);
}

#[cfg(unix)]
pub fn unix_signal_quit(pid: i32) -> Result<()> {
    unsafe { libc::kill(pid, libc::SIGTERM) };
    for _ in 0..100 {
        std::thread::sleep(std::time::Duration::from_millis(100));
        if unsafe { libc::kill(pid, 0) } != 0 {
            return Ok(());
        }
    }
    unsafe { libc::kill(pid, libc::SIGKILL) };
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn instances() -> Vec<RunningInstance> {
        vec![
            RunningInstance {
                pid: 1,
                user_data_dir: Some(PathBuf::from("/p/work")),
            },
            RunningInstance {
                pid: 2,
                user_data_dir: None,
            },
            RunningInstance {
                pid: 3,
                user_data_dir: Some(PathBuf::from("/p/work2")),
            },
        ]
    }

    #[test]
    fn a_profile_matches_only_its_exact_directory() {
        let i = instances();
        assert_eq!(find_for(&i, &PathBuf::from("/p/work"), false), Some(1));
        assert_eq!(find_for(&i, &PathBuf::from("/p/work2"), false), Some(3));
        assert_eq!(find_for(&i, &PathBuf::from("/p/none"), false), None);
    }

    #[test]
    fn the_default_profile_matches_the_process_with_no_flag() {
        assert_eq!(
            find_for(&instances(), &PathBuf::from("/ignored"), true),
            Some(2)
        );
    }

    #[test]
    fn no_flagless_process_means_default_is_not_running() {
        let i = vec![RunningInstance {
            pid: 9,
            user_data_dir: Some(PathBuf::from("/p/x")),
        }];
        assert_eq!(find_for(&i, &PathBuf::from("/ignored"), true), None);
    }
}
