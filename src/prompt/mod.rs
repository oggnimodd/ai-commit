use crate::git::StagedChangesSummary;

const MIN_COMMIT_DESCRIPTION_CHARS: usize = 10;
const MAX_COMMIT_DESCRIPTION_CHARS: usize = 72;

struct CommitType<'a> {
    name: &'a str,
    description: &'a str,
}

const COMMIT_TYPES: &[CommitType] = &[
    CommitType {
        name: "feat",
        description: "A new feature (e.g., adding a new endpoint, a new UI component).",
    },
    CommitType {
        name: "fix",
        description: "A bug fix (e.g., correcting a calculation error, addressing a crash).",
    },
    CommitType {
        name: "docs",
        description: "Documentation only changes (e.g., updating README, API docs).",
    },
    CommitType {
        name: "style",
        description: "Changes that do not affect the meaning of the code (white-space, formatting, missing semi-colons, etc).",
    },
    CommitType {
        name: "refactor",
        description: "A code change that neither fixes a bug nor adds a feature (e.g., renaming a variable, improving code structure).",
    },
    CommitType {
        name: "test",
        description: "Adding missing tests or correcting existing tests.",
    },
    CommitType {
        name: "chore",
        description: "Changes to the build process or auxiliary tools and libraries such as dependency updates, scripts.",
    },
    CommitType {
        name: "build",
        description: "Changes that affect the build system or external dependencies (e.g., Gulp, Broccoli, NPM).",
    },
    CommitType {
        name: "ci",
        description: "Changes to CI configuration files and scripts (e.g., GitHub Actions, Travis).",
    },
    CommitType {
        name: "perf",
        description: "A code change that improves performance.",
    },
    CommitType {
        name: "revert",
        description: "Reverts a previous commit.",
    },
    CommitType {
        name: "readme",
        description: "Specifically for changes to the README file.",
    },
];

fn format_commit_types_for_prompt() -> String {
    let mut s = String::new();
    for ct in COMMIT_TYPES {
        s.push_str(&format!("- {}: {}\n", ct.name, ct.description));
    }
    s
}

