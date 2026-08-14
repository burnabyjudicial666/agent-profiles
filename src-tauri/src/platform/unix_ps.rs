//! The mirror of `win_proc`: compiled on Windows too, so its parser keeps being
//! tested there, but `scan` shells out to `ps` and nothing on Windows can reach
//! it. `parse` is the pure half the tests actually exercise.
#![cfg_attr(not(unix), allow(dead_code))]

use crate::platform::{RunningProcess, ScanTarget};
use anyhow::Result;
use std::path::PathBuf;

/// The part of a command line before its first option.
///
/// Splitting on whitespace and taking the first token looks equivalent and is
/// not: an application whose path contains a space — `/Applications/T3 Code
/// (Alpha).app/Contents/MacOS/T3 Code (Alpha)` — yields `/Applications/T3`, so
/// its marker never matches and the app is invisible to every scan. Invisible
/// means every profile reads as stopped, which means the guard against a second
/// process on one directory never fires.
fn command_region(args: &str) -> &str {
    match args.find(" -") {
        Some(at) => &args[..at],
        None => args,
    }
}

/// One pass over the process table attributes every line to at most one app, so
/// the cost of scanning does not grow with the number of apps installed.
pub fn parse(raw: &str, targets: &[ScanTarget]) -> Vec<RunningProcess> {
    raw.lines()
        .filter_map(|line| {
            let line = line.trim_start();
            let (pid, args) = line.split_once(' ')?;
            let pid: i32 = pid.trim().parse().ok()?;
            let args = args.trim_start();

            let target = targets
                .iter()
                .find(|t| command_region(args).contains(&t.marker))?;

            // Electron helpers carry the same binary and the same flag as the
            // process that spawned them. Counting one would report a profile as
            // running several times over.
            if args.contains("--type=") {
                return None;
            }

            let profile_dir = args
                .find(target.flag)
                .map(|at| args[at + target.flag.len()..].trim_end())
                .filter(|rest| !rest.is_empty())
                .map(PathBuf::from);

            Some(RunningProcess {
                app_id: target.app_id,
                pid,
                profile_dir,
            })
        })
        .collect()
}

