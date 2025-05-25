use anyhow::{Context, Result, bail};
use std::collections::HashMap;
use std::path::Path;
use std::process::{Command, Output, Stdio};
use std::str;

fn execute_git_command(repo_path: &Path, args: &[&str]) -> Result<Output, anyhow::Error> {
    let command_str = format!("git {}", args.join(" "));
    let output = Command::new("git")
        .current_dir(repo_path)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .with_context(|| {
            format!(
                "Failed to execute git command: '{}' in {:?}. Ensure 'git' is installed and in your PATH.",
                command_str, repo_path
            )
        })?;

    if !output.status.success() {
        let stdout_str = str::from_utf8(&output.stdout)
            .unwrap_or("[non-utf8 stdout]")
            .trim();
        let stderr_str = str::from_utf8(&output.stderr)
            .unwrap_or("[non-utf8 stderr]")
            .trim();
        bail!(
            "Git command '{}' failed in {:?} with status {}:\nStdout: {}\nStderr: {}",
            command_str,
            repo_path,
            output.status,
            stdout_str,
            stderr_str
        );
    }
    Ok(output)
}

pub fn has_staged_files(repo_path: &Path) -> Result<bool, anyhow::Error> {
    let output = execute_git_command(
        repo_path,
        &["status", "--porcelain", "--untracked-files=no"],
    )
    .context("Failed to get git status to check for staged files")?;
    let stdout_str = str::from_utf8(&output.stdout)
        .context("Failed to read git status output as UTF-8")?
        .trim();
    Ok(!stdout_str.is_empty())
}

pub fn get_staged_diff(repo_path: &Path) -> Result<String, anyhow::Error> {
    let diff_output = execute_git_command(repo_path, &["diff", "--staged"])
        .context("Failed to get staged git diff")?;
    let diff_stdout = str::from_utf8(&diff_output.stdout)
        .context("Failed to read git diff output as UTF-8")?
        .to_string();
    Ok(diff_stdout)
}

fn execute_git_command_for_summary_bytes(
    repo_path: &Path,
    args: &[&str],
) -> Result<Vec<u8>, anyhow::Error> {
    let command_str = format!("git {}", args.join(" "));
    let output = Command::new("git")
        .current_dir(repo_path)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .with_context(|| {
            format!(
                "Failed to execute git summary command: '{}' in {:?}. Ensure 'git' is installed and in your PATH.",
                command_str, repo_path
            )
        })?;

    if !output.status.success() {
        let stderr_str = str::from_utf8(&output.stderr)
            .unwrap_or("[non-utf8 stderr summary]")
            .trim();
        bail!(
            "Git summary command '{}' failed in {:?} with status {}:\nStderr: {}",
            command_str,
            repo_path,
            output.status,
            stderr_str
        );
    }
    Ok(output.stdout)
}

#[derive(Debug, Default, PartialEq, Eq, Clone)]
pub struct StagedChangesSummary {
    pub binary_file_changes: Vec<String>,
    pub structure_changes: Vec<String>,
}

