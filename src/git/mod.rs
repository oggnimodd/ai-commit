use anyhow::{Context, Result, bail};
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
        let stdout = str::from_utf8(&output.stdout)
            .unwrap_or("[non-utf8 stdout]")
            .trim()
            .to_string();
        let stderr = str::from_utf8(&output.stderr)
            .unwrap_or("[non-utf8 stderr]")
            .trim()
            .to_string();
        bail!(
            "Git command '{}' failed in {:?} with status {}:\nStdout: {}\nStderr: {}",
            command_str,
            repo_path,
            output.status,
            stdout,
            stderr
        );
    }
    Ok(output)
}

pub fn get_staged_diff(repo_path: &Path) -> Result<String, anyhow::Error> {
    let status_output = execute_git_command(
        repo_path,
        &["status", "--porcelain", "--untracked-files=no"],
    )
    .context("Failed to get git status")?;

    let status_stdout = str::from_utf8(&status_output.stdout)
        .context("Failed to read git status output as UTF-8")?
        .trim();

    if status_stdout.is_empty() {
        bail!("No staged files found. Stage files with 'git add' before running ai-commit.");
    }

    let diff_output = execute_git_command(repo_path, &["diff", "--staged"])
        .context("Failed to get staged git diff")?;

    let diff_stdout = str::from_utf8(&diff_output.stdout)
        .context("Failed to read git diff output as UTF-8")?
        .to_string();

    Ok(diff_stdout)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self, File};
    use std::io::Write;
    use std::process::Command;
    use tempfile::TempDir;

    fn setup_git_repo(temp_dir_path: &Path) -> Result<(), anyhow::Error> {
        run_command_in_dir(temp_dir_path, "git", &["init"])?;
        run_command_in_dir(temp_dir_path, "git", &["config", "user.name", "Test User"])?;
        run_command_in_dir(
            temp_dir_path,
            "git",
            &["config", "user.email", "test@example.com"],
        )?;
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
        Ok(output)
    }

    fn create_and_commit_file(
        repo_path: &Path,
        file_name: &str,
        content: &str,
    ) -> Result<(), anyhow::Error> {
        let file_path = repo_path.join(file_name);
        fs::write(&file_path, content)
            .with_context(|| format!("Failed to write to file {:?}", file_path))?;
        run_command_in_dir(repo_path, "git", &["add", file_name])?;
        run_command_in_dir(
            repo_path,
            "git",
            &["commit", "-m", &format!("add {}", file_name)],
        )?;
        Ok(())
    }

    fn stage_file_changes(
        repo_path: &Path,
        file_name: &str,
        new_content: &str,
    ) -> Result<(), anyhow::Error> {
        let file_path = repo_path.join(file_name);
        fs::write(&file_path, new_content)
            .with_context(|| format!("Failed to write new content to file {:?}", file_path))?;
        run_command_in_dir(repo_path, "git", &["add", file_name])?;
        Ok(())
    }

    fn stage_new_file(
        repo_path: &Path,
        file_name: &str,
        content: &str,
    ) -> Result<(), anyhow::Error> {
        let file_path = repo_path.join(file_name);
        fs::write(&file_path, content)
            .with_context(|| format!("Failed to write to new file {:?}", file_path))?;
        run_command_in_dir(repo_path, "git", &["add", file_name])?;
        Ok(())
    }

    #[test]
    fn test_get_staged_diff_no_staged_files() -> Result<(), anyhow::Error> {
        let temp_dir = TempDir::new()?;
        let repo_path = temp_dir.path();
        setup_git_repo(repo_path)?;
        create_and_commit_file(repo_path, "test.txt", "initial content")?;

        let result = get_staged_diff(repo_path);
        assert!(result.is_err());
        if let Err(e) = result {
            assert!(
                e.to_string().contains("No staged files found."),
                "Error message was: {}",
                e
            );
        }
        temp_dir.close()?;
        Ok(())
    }

    #[test]
    fn test_get_staged_diff_with_staged_text_modification() -> Result<(), anyhow::Error> {
        let temp_dir = TempDir::new()?;
        let repo_path = temp_dir.path();
        setup_git_repo(repo_path)?;
        create_and_commit_file(repo_path, "modified.txt", "line1\nline2\n")?;
        stage_file_changes(
            repo_path,
            "modified.txt",
            "line1_changed\nline2\nline3_new\n",
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
    fn test_get_staged_diff_with_new_staged_file() -> Result<(), anyhow::Error> {
        let temp_dir = TempDir::new()?;
        let repo_path = temp_dir.path();
        setup_git_repo(repo_path)?;
        stage_new_file(
            repo_path,
            "new_file.txt",
            "Hello World\nThis is a new file.\n",
        )?;

        let diff_result = get_staged_diff(repo_path);
        assert!(
            diff_result.is_ok(),
            "get_staged_diff failed: {:?}",
            diff_result.err()
        );
        let diff = diff_result.unwrap();

        assert!(
            diff.contains("diff --git a/new_file.txt b/new_file.txt"),
            "Diff content was:\n{}",
            diff
        );
        assert!(diff.contains("new file mode 100644"));
        assert!(diff.contains("--- /dev/null"));
        assert!(diff.contains("+++ b/new_file.txt"));
        assert!(diff.contains("+Hello World"));
        assert!(diff.contains("+This is a new file."));
        temp_dir.close()?;
        Ok(())
    }

    #[test]
    fn test_get_staged_diff_empty_repository_new_staged_file() -> Result<(), anyhow::Error> {
        let temp_dir = TempDir::new()?;
        let repo_path = temp_dir.path();
        setup_git_repo(repo_path)?;

        stage_new_file(repo_path, "first_file.txt", "Content of first file")?;

        let diff_result = get_staged_diff(repo_path);
        assert!(
            diff_result.is_ok(),
            "get_staged_diff failed: {:?}",
            diff_result.err()
        );
        let diff = diff_result.unwrap();

        assert!(diff.contains("diff --git a/first_file.txt b/first_file.txt"));
        assert!(diff.contains("new file mode 100644"));
        assert!(diff.contains("--- /dev/null"));
        assert!(diff.contains("+++ b/first_file.txt"));
        assert!(diff.contains("+Content of first file"));
        temp_dir.close()?;
        Ok(())
    }

    #[test]
    fn test_get_staged_diff_with_staged_binary_file_modification() -> Result<(), anyhow::Error> {
        let temp_dir = TempDir::new()?;
        let repo_path = temp_dir.path();
        setup_git_repo(repo_path)?;

        let file_name = "binary_file.bin";
        let initial_content: [u8; 4] = [0x00, 0x01, 0x02, 0x03];
        let modified_content: [u8; 5] = [0x00, 0x01, 0xFF, 0x02, 0x03];

        let file_path = repo_path.join(file_name);

        let mut f_initial = File::create(&file_path)?;
        f_initial.write_all(&initial_content)?;
        drop(f_initial);
        run_command_in_dir(repo_path, "git", &["add", file_name])?;
        run_command_in_dir(repo_path, "git", &["commit", "-m", "add binary file"])?;

        let mut f_modified = File::create(&file_path)?;
        f_modified.write_all(&modified_content)?;
        drop(f_modified);
        run_command_in_dir(repo_path, "git", &["add", file_name])?;

        let diff_result = get_staged_diff(repo_path);
        assert!(
            diff_result.is_ok(),
            "get_staged_diff failed: {:?}",
            diff_result.err()
        );
        let diff = diff_result.unwrap();

        assert!(diff.contains(&format!("diff --git a/{} b/{}", file_name, file_name)));
        assert!(diff.contains("index "));
        assert!(
            diff.contains(&format!(
                "Binary files a/{} and b/{} differ",
                file_name, file_name
            )) || diff.contains("GIT binary patch"),
            "Binary diff content missing expected markers. Diff:\n{}",
            diff
        );
        temp_dir.close()?;
        Ok(())
    }
}
