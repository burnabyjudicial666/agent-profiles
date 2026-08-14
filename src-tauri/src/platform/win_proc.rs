//! Compiled on every platform so its tests keep running, but the code that
//! calls these helpers is the Windows `Platform` impl, which exists only there.
//!
//! The `test` arm matters even on Windows: `scan` shells out to PowerShell, so
//! nothing in the test binary can reach it, and a lib-test build sees it as dead.
//! Its pure counterpart `parse` is what the tests actually exercise.
#![cfg_attr(any(not(target_os = "windows"), test), allow(dead_code))]

use crate::platform::{RunningProcess, ScanTarget};
use anyhow::Result;
use std::path::PathBuf;

pub fn parse(raw: &str, targets: &[ScanTarget]) -> Vec<RunningProcess> {
    raw.lines()
        .skip(1) // CSV header
        .filter_map(|line| {
            let (pid_field, rest) = split_csv_field(line.trim_end_matches('\r'))?;
            let pid: i32 = pid_field.parse().ok()?;
            let (command_line, _) = split_csv_field(rest)?;

            // Windows paths are case-insensitive, and the casing in a command
            // line comes from however the process happened to be started.
            let haystack = command_line.to_ascii_lowercase();
            let target = targets
                .iter()
                .find(|t| haystack.contains(&t.marker.to_ascii_lowercase()))?;

            if command_line.contains("--type=") {
                return None;
            }

            let profile_dir = command_line
                .find(target.flag)
                .map(|at| command_line[at + target.flag.len()..].trim().to_string())
                .filter(|s| !s.is_empty())
                .map(PathBuf::from);

            Some(RunningProcess {
                app_id: target.app_id,
                pid,
                profile_dir,
            })
        })
        .collect()
}

/// Reads one `"..."` CSV field, unescaping doubled quotes, and returns it with
/// the remainder of the line after the following comma.
fn split_csv_field(line: &str) -> Option<(String, &str)> {
    let body = line.strip_prefix('"')?;
    let mut value = String::new();
    let mut chars = body.char_indices();

    while let Some((i, c)) = chars.next() {
        if c != '"' {
            value.push(c);
            continue;
        }
        match body[i + 1..].chars().next() {
            Some('"') => {
                value.push('"');
                chars.next();
            }
            _ => {
                let rest = body[i + 1..].strip_prefix(',').unwrap_or("");
                return Some((value, rest));
            }
        }
    }
    None
}

/// One `Win32_Process` query covering every app, rather than one per app.
pub fn process_filter(targets: &[ScanTarget]) -> String {
    targets
        .iter()
        .map(|t| format!("Name='{}'", t.marker))
        .collect::<Vec<_>>()
        .join(" OR ")
}

pub fn scan(targets: &[ScanTarget]) -> Result<Vec<RunningProcess>> {
    if targets.is_empty() {
        return Ok(Vec::new());
    }
    let query = format!(
        "Get-CimInstance Win32_Process -Filter \"{}\" | \
         Select-Object ProcessId,CommandLine | ConvertTo-Csv -NoTypeInformation",
        process_filter(targets)
    );
    let out = std::process::Command::new("powershell")
        .args(["-NoProfile", "-Command", &query])
        .output()?;
    Ok(parse(&String::from_utf8_lossy(&out.stdout), targets))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn targets() -> Vec<ScanTarget> {
        vec![
            ScanTarget {
                app_id: "claude",
                marker: "claude.exe".into(),
                flag: "--user-data-dir=",
            },
            ScanTarget {
                app_id: "codex",
                marker: "ChatGPT.exe".into(),
                flag: "--user-data-dir=",
            },
        ]
    }

    const FIXTURE: &str = concat!(
        "\"ProcessId\",\"CommandLine\"\r\n",
        "\"4120\",\"\"\"C:\\Users\\h\\AppData\\Local\\AnthropicClaude\\claude.exe\"\" --user-data-dir=C:\\Users\\h\\AppData\\Roaming\\Agent Profiles\\claude\\profiles\\a\"\r\n",
        "\"4188\",\"\"\"C:\\Users\\h\\AppData\\Local\\AnthropicClaude\\claude.exe\"\" --type=renderer --user-data-dir=C:\\Users\\h\\AppData\\Roaming\\Agent Profiles\\claude\\profiles\\a\"\r\n",
        "\"4200\",\"\"\"C:\\Users\\h\\AppData\\Local\\AnthropicClaude\\claude.exe\"\"\"\r\n",
        "\"5100\",\"\"\"C:\\Users\\h\\AppData\\Local\\Programs\\ChatGPT\\ChatGPT.exe\"\" --user-data-dir=C:\\p\\gpt\"\r\n",
    );

    #[test]
    fn helper_processes_are_excluded() {
        let pids: Vec<i32> = parse(FIXTURE, &targets()).iter().map(|p| p.pid).collect();
        assert_eq!(pids, vec![4120, 4200, 5100]);
    }

    #[test]
    fn each_process_is_attributed_to_the_app_that_owns_it() {
        let found = parse(FIXTURE, &targets());
        assert_eq!(found[0].app_id, "claude");
        assert_eq!(found.last().unwrap().app_id, "codex");
    }

    #[test]
    fn an_executable_named_in_different_casing_still_matches() {
        // The command line's casing comes from whoever launched the process, and
        // on Windows that says nothing about the file on disk.
        let shouty = "\"ProcessId\",\"CommandLine\"\r\n\"7000\",\"\"\"C:\\P\\CHATGPT.EXE\"\" --user-data-dir=C:\\p\\x\"\r\n";
        let found = parse(shouty, &targets());
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].app_id, "codex");
    }

    #[test]
    fn a_profile_directory_containing_spaces_is_captured_whole() {
        let found = parse(FIXTURE, &targets());
        assert_eq!(
            found[0].profile_dir,
            Some(PathBuf::from(
                r"C:\Users\h\AppData\Roaming\Agent Profiles\claude\profiles\a"
            ))
        );
    }

    #[test]
    fn a_process_without_the_flag_is_the_stock_profile() {
        assert_eq!(parse(FIXTURE, &targets())[1].profile_dir, None);
    }

    #[test]
    fn a_blank_or_headers_only_output_yields_nothing() {
        assert!(parse("", &targets()).is_empty());
        assert!(parse("\"ProcessId\",\"CommandLine\"\r\n", &targets()).is_empty());
    }

    #[test]
    fn the_query_asks_for_every_app_in_one_filter() {
        assert_eq!(
            process_filter(&targets()),
            "Name='claude.exe' OR Name='ChatGPT.exe'"
        );
    }
}