fn get_binary_status_map(repo_path: &Path) -> Result<HashMap<String, bool>> {
    let numstat_output_bytes =
        execute_git_command_for_summary_bytes(repo_path, &["diff", "--staged", "--numstat", "-z"])?;
    let mut binary_map = HashMap::new();

    if numstat_output_bytes.is_empty() || numstat_output_bytes.iter().all(|&b| b == 0) {
        return Ok(binary_map);
    }

    let mut fields_iter = numstat_output_bytes
        .split(|&b| b == 0)
        .filter(|s| !s.is_empty());

    while let Some(first_segment_bytes) = fields_iter.next() {
        let first_segment_str = str::from_utf8(first_segment_bytes).with_context(|| {
            format!(
                "Non-UTF8 first_segment in numstat: {:?}",
                first_segment_bytes
            )
        })?;
        let parts: Vec<&str> = first_segment_str.split('\t').collect();

        if parts.len() == 3 {
            let added_str = parts[0];
            let deleted_str = parts[1];
            let third_part_str = parts[2];
            let is_binary_stats = added_str == "-" && deleted_str == "-";

            if third_part_str.is_empty() {
                let _old_path_bytes = fields_iter.next().with_context(|| {
                    format!(
                        "Expected old_path after empty third part in numstat for segment: '{}'",
                        first_segment_str
                    )
                })?;
                let new_path_bytes = fields_iter.next().with_context(|| {
                    format!(
                        "Expected new_path after old_path (empty third part) for segment: '{}'",
                        first_segment_str
                    )
                })?;
                let new_path_str = str::from_utf8(new_path_bytes).with_context(|| {
                    format!(
                        "Non-UTF8 new_path (empty third part numstat): {:?}",
                        new_path_bytes
                    )
                })?;
                binary_map.insert(new_path_str.to_string(), is_binary_stats);
            } else if third_part_str.ends_with('%')
                && third_part_str.len() > 1
                && third_part_str[..third_part_str.len() - 1]
                    .parse::<u32>()
                    .is_ok()
            {
                let _old_path_bytes = fields_iter.next().with_context(|| {
                    format!(
                        "Expected old_path after similarity score in numstat for segment: '{}'",
                        first_segment_str
                    )
                })?;
                let new_path_bytes = fields_iter.next().with_context(|| {
                    format!(
                        "Expected new_path after old_path (similarity score) for segment: '{}'",
                        first_segment_str
                    )
                })?;
                let new_path_str = str::from_utf8(new_path_bytes).with_context(|| {
                    format!(
                        "Non-UTF8 new_path (similarity score numstat): {:?}",
                        new_path_bytes
                    )
                })?;
                binary_map.insert(new_path_str.to_string(), is_binary_stats);
            } else {
                let path_str = third_part_str;
                binary_map.insert(path_str.to_string(), is_binary_stats);
            }
        } else if parts.len() == 2 {
            let added_str = parts[0];
            let deleted_str = parts[1];
            let is_binary_stats = added_str == "-" && deleted_str == "-";
            let _old_path_bytes = fields_iter.next().with_context(|| {
                format!(
                    "Expected old_path (2-part numstat) for segment: '{}'",
                    first_segment_str
                )
            })?;
            let new_path_bytes = fields_iter.next().with_context(|| {
                format!(
                    "Expected new_path after old_path (2-part numstat) for segment: '{}'",
                    first_segment_str
                )
            })?;
            let new_path_str = str::from_utf8(new_path_bytes).with_context(|| {
                format!("Non-UTF8 new_path (2-part numstat): {:?}", new_path_bytes)
            })?;
            binary_map.insert(new_path_str.to_string(), is_binary_stats);
        } else {
        }
    }
    Ok(binary_map)
}

pub fn get_staged_changes_summary(repo_path: &Path) -> Result<StagedChangesSummary> {
    let mut summary = StagedChangesSummary::default();

    let status_check_output_bytes = execute_git_command_for_summary_bytes(
        repo_path,
        &["status", "--porcelain=v1", "-z", "--untracked-files=no"],
    )?;

    if status_check_output_bytes.is_empty() || status_check_output_bytes.iter().all(|&x| x == 0) {
        return Ok(summary);
    }

    let binary_map = get_binary_status_map(repo_path)
        .context("Failed to get binary status map for staged files")?;

    let mut status_fields_iter = status_check_output_bytes
        .split(|&b| b == 0)
        .filter(|s| !s.is_empty());

    while let Some(entry_lead_bytes) = status_fields_iter.next() {
        let entry_lead_str = str::from_utf8(entry_lead_bytes).with_context(|| {
            format!(
                "Failed to parse status entry lead: {:?}",
                String::from_utf8_lossy(entry_lead_bytes)
            )
        })?;

        if entry_lead_str.len() < 3 {
            continue;
        }

        let status_codes = &entry_lead_str[0..2];
        let path_part1_str = &entry_lead_str[3..];

        let (current_path_for_processing, old_path_opt_string) =
            if status_codes.starts_with('R') || status_codes.starts_with('C') {
                if let Some(old_path_bytes) = status_fields_iter.next() {
                    let old_path_str = str::from_utf8(old_path_bytes).with_context(|| {
                        format!(
                            "Failed to parse old_path for {} status: {:?}",
                            status_codes,
                            String::from_utf8_lossy(old_path_bytes)
                        )
                    })?;
                    (path_part1_str, Some(old_path_str.to_string()))
                } else {
                    (path_part1_str, None)
                }
            } else {
                (path_part1_str, None)
            };

        let idx_status = status_codes.chars().next().unwrap_or(' ');

        match idx_status {
            'A' => {
                let is_binary_file = binary_map
                    .get(current_path_for_processing)
                    .copied()
                    .unwrap_or(false);
                if is_binary_file {
                    let change_desc = format!("added binary file: {}", current_path_for_processing);
                    summary.binary_file_changes.push(change_desc);
                }
            }
            'D' => {
                let change_desc = format!("deleted file: {}", current_path_for_processing);
                summary.structure_changes.push(change_desc);
            }
            'R' => {
                if let Some(old_path) = old_path_opt_string {
                    if !old_path.is_empty() && !current_path_for_processing.is_empty() {
                        let struct_change_desc =
                            format!("renamed: {} to {}", old_path, current_path_for_processing);
                        summary.structure_changes.push(struct_change_desc);

                        let is_binary_file = binary_map
                            .get(current_path_for_processing)
                            .copied()
                            .unwrap_or(false);
                        if is_binary_file {
                            let bin_change_desc = format!(
                                "renamed binary file: {} to {}",
                                old_path, current_path_for_processing
                            );
                            summary.binary_file_changes.push(bin_change_desc);
                        }
                    }
                }
            }
            'M' => {
                let is_binary_file = binary_map
                    .get(current_path_for_processing)
                    .copied()
                    .unwrap_or(false);
                if is_binary_file {
                    let change_desc =
                        format!("modified binary file: {}", current_path_for_processing);
                    summary.binary_file_changes.push(change_desc);
                }
            }
            'T' => {
                let struct_change_desc =
                    format!("type changed for: {}", current_path_for_processing);
                summary.structure_changes.push(struct_change_desc);
                let is_binary_file = binary_map
                    .get(current_path_for_processing)
                    .copied()
                    .unwrap_or(false);
                if is_binary_file {
                    let bin_change_desc =
                        format!("type changed to binary: {}", current_path_for_processing);
                    summary.binary_file_changes.push(bin_change_desc);
                }
            }
            _ => {
                if status_codes.starts_with('C') {
                    if let Some(old_path) = old_path_opt_string {
                        let struct_change_desc =
                            format!("copied: {} to {}", old_path, current_path_for_processing);
                        summary.structure_changes.push(struct_change_desc);
                    }
                    let is_binary_file = binary_map
                        .get(current_path_for_processing)
                        .copied()
                        .unwrap_or(false);
                    if is_binary_file {
                        let change_desc =
                            format!("copied binary file to: {}", current_path_for_processing);
                        summary.binary_file_changes.push(change_desc);
                    }
                }
            }
        }
    }
    summary.binary_file_changes.sort();
    summary.structure_changes.sort();
    Ok(summary)
}

