use crate::git::StagedChangesSummary;

const MIN_COMMIT_DESCRIPTION_CHARS: usize = 10;
const MAX_COMMIT_DESCRIPTION_CHARS: usize = 72;

#[derive(Clone, Copy)]
struct CommitType<'a> {
    name: &'a str,
    description: &'a str,
    example: &'a str,
    priority: u8,
}

const COMMIT_TYPES: &[CommitType] = &[
    CommitType {
        name: "feat",
        description: "A new feature or significant functionality addition (e.g., adding new endpoints, UI components, initial project setup).",
        example: "feat: Implement user authentication via OAuth",
        priority: 9,
    },
    CommitType {
        name: "fix",
        description: "A bug fix (e.g., correcting calculation errors, addressing crashes, security vulnerabilities).",
        example: "fix: Correct off-by-one error in pagination",
        priority: 8,
    },
    CommitType {
        name: "perf",
        description: "A code change that improves performance without adding features or fixing bugs.",
        example: "perf: Optimize image loading by using WebP format",
        priority: 7,
    },
    CommitType {
        name: "refactor",
        description: "A code change that neither fixes a bug nor adds a feature (e.g., renaming variables, improving code structure, reorganizing files, removing unused/dead code or obsolete comments/commented-out code).",
        example: "refactor: Extract user service from main controller",
        priority: 6,
    },
    CommitType {
        name: "build",
        description: "Changes that affect the build system or external dependencies (e.g., Webpack, NPM, package.json updates).",
        example: "build: Configure webpack for tree shaking optimization",
        priority: 5,
    },
    CommitType {
        name: "ci",
        description: "Changes to CI configuration files and scripts (e.g., GitHub Actions, Travis, deployment pipelines).",
        example: "ci: Add automated deployment step to GitHub Actions",
        priority: 5,
    },
    CommitType {
        name: "test",
        description: "Adding new tests, correcting existing *failing or logically flawed* tests, or significantly altering test logic. IMPORTANT: Minor cleanups, comment removal, or style adjustments within test files should typically use 'refactor', 'docs', or 'style', not 'test'.",
        example: "test: Add unit tests for new payment_processor module",
        priority: 4,
    },
    CommitType {
        name: "docs",
        description: "Documentation only changes (e.g., updating README, API docs, adding, clarifying, or removing explanatory comments in code). If removing obsolete/commented-out code, 'refactor' is often more appropriate.",
        example: "docs: Update README with setup instructions",
        priority: 3,
    },
    CommitType {
        name: "style",
        description: "Changes that do not affect the meaning of the code (white-space, formatting, missing semi-colons, etc).",
        example: "style: Format code according to project guidelines",
        priority: 2,
    },
    CommitType {
        name: "chore",
        description: "Maintenance tasks, dependency updates, or tooling changes that don't modify application code.",
        example: "chore: Update ESLint to version 8.50.0",
        priority: 3,
    },
    CommitType {
        name: "revert",
        description: "Reverts a previous commit.",
        example: "revert: Revert commit 'abcdef12' due to critical bug",
        priority: 8,
    },
    CommitType {
        name: "readme",
        description: "Specifically for standalone changes to the README file only. If README changes are part of a larger 'feat' or 'docs' effort, use that type.",
        example: "readme: Add contribution guidelines and code of conduct",
        priority: 2,
    },
];

fn format_commit_types_for_prompt() -> String {
    let mut s = String::new();
    let mut sorted_commit_types: Vec<CommitType> = COMMIT_TYPES.to_vec();
    sorted_commit_types
        .sort_by(|a, b| b.priority.cmp(&a.priority).then_with(|| a.name.cmp(b.name)));
    for ct in sorted_commit_types {
        s.push_str(&format!(
            "- {}: {} (Example: \"{}\")\n",
            ct.name, ct.description, ct.example
        ));
    }
    s
}