pub fn scan(targets: &[ScanTarget]) -> Result<Vec<RunningProcess>> {
    let out = std::process::Command::new("ps")
        .args(["-axo", "pid=,args="])
        .output()?;
    Ok(parse(&String::from_utf8_lossy(&out.stdout), targets))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    const CLAUDE_BIN: &str = "/Applications/Claude.app/Contents/MacOS/Claude";
    const CODEX_BIN: &str = "/Applications/ChatGPT.app/Contents/MacOS/ChatGPT";

    fn targets() -> Vec<ScanTarget> {
        vec![
            ScanTarget {
                app_id: "claude",
                marker: CLAUDE_BIN.into(),
                flag: "--user-data-dir=",
            },
            ScanTarget {
                app_id: "codex",
                marker: CODEX_BIN.into(),
                flag: "--user-data-dir=",
            },
        ]
    }

    const MAC_FIXTURE: &str = concat!(
        "  501 /Applications/Claude.app/Contents/MacOS/Claude --user-data-dir=/p/work\n",
        "  502 /Applications/Claude.app/Contents/Frameworks/Claude Helper.app/Contents/MacOS/Claude Helper --type=gpu-process --user-data-dir=/p/work\n",
        "  503 /Applications/Claude.app/Contents/MacOS/Claude\n",
        "  504 /usr/bin/unrelated --user-data-dir=/p/work\n",
        "  505 /Applications/Claude.app/Contents/MacOS/Claude --user-data-dir=/p/work2\n",
        "  506 /Applications/Claude.app/Contents/MacOS/Claude --user-data-dir=/Users/h/Library/Application Support/Agent Profiles/claude/profiles/abc\n",
        "  601 /Applications/ChatGPT.app/Contents/MacOS/ChatGPT --user-data-dir=/p/gpt\n",
        "  602 /Applications/ChatGPT.app/Contents/Frameworks/ChatGPT Helper.app/Contents/MacOS/ChatGPT Helper --type=renderer --user-data-dir=/p/gpt\n",
        "  603 /Applications/ChatGPT.app/Contents/MacOS/ChatGPT\n",
    );

    const LINUX_FIXTURE: &str = concat!(
        " 1201 /usr/lib/claude-desktop/claude-desktop --user-data-dir=/home/h/.config/ap/a\n",
        " 1202 /usr/lib/claude-desktop/claude-desktop --type=renderer --user-data-dir=/home/h/.config/ap/a\n",
        " 1203 /usr/lib/claude-desktop/claude-desktop\n",
    );

    #[test]
    fn only_main_processes_are_returned() {
        let pids: Vec<i32> = parse(MAC_FIXTURE, &targets())
            .iter()
            .map(|p| p.pid)
            .collect();
        assert_eq!(pids, vec![501, 503, 505, 506, 601, 603]);
    }

    #[test]
    fn each_process_is_attributed_to_the_app_that_owns_it() {
        let found = parse(MAC_FIXTURE, &targets());
        assert_eq!(found[0].app_id, "claude");
        let gpt: Vec<i32> = found
            .iter()
            .filter(|p| p.app_id == "codex")
            .map(|p| p.pid)
            .collect();
        assert_eq!(gpt, vec![601, 603]);
    }

    #[test]
    fn one_sweep_finds_every_app_at_once() {
        // The property that keeps a third app free: scanning cost is one pass,
        // not one pass per app.
        let found = parse(MAC_FIXTURE, &targets());
        let apps: std::collections::HashSet<_> = found.iter().map(|p| p.app_id).collect();
        assert_eq!(apps.len(), 2);
    }

    #[test]
    fn an_app_that_is_not_a_target_is_invisible() {
        let claude_only = vec![ScanTarget {
            app_id: "claude",
            marker: CLAUDE_BIN.into(),
            flag: "--user-data-dir=",
        }];
        let found = parse(MAC_FIXTURE, &claude_only);
        assert!(found.iter().all(|p| p.app_id == "claude"));
    }

    #[test]
    fn a_profile_directory_containing_spaces_is_captured_whole() {
        let found = parse(MAC_FIXTURE, &targets());
        let with_spaces = found.iter().find(|p| p.pid == 506).unwrap();
        assert_eq!(
            with_spaces.profile_dir,
            Some(PathBuf::from(
                "/Users/h/Library/Application Support/Agent Profiles/claude/profiles/abc"
            ))
        );
    }

    #[test]
    fn a_process_without_the_flag_is_the_stock_profile() {
        let found = parse(MAC_FIXTURE, &targets());
        assert_eq!(found[0].profile_dir, Some(PathBuf::from("/p/work")));
        assert_eq!(found[1].profile_dir, None);
    }

    #[test]
    fn the_linux_binary_name_is_matched_by_substring() {
        let linux = vec![ScanTarget {
            app_id: "claude",
            marker: "claude-desktop".into(),
            flag: "--user-data-dir=",
        }];
        let pids: Vec<i32> = parse(LINUX_FIXTURE, &linux).iter().map(|p| p.pid).collect();
        assert_eq!(pids, vec![1201, 1203]);
    }

    const SPACED: &str = "/Applications/T3 Code (Alpha).app/Contents/MacOS/T3 Code (Alpha)";

    fn spaced_target() -> Vec<ScanTarget> {
        vec![ScanTarget {
            app_id: "t3",
            marker: SPACED.into(),
            flag: "--user-data-dir=",
        }]
    }

    #[test]
    fn a_binary_whose_path_contains_spaces_is_still_matched() {
        // Real case, found by probing T3 Code: the bundle name carries a space
        // and brackets. Taking the first whitespace-delimited token as the
        // command hid the app completely, and an app that cannot be seen is an
        // app whose duplicate-launch guard never fires.
        let line = format!("  601 {SPACED} --user-data-dir=/p/one\n");
        let found = parse(&line, &spaced_target());
        assert_eq!(found.len(), 1, "a spaced binary path must still be found");
        assert_eq!(found[0].profile_dir, Some(PathBuf::from("/p/one")));
    }

    #[test]
    fn a_spaced_binary_running_its_stock_profile_is_recognised() {
        let line = format!("  602 {SPACED}\n");
        let found = parse(&line, &spaced_target());
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].profile_dir, None);
    }

    #[test]
    fn an_unrelated_binary_carrying_the_flag_is_ignored() {
        let found = parse(MAC_FIXTURE, &targets());
        assert!(found.iter().all(|p| p.pid != 504));
    }
}