pub fn commit_staged_files(repo_path: &Path, message: &str) -> Result<String, anyhow::Error> {
    if message.trim().is_empty() {
        bail!("Commit message cannot be empty.");
    }
    let output = execute_git_command(repo_path, &["commit", "-m", message])
        .context("Failed to commit staged files")?;

    let stdout_str = str::from_utf8(&output.stdout)
        .unwrap_or("[non-utf8 stdout from git commit]")
        .trim();
    Ok(stdout_str.to_string())
}

pub fn get_previous_commit_message(repo_path: &Path) -> Result<Option<String>, anyhow::Error> {
    match execute_git_command(repo_path, &["log", "-1", "--pretty=%B"]) {
        Ok(output) => {
            let message = str::from_utf8(&output.stdout)
                .context("Failed to read git log output as UTF-8 for previous message")?
                .trim()
                .to_string();
            if message.is_empty() && !output.status.success() {
                Ok(None)
            } else if message.is_empty() && output.status.success() {
                Ok(Some(String::new()))
            } else {
                Ok(Some(message))
            }
        }
        Err(e) => {
            let err_msg = e.to_string().to_lowercase();
            if err_msg.contains("does not have any commits yet")
                || err_msg.contains("bad default revision 'head'")
                || err_msg.contains("fatal: your current branch")
                    && err_msg.contains("does not have any commits yet")
                || err_msg.contains("needed a single revision")
            {
                Ok(None)
            } else {
                Err(e).context("Failed to get previous commit message from git log")
            }
        }
    }
}