pub fn build_prompt(
    diff_content: &str,
    changes_summary: &StagedChangesSummary,
    num_suggestions: u32,
    previous_message: Option<&str>,
) -> String {
    let commit_types_formatted = format_commit_types_for_prompt();

    let binary_changes_summary_str = if changes_summary.binary_file_changes.is_empty() {
        "No binary file changes detected.".to_string()
    } else {
        changes_summary.binary_file_changes.join("\n")
    };

    let folder_structure_changes_summary_str = if changes_summary.structure_changes.is_empty() {
        "No folder structure changes detected.".to_string()
    } else {
        changes_summary.structure_changes.join("\n")
    };

    let mut prompt_parts: Vec<String> = Vec::new();

    prompt_parts.push(format!(
        "Analyze the following code changes and repository structure modifications. Generate {} Git commit message(s).",
        num_suggestions
    ));
    prompt_parts.push("Each message MUST follow this format: <type>: <description>".to_string());

    prompt_parts.push(format!(
        "Available <type>s are:\n{}",
        commit_types_formatted.trim_end()
    ));

    prompt_parts.push(format!(
        "The AI should choose the <type> that best describes the overall changes.\n\
        The <description> should be concise, start with a verb in the imperative mood if possible, and be between {} and {} characters.",
        MIN_COMMIT_DESCRIPTION_CHARS, MAX_COMMIT_DESCRIPTION_CHARS
    ));
    prompt_parts
        .push("Do not include any other explanatory text, just the commit message(s).".to_string());

    if let Some(prev_msg) = previous_message {
        prompt_parts.push(format!(
            "The previous commit message was: '{}'. Please generate a new, improved message based on the changes, considering why the previous one might have been suboptimal.",
            prev_msg
        ));
    }

    prompt_parts.push("Diff:\n\n---".to_string());
    prompt_parts.push(if diff_content.trim().is_empty() {
        "No textual diff.".to_string()
    } else {
        diff_content.to_string()
    });
    prompt_parts.push("---".to_string());

    prompt_parts.push("Binary file changes:".to_string());
    prompt_parts.push(binary_changes_summary_str);
    prompt_parts.push("---".to_string());

    prompt_parts.push("Folder structure changes:".to_string());
    prompt_parts.push(folder_structure_changes_summary_str);
    prompt_parts.push("---".to_string());

    prompt_parts.join("\n\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::StagedChangesSummary;

    #[test]
    fn test_build_prompt_basic() {
        let diff = "diff --git a/file.txt b/file.txt\nindex 123..456 100644\n--- a/file.txt\n+++ b/file.txt\n@@ -1 +1 @@\n-old\n+new";
        let summary = StagedChangesSummary {
            binary_file_changes: vec!["added binary file: image.png".to_string()],
            structure_changes: vec!["renamed: old_dir/file.txt to new_dir/file.txt".to_string()],
        };
        let prompt = build_prompt(diff, &summary, 1, None);

        assert!(prompt.contains("Generate 1 Git commit message(s)."));
        assert!(prompt.contains("- feat: A new feature"));
        assert!(prompt.contains(&format!(
            "between {} and {} characters.",
            MIN_COMMIT_DESCRIPTION_CHARS, MAX_COMMIT_DESCRIPTION_CHARS
        )));
        assert!(prompt.contains(diff));
        assert!(prompt.contains("Binary file changes:\n\nadded binary file: image.png\n\n---"));
        assert!(prompt.contains(
            "Folder structure changes:\n\nrenamed: old_dir/file.txt to new_dir/file.txt\n\n---"
        ));
        assert!(!prompt.contains("The previous commit message was:"));
    }

    #[test]
    fn test_build_prompt_with_amend() {
        let diff = "diff --git a/another.txt b/another.txt\n--- a/another.txt\n+++ b/another.txt\n@@ -1 +1 @@\n-old content\n+new content";
        let summary = StagedChangesSummary::default();
        let prev_msg = "fix: did a thing wrong";
        let prompt = build_prompt(diff, &summary, 3, Some(prev_msg));

        assert!(prompt.contains("Generate 3 Git commit message(s)."));
        assert!(prompt.contains(&format!("The previous commit message was: '{}'.", prev_msg)));
        assert!(prompt.contains(diff));
        assert!(prompt.contains("Binary file changes:\n\nNo binary file changes detected.\n\n---"));
        assert!(
            prompt.contains(
                "Folder structure changes:\n\nNo folder structure changes detected.\n\n---"
            )
        );
    }

    #[test]
    fn test_build_prompt_no_textual_diff() {
        let diff = "";
        let summary = StagedChangesSummary {
            binary_file_changes: vec!["added binary file: data.zip".to_string()],
            structure_changes: vec![],
        };
        let prompt = build_prompt(diff, &summary, 1, None);
        assert!(prompt.contains("Diff:\n\n---\n\nNo textual diff.\n\n---"));
        assert!(prompt.contains("Binary file changes:\n\nadded binary file: data.zip\n\n---"));
    }

    #[test]
    fn test_build_prompt_empty_summary() {
        let diff = "diff --git a/file.txt b/file.txt\n--- a/file.txt\n+++ b/file.txt\n@@ -1 +1 @@\n-old\n+new";
        let summary = StagedChangesSummary::default();
        let prompt = build_prompt(diff, &summary, 1, None);

        assert!(prompt.contains(diff));
        assert!(prompt.contains("Binary file changes:\n\nNo binary file changes detected.\n\n---"));
        assert!(
            prompt.contains(
                "Folder structure changes:\n\nNo folder structure changes detected.\n\n---"
            )
        );
    }

    #[test]
    fn test_format_commit_types_for_prompt() {
        let formatted_types = format_commit_types_for_prompt();
        assert!(formatted_types.contains("- feat: A new feature"));
        assert!(formatted_types.contains("- fix: A bug fix"));
        assert!(formatted_types.ends_with(".\n"));
        assert_eq!(formatted_types.lines().count(), COMMIT_TYPES.len());
    }

    #[test]
    fn test_prompt_structure_newlines() {
        let diff = "text diff";
        let summary = StagedChangesSummary {
            binary_file_changes: vec!["binary change".to_string()],
            structure_changes: vec!["structure change".to_string()],
        };
        let prompt = build_prompt(diff, &summary, 1, None);

        let expected_structure = "Analyze the following code changes and repository structure modifications. Generate 1 Git commit message(s).\n\n\
Each message MUST follow this format: <type>: <description>\n\n\
Available <type>s are:\n\
- feat: A new feature (e.g., adding a new endpoint, a new UI component).\n\
- fix: A bug fix (e.g., correcting a calculation error, addressing a crash).\n\
- docs: Documentation only changes (e.g., updating README, API docs).\n\
- style: Changes that do not affect the meaning of the code (white-space, formatting, missing semi-colons, etc).\n\
- refactor: A code change that neither fixes a bug nor adds a feature (e.g., renaming a variable, improving code structure).\n\
- test: Adding missing tests or correcting existing tests.\n\
- chore: Changes to the build process or auxiliary tools and libraries such as dependency updates, scripts.\n\
- build: Changes that affect the build system or external dependencies (e.g., Gulp, Broccoli, NPM).\n\
- ci: Changes to CI configuration files and scripts (e.g., GitHub Actions, Travis).\n\
- perf: A code change that improves performance.\n\
- revert: Reverts a previous commit.\n\
- readme: Specifically for changes to the README file.\n\n\
The AI should choose the <type> that best describes the overall changes.\n\
The <description> should be concise, start with a verb in the imperative mood if possible, and be between 10 and 72 characters.\n\n\
Do not include any other explanatory text, just the commit message(s).\n\n\
Diff:\n\n\
---\n\n\
text diff\n\n\
---\n\n\
Binary file changes:\n\n\
binary change\n\n\
---\n\n\
Folder structure changes:\n\n\
structure change\n\n\
---";
        assert_eq!(prompt, expected_structure);
    }
}