fn build_type_selection_guidance() -> String {
    format!(
        "CRITICAL: Type Selection Hierarchy and Guidance - When determining the commit type, strictly follow this decision process in order:\n\
         1. 'feat': New functionality, features, or initial project setup.\n\
         2. 'fix': Bug fixes, error corrections, or security vulnerability patches.\n\
         3. 'perf': Performance improvements without new features or bug fixes.\n\
         4. 'refactor': Restructuring code without changing its external behavior or fixing bugs/adding features. \
            This INCLUDES removing unused/dead code, reorganizing files, simplifying logic, or cleaning up obsolete comments/commented-out code. \
            If changes are *solely* removing commented-out code or obsolete comments (even within test files), 'refactor' is the correct type.\n\
         5. 'build': Changes to build system, external dependencies (e.g., package.json, Cargo.toml updates).\n\
         6. 'ci': Changes to CI/CD configuration files and scripts.\n\
         7. 'docs': Changes ONLY to documentation (README, API docs, explanatory comments in code). \
            This means adding, clarifying, or removing comments that explain the code's intent or usage. \
            If comments are removed because they are obsolete or represent commented-out code, prefer 'refactor'.\n\
         8. 'test': Adding new tests, correcting existing *failing or logically flawed* tests, or significantly altering test logic/assertions. \
            IMPORTANT: Changes *within* test files that are primarily refactoring the test code itself, removing comments, or style adjustments should use 'refactor', 'docs', or 'style' respectively, NOT 'test', unless they also change test assertions or core test behavior.\n\
         9. 'style': Purely stylistic changes that do not affect code meaning or runtime behavior (e.g., whitespace, formatting, linter fixes).\n\
         10. 'chore': Maintenance tasks, tooling changes, or dependency updates not covered by 'build' or other more specific types.\n\
         \n\
         PRIMARY PURPOSE RULE: Always choose the type that represents the PRIMARY PURPOSE of the entire commit. \
         For example:\n\
         - Initial project setup (source files, README, config) is 'feat'.\n\
         - Removing obsolete comments or commented-out code from test files is 'refactor', NOT 'test'.\n\
         - Adding explanatory comments to test utility functions is 'docs', NOT 'test'.\n\
         - A bug fix that also includes adding a regression test is 'fix'.\n\
         - A feature implementation that also includes tests for the new feature is 'feat'.\n\
         - Refactoring production code and updating its corresponding tests to match the new structure is 'refactor'."
    )
}

fn build_diff_reading_guide() -> String {
    "Understanding the 'Diff' Section (How to Read Code Changes):\n\
    The 'Diff' section below shows the exact changes to the code files. It has been pre-processed to make additions and removals very explicit.\n\
    - File Indicators: Lines like 'diff --git a/path/to/file.ext b/path/to/file.ext', '--- a/path/to/file.ext', and '+++ b/path/to/file.ext' identify the files being compared. 'a/' refers to the original version and 'b/' to the new version.\n\
    - Hunk Headers: Lines starting with '@@ -old_line_info +new_line_info @@' (e.g., '@@ -242,7 +242,6 @@') mark the beginning of a \"hunk\" or a specific block of changes. The numbers indicate [start line],[number of lines] for the original and new versions of the file within that hunk.\n\
    - [REMOVED_LINE]: Any line starting with the marker '[REMOVED_LINE]: ' indicates a line that was REMOVED from the original file. The content after this marker is the actual line that was removed.\n\
    - [ADDED_LINE]: Any line starting with the marker '[ADDED_LINE]: ' indicates a line that was ADDED to the new version of the file. The content after this marker is the actual line that was added.\n\
    - CONTEXT Lines: Lines that DO NOT start with '[REMOVED_LINE]: ' or '[ADDED_LINE]: ' (and are not file/hunk headers) are UNCHANGED context lines. They are shown to help understand where the additions and removals occurred but are NOT changes themselves.\n\n\
    Your primary focus for understanding the *actual modifications* should be on the lines marked with '[ADDED_LINE]: ' and '[REMOVED_LINE]: '. \
    Based *only* on what is marked as added or removed, determine the nature of the change (e.g., adding new code, removing obsolete code, fixing a typo, refactoring logic, updating documentation comments). \
    Pay close attention to whether the content of these marked lines are code, comments, or whitespace to help select the correct commit <type>.".to_string()
}

