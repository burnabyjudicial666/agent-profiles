//! The mirror of `win_proc`: compiled on Windows too, so its parser keeps being
//! tested there, but `scan` shells out to `ps` and nothing on Windows can reach
//! it. `parse` is the pure half the tests actually exercise.
#![cfg_attr(not(unix), allow(dead_code))]

use crate::platform::RunningInstance;
use anyhow::Result;
use std::path::PathBuf;

const FLAG: &str = "--user-data-dir=";

pub fn parse(raw: &str, main_binaries: &[&str]) -> Vec<RunningInstance> {
    raw.lines()
        .filter_map(|line| {
            let line = line.trim_start();
            let (pid, args) = line.split_once(' ')?;
            let pid: i32 = pid.trim().parse().ok()?;
            let args = args.trim_start();

            let command = args.split_whitespace().next().unwrap_or("");
            if !main_binaries.iter().any(|b| command.contains(b)) {
                return None;
            }
            if args.contains("--type=") {
                return None;
            }

            let user_data_dir = args
                .find(FLAG)
                .map(|at| args[at + FLAG.len()..].trim_end())
                .filter(|rest| !rest.is_empty())
                .map(PathBuf::from);

            Some(RunningInstance { pid, user_data_dir })
        })
        .collect()
}

pub fn scan(main_binaries: &[&str]) -> Result<Vec<RunningInstance>> {
    let out = std::process::Command::new("ps")
        .args(["-axo", "pid=,args="])
        .output()?;
    Ok(parse(&String::from_utf8_lossy(&out.stdout), main_binaries))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    const MAC: &str = "/Applications/Claude.app/Contents/MacOS/Claude";

    const MAC_FIXTURE: &str = concat!(
        "  501 /Applications/Claude.app/Contents/MacOS/Claude --user-data-dir=/p/work\n",
        "  502 /Applications/Claude.app/Contents/Frameworks/Claude Helper.app/Contents/MacOS/Claude Helper --type=gpu-process --user-data-dir=/p/work\n",
        "  503 /Applications/Claude.app/Contents/MacOS/Claude\n",
        "  504 /usr/bin/unrelated --user-data-dir=/p/work\n",
        "  505 /Applications/Claude.app/Contents/MacOS/Claude --user-data-dir=/p/work2\n",
        "  506 /Applications/Claude.app/Contents/MacOS/Claude --user-data-dir=/Users/h/Library/Application Support/Claude Profiles/profiles/abc\n",
    );

    const LINUX_FIXTURE: &str = concat!(
        " 1201 /usr/lib/claude-desktop/claude-desktop --user-data-dir=/home/h/.config/cp/profiles/a\n",
        " 1202 /usr/lib/claude-desktop/claude-desktop --type=renderer --user-data-dir=/home/h/.config/cp/profiles/a\n",
        " 1203 /usr/lib/claude-desktop/claude-desktop\n",
    );

    #[test]
    fn only_main_processes_are_returned() {
        let pids: Vec<i32> = parse(MAC_FIXTURE, &[MAC]).iter().map(|i| i.pid).collect();
        assert_eq!(pids, vec![501, 503, 505, 506]);
    }

    #[test]
    fn a_user_data_dir_containing_spaces_is_captured_whole() {
        let found = parse(MAC_FIXTURE, &[MAC]);
        assert_eq!(
            found.last().unwrap().user_data_dir,
            Some(PathBuf::from(
                "/Users/h/Library/Application Support/Claude Profiles/profiles/abc"
            ))
        );
    }

    #[test]
    fn the_user_data_dir_argument_is_extracted() {
        let found = parse(MAC_FIXTURE, &[MAC]);
        assert_eq!(found[0].user_data_dir, Some(PathBuf::from("/p/work")));
        assert_eq!(found[1].user_data_dir, None);
    }

    #[test]
    fn the_linux_binary_name_is_matched_by_substring() {
        let found = parse(LINUX_FIXTURE, &["claude-desktop"]);
        let pids: Vec<i32> = found.iter().map(|i| i.pid).collect();
        assert_eq!(pids, vec![1201, 1203]);
    }

    #[test]
    fn an_unrelated_binary_carrying_the_flag_is_ignored() {
        let found = parse(MAC_FIXTURE, &[MAC]);
        assert!(found.iter().all(|i| i.pid != 504));
    }
}