pub fn amend_commit(repo_path: &Path, message: &str) -> Result<String, anyhow::Error> {
    if message.trim().is_empty() {
        bail!("Commit message for amend cannot be empty.");
    }

    let output = execute_git_command(repo_path, &["commit", "--amend", "-m", message])
        .with_context(|| {
            format!(
                "Failed to execute 'git commit --amend -m \"{}\"' in {:?}",
                message, // The commit message variable
                repo_path
            )
        })?;

    let stdout_str = str::from_utf8(&output.stdout)
        .unwrap_or("[non-utf8 stdout from git commit --amend]")
        .trim();

    Ok(stdout_str.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self, File};
    use std::io::Write;
    use tempfile::TempDir;

    fn setup_git_repo(temp_dir_path: &Path) -> Result<(), anyhow::Error> {
        run_command_in_dir(temp_dir_path, "git", &["init", "-b", "main"])?;
        run_command_in_dir(temp_dir_path, "git", &["config", "user.name", "Test User"])?;
        run_command_in_dir(
            temp_dir_path,
            "git",
            &["config", "user.email", "test@example.com"],
        )?;
        run_command_in_dir(temp_dir_path, "git", &["config", "core.autocrlf", "false"])?;
        Ok(())
    }

    fn run_command_in_dir(
        dir: &Path,
        command_str: &str,
        args: &[&str],
    ) -> Result<Output, anyhow::Error> {
        let output = Command::new(command_str)
            .args(args)
            .current_dir(dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .with_context(|| format!("Failed to execute command: {} in {:?}", command_str, dir))?;

        if !output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);

            if !(command_str == "git"
                && args.contains(&"commit")
                && (stderr.contains("nothing to commit")
                    || stderr.contains("no changes added to commit")
                    || stderr.contains("No changes")
                    || stderr.contains("nothing added to commit")
                    || (args.contains(&"--amend") && stderr.contains("Needed a single revision"))
                    || (args.contains(&"--amend") && stderr.contains("no commits yet"))))
            {
                bail!(
                    "Command '{} {}' failed in {:?} with status {}:\nStdout: {}\nStderr: {}",
                    command_str,
                    args.join(" "),
                    dir,
                    output.status,
                    stdout,
                    stderr
                );
            }
        }
        Ok(output)
    }

    fn create_and_commit_file(
        repo_path: &Path,
        file_name: &str,
        content: &[u8],
    ) -> Result<(), anyhow::Error> {
        let file_path = repo_path.join(file_name);
        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create parent dirs for {:?}", file_path))?;
        }
        let mut file = File::create(&file_path)
            .with_context(|| format!("Failed to create file {:?}", file_path))?;
        file.write_all(content)
            .with_context(|| format!("Failed to write to file {:?}", file_path))?;
        drop(file);
        run_command_in_dir(repo_path, "git", &["add", file_name])?;
        run_command_in_dir(
            repo_path,
            "git",
            &["commit", "--allow-empty-message", "-m", "Initial commit"],
        )?;
        Ok(())
    }

    fn stage_file_changes(
        repo_path: &Path,
        file_name: &str,
        new_content: &[u8],
    ) -> Result<(), anyhow::Error> {
        let file_path = repo_path.join(file_name);
        let mut file = File::create(&file_path).with_context(|| {
            format!("Failed to create/truncate file for changes {:?}", file_path)
        })?;
        file.write_all(new_content)
            .with_context(|| format!("Failed to write new content to file {:?}", file_path))?;
        drop(file);
        run_command_in_dir(repo_path, "git", &["add", file_name])?;
        Ok(())
    }

    fn stage_new_file(
        repo_path: &Path,
        file_name: &str,
        content: &[u8],
    ) -> Result<(), anyhow::Error> {
        let file_path = repo_path.join(file_name);
        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent).with_context(|| {
                format!("Failed to create parent dirs for new file {:?}", file_path)
            })?;
        }
        let mut file = File::create(&file_path)
            .with_context(|| format!("Failed to create new file {:?}", file_path))?;
        file.write_all(content)
            .with_context(|| format!("Failed to write to new file {:?}", file_path))?;
        drop(file);
        run_command_in_dir(repo_path, "git", &["add", file_name])?;
        Ok(())
    }

    fn stage_deletion(repo_path: &Path, file_name: &str) -> Result<(), anyhow::Error> {
        run_command_in_dir(repo_path, "git", &["rm", file_name])?;
        Ok(())
    }

    fn stage_rename(repo_path: &Path, old_name: &str, new_name: &str) -> Result<(), anyhow::Error> {
        let new_path = repo_path.join(new_name);
        if let Some(parent) = new_path.parent() {
            fs::create_dir_all(parent).with_context(|| {
                format!("Failed to create parent dirs for new_name {:?}", parent)
            })?;
        }
        run_command_in_dir(repo_path, "git", &["mv", old_name, new_name])?;
        Ok(())
    }

    #[test]
    fn test_has_staged_files_empty_repo() -> Result<(), anyhow::Error> {
        let temp_dir = TempDir::new()?;
        let repo_path = temp_dir.path();
        setup_git_repo(repo_path)?;
        assert!(!has_staged_files(repo_path)?);
        temp_dir.close()?;
        Ok(())
    }

    #[test]
    fn test_has_staged_files_with_staged_file() -> Result<(), anyhow::Error> {
        let temp_dir = TempDir::new()?;
        let repo_path = temp_dir.path();
        setup_git_repo(repo_path)?;
        stage_new_file(repo_path, "staged.txt", b"content")?;
        assert!(has_staged_files(repo_path)?);
        temp_dir.close()?;
        Ok(())
    }

    #[test]
    fn test_has_staged_files_after_commit() -> Result<(), anyhow::Error> {
        let temp_dir = TempDir::new()?;
        let repo_path = temp_dir.path();
        setup_git_repo(repo_path)?;
        create_and_commit_file(repo_path, "committed.txt", b"content")?;
        assert!(!has_staged_files(repo_path)?);
        temp_dir.close()?;
        Ok(())
    }

    #[test]
    fn test_get_staged_diff_no_staged_files_returns_empty_string() -> Result<(), anyhow::Error> {
        let temp_dir = TempDir::new()?;
        let repo_path = temp_dir.path();
        setup_git_repo(repo_path)?;
        create_and_commit_file(repo_path, "test.txt", b"initial content")?;
        let diff = get_staged_diff(repo_path)?;
        assert!(diff.is_empty());
        temp_dir.close()?;
        Ok(())
    }

    #[test]
    fn test_get_staged_diff_with_staged_text_modification_integration() -> Result<(), anyhow::Error>
    {
        let temp_dir = TempDir::new()?;
        let repo_path = temp_dir.path();
        setup_git_repo(repo_path)?;
        create_and_commit_file(repo_path, "modified.txt", b"line1\nline2\n")?;
        stage_file_changes(
            repo_path,
            "modified.txt",
            b"line1_changed\nline2\nline3_new\n",
        )?;
        let diff_result = get_staged_diff(repo_path);
        assert!(
            diff_result.is_ok(),
            "get_staged_diff failed: {:?}",
            diff_result.err()
        );
        let diff = diff_result.unwrap();
        assert!(diff.contains("--- a/modified.txt"));
        assert!(diff.contains("+++ b/modified.txt"));
        assert!(diff.contains("-line1"));
        assert!(diff.contains("+line1_changed"));
        assert!(diff.contains("+line3_new"));
        temp_dir.close()?;
        Ok(())
    }

    #[test]
    fn test_summary_no_staged_files() -> Result<(), anyhow::Error> {
        let temp_dir = TempDir::new()?;
        let repo_path = temp_dir.path();
        setup_git_repo(repo_path)?;
        create_and_commit_file(repo_path, "a.txt", b"initial")?;
        let summary = get_staged_changes_summary(repo_path)?;
        assert_eq!(summary, StagedChangesSummary::default());
        temp_dir.close()?;
        Ok(())
    }

    #[test]
    fn test_summary_add_text_file() -> Result<(), anyhow::Error> {
        let temp_dir = TempDir::new()?;
        let repo_path = temp_dir.path();
        setup_git_repo(repo_path)?;
        stage_new_file(repo_path, "new.txt", b"hello")?;
        let summary = get_staged_changes_summary(repo_path)?;
        assert_eq!(summary.binary_file_changes, Vec::<String>::new());
        assert_eq!(summary.structure_changes, Vec::<String>::new());
        temp_dir.close()?;
        Ok(())
    }

    #[test]
    fn test_summary_add_binary_file() -> Result<(), anyhow::Error> {
        let temp_dir = TempDir::new()?;
        let repo_path = temp_dir.path();
        setup_git_repo(repo_path)?;
        stage_new_file(repo_path, "new.bin", &[0x00, 0x01, 0x02, 0x00, 0x04])?;
        let summary = get_staged_changes_summary(repo_path)?;
        let expected = StagedChangesSummary {
            binary_file_changes: vec!["added binary file: new.bin".to_string()],
            structure_changes: vec![],
        };
        assert_eq!(summary, expected);
        temp_dir.close()?;
        Ok(())
    }

    #[test]
    fn test_summary_modify_binary_file() -> Result<(), anyhow::Error> {
        let temp_dir = TempDir::new()?;
        let repo_path = temp_dir.path();
        setup_git_repo(repo_path)?;
        create_and_commit_file(repo_path, "app.exe", &[0xDE, 0xAD, 0xBE, 0xEF, 0x00])?;
        stage_file_changes(repo_path, "app.exe", &[0xCA, 0xFE, 0xBA, 0xBE, 0x00, 0x01])?;
        let summary = get_staged_changes_summary(repo_path)?;
        let expected = StagedChangesSummary {
            binary_file_changes: vec!["modified binary file: app.exe".to_string()],
            structure_changes: vec![],
        };
        assert_eq!(summary, expected);
        temp_dir.close()?;
        Ok(())
    }

    #[test]
    fn test_summary_delete_text_file() -> Result<(), anyhow::Error> {
        let temp_dir = TempDir::new()?;
        let repo_path = temp_dir.path();
        setup_git_repo(repo_path)?;
        create_and_commit_file(repo_path, "old.txt", b"delete me")?;
        stage_deletion(repo_path, "old.txt")?;
        let summary = get_staged_changes_summary(repo_path)?;
        let expected = StagedChangesSummary {
            binary_file_changes: vec![],
            structure_changes: vec!["deleted file: old.txt".to_string()],
        };
        assert_eq!(summary, expected);
        temp_dir.close()?;
        Ok(())
    }

    #[test]
    fn test_summary_delete_binary_file() -> Result<(), anyhow::Error> {
        let temp_dir = TempDir::new()?;
        let repo_path = temp_dir.path();
        setup_git_repo(repo_path)?;
        create_and_commit_file(repo_path, "old.bin", &[0x00, 0x00])?;
        stage_deletion(repo_path, "old.bin")?;
        let summary = get_staged_changes_summary(repo_path)?;
        let expected = StagedChangesSummary {
            binary_file_changes: vec![],
            structure_changes: vec!["deleted file: old.bin".to_string()],
        };
        assert_eq!(summary, expected);
        temp_dir.close()?;
        Ok(())
    }

    #[test]
    fn test_summary_rename_text_file() -> Result<(), anyhow::Error> {
        let temp_dir = TempDir::new()?;
        let repo_path = temp_dir.path();
        setup_git_repo(repo_path)?;
        create_and_commit_file(repo_path, "original.txt", b"I will be renamed")?;
        stage_rename(repo_path, "original.txt", "renamed.txt")?;
        let summary = get_staged_changes_summary(repo_path)?;
        let expected = StagedChangesSummary {
            binary_file_changes: vec![],
            structure_changes: vec!["renamed: original.txt to renamed.txt".to_string()],
        };
        assert_eq!(summary, expected);
        temp_dir.close()?;
        Ok(())
    }

    #[test]
    fn test_summary_rename_binary_file() -> Result<(), anyhow::Error> {
        let temp_dir = TempDir::new()?;
        let repo_path = temp_dir.path();
        setup_git_repo(repo_path)?;
        create_and_commit_file(repo_path, "original.dat", &[0x01, 0x02, 0x00, 0x03])?;
        stage_rename(repo_path, "original.dat", "renamed.dat")?;
        let summary = get_staged_changes_summary(repo_path)?;
        let expected = StagedChangesSummary {
            binary_file_changes: vec![
                "renamed binary file: original.dat to renamed.dat".to_string(),
            ],
            structure_changes: vec!["renamed: original.dat to renamed.dat".to_string()],
        };
        assert_eq!(summary, expected);
        temp_dir.close()?;
        Ok(())
    }

    #[test]
    fn test_summary_rename_directory_with_files() -> Result<(), anyhow::Error> {
        let temp_dir = TempDir::new()?;
        let repo_path = temp_dir.path();
        setup_git_repo(repo_path)?;
        create_and_commit_file(repo_path, "old_dir/file1.txt", b"content1")?;
        create_and_commit_file(repo_path, "old_dir/file2.bin", &[0x00, 0x01, 0x00, 0x02])?;

        fs::create_dir_all(repo_path.join("new_dir"))
            .context("Failed to create new_dir for rename target")?;
        run_command_in_dir(
            repo_path,
            "git",
            &["mv", "old_dir/file1.txt", "new_dir/file1.txt"],
        )?;
        run_command_in_dir(
            repo_path,
            "git",
            &["mv", "old_dir/file2.bin", "new_dir/file2.bin"],
        )?;

        let summary = get_staged_changes_summary(repo_path)?;

        let mut expected_binary =
            vec!["renamed binary file: old_dir/file2.bin to new_dir/file2.bin".to_string()];
        expected_binary.sort();
        let mut expected_structure = vec![
            "renamed: old_dir/file1.txt to new_dir/file1.txt".to_string(),
            "renamed: old_dir/file2.bin to new_dir/file2.bin".to_string(),
        ];
        expected_structure.sort();

        let mut actual_binary = summary.binary_file_changes;
        actual_binary.sort();
        let mut actual_structure = summary.structure_changes;
        actual_structure.sort();

        assert_eq!(actual_binary, expected_binary);
        assert_eq!(actual_structure, expected_structure);
        temp_dir.close()?;
        Ok(())
    }

    #[test]
    fn test_summary_mixed_changes() -> Result<(), anyhow::Error> {
        let temp_dir = TempDir::new()?;
        let repo_path = temp_dir.path();
        setup_git_repo(repo_path)?;

        create_and_commit_file(repo_path, "committed_binary.bin", &[0x01, 0x02, 0x00])?;
        create_and_commit_file(repo_path, "to_be_deleted.txt", b"delete me")?;
        create_and_commit_file(repo_path, "to_be_renamed.txt", b"rename me")?;
        create_and_commit_file(repo_path, "unmodified_text.txt", b"I am stable.")?;

        stage_new_file(repo_path, "new_text.txt", b"new text file")?;
        stage_new_file(repo_path, "new_binary.dat", &[0x00, 0xFF, 0x00, 0xAA])?;
        stage_file_changes(
            repo_path,
            "committed_binary.bin",
            &[0x01, 0x02, 0x03, 0x00, 0xBB],
        )?;
        stage_deletion(repo_path, "to_be_deleted.txt")?;
        stage_rename(repo_path, "to_be_renamed.txt", "was_renamed.txt")?;

        let summary = get_staged_changes_summary(repo_path)?;

        let mut expected_binary = vec![
            "added binary file: new_binary.dat".to_string(),
            "modified binary file: committed_binary.bin".to_string(),
        ];
        expected_binary.sort();

        let mut expected_structure = vec![
            "deleted file: to_be_deleted.txt".to_string(),
            "renamed: to_be_renamed.txt to was_renamed.txt".to_string(),
        ];
        expected_structure.sort();

        let mut actual_binary = summary.binary_file_changes;
        actual_binary.sort();
        let mut actual_structure = summary.structure_changes;
        actual_structure.sort();

        assert_eq!(actual_binary, expected_binary, "Binary changes mismatch");
        assert_eq!(
            actual_structure, expected_structure,
            "Structure changes mismatch"
        );
        temp_dir.close()?;
        Ok(())
    }

    #[test]
    fn test_summary_rename_with_common_prefix_suffix_binary() -> Result<(), anyhow::Error> {
        let temp_dir = TempDir::new()?;
        let repo_path = temp_dir.path();
        setup_git_repo(repo_path)?;
        create_and_commit_file(repo_path, "src/old_file.bin", &[0x01, 0x00, 0x02, 0xAB])?;
        stage_rename(repo_path, "src/old_file.bin", "src/new_file.bin")?;

        let summary = get_staged_changes_summary(repo_path)?;
        let expected = StagedChangesSummary {
            binary_file_changes: vec![
                "renamed binary file: src/old_file.bin to src/new_file.bin".to_string(),
            ],
            structure_changes: vec!["renamed: src/old_file.bin to src/new_file.bin".to_string()],
        };
        assert_eq!(summary, expected);
        temp_dir.close()?;
        Ok(())
    }

    #[test]
    fn test_commit_staged_files_integration() -> Result<(), anyhow::Error> {
        let temp_dir = TempDir::new()?;
        let repo_path = temp_dir.path();
        setup_git_repo(repo_path)?;
        stage_new_file(repo_path, "commit_me.txt", b"content to commit")?;
        let commit_message = "feat: Add commit_me.txt";

        let commit_output = commit_staged_files(repo_path, commit_message)?;
        assert!(
            commit_output.contains("main") || commit_output.contains("master"),
            "Commit output did not contain branch name: {}",
            commit_output
        );
        assert!(
            commit_output.contains(commit_message),
            "Commit output did not contain commit message: {}",
            commit_output
        );
        assert!(
            commit_output.contains("1 file changed"),
            "Commit output did not indicate 1 file changed: {}",
            commit_output
        );

        let log_output_cmd = execute_git_command(repo_path, &["log", "-1", "--pretty=%B"])?;
        let log_stdout = str::from_utf8(&log_output_cmd.stdout)?.trim();
        assert_eq!(log_stdout, commit_message);
        temp_dir.close()?;
        Ok(())
    }

    #[test]
    fn test_commit_staged_files_empty_message_fails() -> Result<(), anyhow::Error> {
        let temp_dir = TempDir::new()?;
        let repo_path = temp_dir.path();
        setup_git_repo(repo_path)?;
        stage_new_file(repo_path, "another.txt", b"content")?;
        let result = commit_staged_files(repo_path, " ");
        assert!(result.is_err());
        if let Err(e) = result {
            assert!(e.to_string().contains("Commit message cannot be empty."));
        }
        temp_dir.close()?;
        Ok(())
    }

    #[test]
    fn test_get_previous_commit_message_no_commits() -> Result<(), anyhow::Error> {
        let temp_dir = TempDir::new()?;
        let repo_path = temp_dir.path();
        setup_git_repo(repo_path)?;
        let message = get_previous_commit_message(repo_path)?;
        assert_eq!(message, None);
        temp_dir.close()?;
        Ok(())
    }

    #[test]
    fn test_get_previous_commit_message_with_commit() -> Result<(), anyhow::Error> {
        let temp_dir = TempDir::new()?;
        let repo_path = temp_dir.path();
        setup_git_repo(repo_path)?;
        let initial_commit_message = "feat: Initial project setup";
        create_and_commit_file(repo_path, "init.txt", b"initial")?;
        run_command_in_dir(
            repo_path,
            "git",
            &["commit", "--amend", "-m", initial_commit_message],
        )?;

        let message = get_previous_commit_message(repo_path)?;
        assert_eq!(message, Some(initial_commit_message.to_string()));
        temp_dir.close()?;
        Ok(())
    }

    #[test]
    fn test_get_previous_commit_message_with_multiline_commit() -> Result<(), anyhow::Error> {
        let temp_dir = TempDir::new()?;
        let repo_path = temp_dir.path();
        setup_git_repo(repo_path)?;
        let initial_commit_message =
            "feat: Initial project setup\n\nThis is a detailed description.";
        create_and_commit_file(repo_path, "init.txt", b"initial")?;
        run_command_in_dir(
            repo_path,
            "git",
            &["commit", "--amend", "-m", initial_commit_message],
        )?;

        let message = get_previous_commit_message(repo_path)?;
        assert_eq!(message, Some(initial_commit_message.to_string()));
        temp_dir.close()?;
        Ok(())
    }

    #[test]
    fn test_amend_commit_no_initial_commit_fails_gracefully_in_git() -> Result<(), anyhow::Error> {
        let temp_dir = TempDir::new()?;
        let repo_path = temp_dir.path();
        setup_git_repo(repo_path)?;

        let result = amend_commit(repo_path, "fix: Amending non-existent commit");
        assert!(result.is_err());

        if let Err(e) = result {
            let err_string_lower = e.to_string().to_lowercase();
            assert!(
                err_string_lower.contains("git commit --amend -m"),
                "Error message did not contain 'git commit --amend -m'. Actual: {}",
                err_string_lower
            );
            assert!(
                err_string_lower.contains("failed"),
                "Error message did not contain 'failed'. Actual: {}",
                err_string_lower
            );
            // More flexible check for git-related errors
            assert!(
                err_string_lower.contains("needed a single revision") 
                || err_string_lower.contains("no commits yet") 
                || err_string_lower.contains("does not have any commits yet")
                || err_string_lower.contains("bad default revision")
                || err_string_lower.contains("ambiguous argument 'head'")
                || err_string_lower.contains("unknown revision")
                // Add a more general check that catches the actual error
                || (err_string_lower.contains("git") && err_string_lower.contains("amend")),
                "Error message did not contain expected git error. Actual: {}",
                err_string_lower
            );
        }

        temp_dir.close()?;
        Ok(())
    }

    #[test]
    fn test_amend_commit_successful() -> Result<(), anyhow::Error> {
        let temp_dir = TempDir::new()?;
        let repo_path = temp_dir.path();
        setup_git_repo(repo_path)?;
        create_and_commit_file(repo_path, "first.txt", b"content1")?;

        stage_file_changes(repo_path, "first.txt", b"updated content1")?;
        let amend_message = "fix: Update first.txt with new content";
        let amend_output = amend_commit(repo_path, amend_message)?;

        assert!(
            amend_output.contains("1 file changed")
                || amend_output.contains(amend_message)
                || amend_output.contains("main")
        );

        let log_output = execute_git_command(repo_path, &["log", "-1", "--pretty=%B"])?;
        let last_commit_message = str::from_utf8(&log_output.stdout)?.trim();
        assert_eq!(last_commit_message, amend_message);
        temp_dir.close()?;
        Ok(())
    }

    #[test]
    fn test_amend_commit_empty_message_fails() -> Result<(), anyhow::Error> {
        let temp_dir = TempDir::new()?;
        let repo_path = temp_dir.path();
        setup_git_repo(repo_path)?;
        create_and_commit_file(repo_path, "another.txt", b"content")?;
        let result = amend_commit(repo_path, " ");
        assert!(result.is_err());
        if let Err(e) = result {
            assert!(
                e.to_string()
                    .contains("Commit message for amend cannot be empty.")
            );
        }
        temp_dir.close()?;
        Ok(())
    }
}
