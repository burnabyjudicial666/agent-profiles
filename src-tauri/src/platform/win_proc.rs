use crate::platform::RunningInstance;
use anyhow::Result;
use std::path::PathBuf;

const FLAG: &str = "--user-data-dir=";

pub fn parse(raw: &str) -> Vec<RunningInstance> {
    raw.lines()
        .skip(1) // CSV header
        .filter_map(|line| {
            let (pid_field, rest) = split_csv_field(line.trim_end_matches('\r'))?;
            let pid: i32 = pid_field.parse().ok()?;
            let (command_line, _) = split_csv_field(rest)?;

            if command_line.contains("--type=") {
                return None;
            }

            let user_data_dir = command_line
                .find(FLAG)
                .map(|at| command_line[at + FLAG.len()..].trim().to_string())
                .filter(|s| !s.is_empty())
                .map(PathBuf::from);

            Some(RunningInstance { pid, user_data_dir })
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

pub fn scan() -> Result<Vec<RunningInstance>> {
    let query = "Get-CimInstance Win32_Process -Filter \"Name='claude.exe'\" | \
                 Select-Object ProcessId,CommandLine | ConvertTo-Csv -NoTypeInformation";
    let out = std::process::Command::new("powershell")
        .args(["-NoProfile", "-Command", query])
        .output()?;
    Ok(parse(&String::from_utf8_lossy(&out.stdout)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    const FIXTURE: &str = concat!(
        "\"ProcessId\",\"CommandLine\"\r\n",
        "\"4120\",\"\"\"C:\\Users\\h\\AppData\\Local\\AnthropicClaude\\claude.exe\"\" --user-data-dir=C:\\Users\\h\\AppData\\Roaming\\Claude Profiles\\profiles\\a\"\r\n",
        "\"4188\",\"\"\"C:\\Users\\h\\AppData\\Local\\AnthropicClaude\\claude.exe\"\" --type=renderer --user-data-dir=C:\\Users\\h\\AppData\\Roaming\\Claude Profiles\\profiles\\a\"\r\n",
        "\"4200\",\"\"\"C:\\Users\\h\\AppData\\Local\\AnthropicClaude\\claude.exe\"\"\"\r\n",
    );

    #[test]
    fn helper_processes_are_excluded() {
        let pids: Vec<i32> = parse(FIXTURE).iter().map(|i| i.pid).collect();
        assert_eq!(pids, vec![4120, 4200]);
    }

    #[test]
    fn a_user_data_dir_containing_spaces_is_captured_whole() {
        let found = parse(FIXTURE);
        assert_eq!(
            found[0].user_data_dir,
            Some(PathBuf::from(
                r"C:\Users\h\AppData\Roaming\Claude Profiles\profiles\a"
            ))
        );
    }

    #[test]
    fn a_process_without_the_flag_is_the_default_profile() {
        assert_eq!(parse(FIXTURE)[1].user_data_dir, None);
    }

    #[test]
    fn a_blank_or_headers_only_output_yields_nothing() {
        assert!(parse("").is_empty());
        assert!(parse("\"ProcessId\",\"CommandLine\"\r\n").is_empty());
    }
}