pub fn build_prompt(
    diff_content: &str,
    changes_summary: &StagedChangesSummary,
    num_suggestions: u32,
    previous_message: Option<&str>,
) -> String {
    let commit_types_formatted = format_commit_types_for_prompt();
    let type_selection_guidance = build_type_selection_guidance();
    let diff_reading_guide = build_diff_reading_guide();

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

    if num_suggestions == 1 {
        prompt_parts.push(format!(
            "Analyze the following code changes and repository structure modifications. Generate 1 Git commit message."
        ));
    } else {
        prompt_parts.push(format!(
            "Analyze the following code changes and repository structure modifications. \
            Your task is to generate {} *alternative* Git commit messages. \
            Each of these {} messages must be a complete and valid commit message that summarizes *all* the changes provided below. \
            They should represent different ways of phrasing a *single* commit for the *entirety* of these changes, offering variations in wording or emphasis, but all pertaining to the same overall update. \
            Do not generate messages for individual files or sub-tasks within the diff if they are part of the same logical change. \
            \n\nIMPORTANT FOR MULTIPLE VARIATIONS: All {} variations should use the SAME commit type (the most appropriate one for the entire changeset). \
            Only vary the description part to provide different phrasings of the same conceptual change.",
            num_suggestions, num_suggestions, num_suggestions
        ));
    }

    prompt_parts.push("Each message MUST follow this format: <type>: <description>".to_string());
    prompt_parts.push(type_selection_guidance);
    prompt_parts.push(format!(
        "Available <type>s, their descriptions, and EXAMPLES of their use are:\n{}",
        commit_types_formatted.trim_end()
    ));

    let consistency_instruction = if num_suggestions > 1 {
        format!(
            "For the {} variations requested, determine the single most appropriate <type> that best describes the overall changes, \
            then create {} different descriptions using that same type. The variations should differ in wording, emphasis, or perspective, \
            but should all use the same commit type that represents the primary nature of the entire changeset.",
            num_suggestions, num_suggestions
        )
    } else {
        "Choose the <type> that best describes the overall changes".to_string()
    };

    prompt_parts.push(format!(
        "{}. Use the provided examples and hierarchy guidance above to ensure correct type usage.\n\
        The <description> should be concise, start with a verb in the imperative mood if possible, and be between {} and {} characters.",
        consistency_instruction, MIN_COMMIT_DESCRIPTION_CHARS, MAX_COMMIT_DESCRIPTION_CHARS
    ));

    prompt_parts
        .push("Do not include any other explanatory text, just the commit message(s).".to_string());

    if let Some(prev_msg) = previous_message {
        let num_variations_str = if num_suggestions > 1 {
            format!("{} variations of it", num_suggestions)
        } else {
            "it".to_string()
        };
        prompt_parts.push(format!(
            "The previous commit message was: '{}'. Please generate a new, improved message (or {} if multiple are requested) based on the changes, \
            considering why the previous one might have been suboptimal. Ensure the <type> is appropriate for the changes, \
            guided by the hierarchy and examples provided above. If generating multiple variations, they should all use the same improved type.",
            prev_msg, num_variations_str
        ));
    }

    prompt_parts.push(diff_reading_guide);

    prompt_parts.push("Diff:\n\n---".to_string());
    prompt_parts.push(if diff_content.trim().is_empty() {
        "No textual diff provided or detected.".to_string()
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

    #[test]
    fn test_build_prompt_basic() {
        let preprocessed_diff_example = "[ADDED_LINE]: new line";
        let summary = StagedChangesSummary {
            binary_file_changes: vec!["added binary file: image.png".to_string()],
            structure_changes: vec!["renamed: old_dir/file.txt to new_dir/file.txt".to_string()],
        };
        let prompt = build_prompt(preprocessed_diff_example, &summary, 1, None);
        assert!(prompt.contains("Generate 1 Git commit message."));
        assert!(prompt.contains("- feat: A new feature or significant functionality addition"));
        assert!(prompt.contains(&format!(
            "between {} and {} characters.",
            MIN_COMMIT_DESCRIPTION_CHARS, MAX_COMMIT_DESCRIPTION_CHARS
        )));
        assert!(prompt.contains(preprocessed_diff_example));
        assert!(prompt.contains("Binary file changes:\n\nadded binary file: image.png"));
        assert!(prompt.contains(
            "Folder structure changes:\n\nrenamed: old_dir/file.txt to new_dir/file.txt"
        ));
        assert!(!prompt.contains("The previous commit message was:"));
        assert!(prompt.contains("Use the provided examples and hierarchy guidance above"));
        assert!(prompt.contains("Understanding the 'Diff' Section (How to Read Code Changes):"));
        assert!(
            prompt.contains("[REMOVED_LINE]: Any line starting with the marker '[REMOVED_LINE]: '")
        );
        assert!(
            prompt.contains("[ADDED_LINE]: Any line starting with the marker '[ADDED_LINE]: '")
        );
    }

    #[test]
    fn test_build_prompt_multiple_suggestions() {
        let preprocessed_diff_example = "[REMOVED_LINE]: old\n[ADDED_LINE]: new";
        let summary = StagedChangesSummary::default();
        let prompt = build_prompt(preprocessed_diff_example, &summary, 5, None);
        assert!(prompt.contains("Your task is to generate 5 *alternative* Git commit messages."));
        assert!(prompt.contains("All 5 variations should use the SAME commit type"));
        assert!(prompt.contains("CRITICAL: Type Selection Hierarchy and Guidance"));
        assert!(prompt.contains("Understanding the 'Diff' Section (How to Read Code Changes):"));
        assert!(
            prompt.contains("[ADDED_LINE]: Any line starting with the marker '[ADDED_LINE]: '")
        );
    }

    #[test]
    fn test_build_prompt_with_amend_single_suggestion() {
        let preprocessed_diff_example = "[ADDED_LINE]: new content";
        let summary = StagedChangesSummary::default();
        let prev_msg = "fix: did a thing wrong";
        let prompt = build_prompt(preprocessed_diff_example, &summary, 1, Some(prev_msg));
        assert!(prompt.contains("Generate 1 Git commit message."));
        assert!(prompt.contains(&format!("The previous commit message was: '{}'.", prev_msg)));
        assert!(prompt.contains(preprocessed_diff_example));
        assert!(prompt.contains("Understanding the 'Diff' Section (How to Read Code Changes):"));
        assert!(
            prompt.contains("[ADDED_LINE]: Any line starting with the marker '[ADDED_LINE]: '")
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
        assert!(prompt.contains("Diff:\n\n---\n\nNo textual diff provided or detected.\n\n---"));
        assert!(prompt.contains("Binary file changes:\n\nadded binary file: data.zip"));
        assert!(prompt.contains("Understanding the 'Diff' Section (How to Read Code Changes):"));
    }

    #[test]
    fn test_format_commit_types_for_prompt() {
        let formatted_types = format_commit_types_for_prompt();
        assert!(
            formatted_types.contains("- feat: A new feature or significant functionality addition")
        );
        assert!(formatted_types.contains("- fix: A bug fix"));
        assert!(formatted_types.lines().count() == COMMIT_TYPES.len());
    }

    #[test]
    fn test_type_selection_guidance_generation() {
        let guidance = build_type_selection_guidance();
        assert!(guidance.contains("CRITICAL: Type Selection Hierarchy and Guidance"));
        assert!(guidance.contains("strictly follow this decision process in order:"));
        assert!(guidance.contains(
            "If changes are *solely* removing commented-out code or obsolete comments (even within test files), 'refactor' is the correct type."
        ));
    }

    #[test]
    fn test_diff_reading_guide_generation_updated_markers() {
        let guide = build_diff_reading_guide();
        assert!(guide.starts_with("Understanding the 'Diff' Section (How to Read Code Changes):"));
        assert!(
            guide.contains(
                "It has been pre-processed to make additions and removals very explicit."
            )
        );
        assert!(
            guide.contains("[REMOVED_LINE]: Any line starting with the marker '[REMOVED_LINE]: '")
        );
        assert!(guide.contains("[ADDED_LINE]: Any line starting with the marker '[ADDED_LINE]: '"));
        assert!(guide.contains(
            "CONTEXT Lines: Lines that DO NOT start with '[REMOVED_LINE]: ' or '[ADDED_LINE]: '"
        ));
        assert!(guide.contains("Your primary focus for understanding the *actual modifications* should be on the lines marked with '[ADDED_LINE]: ' and '[REMOVED_LINE]: '."));
    }
}
