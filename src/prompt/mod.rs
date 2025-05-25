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
        description: "A code change that neither fixes a bug nor adds a feature (e.g., renaming variables, improving code structure, reorganizing files).",
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
        description: "Adding missing tests or correcting existing tests without changing application logic.",
        example: "test: Add unit tests for new payment_processor module",
        priority: 4,
    },
    CommitType {
        name: "docs",
        description: "Documentation only changes that don't affect code functionality (e.g., updating README, API docs, comments).",
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
        description: "Specifically for standalone changes to the README file only.",
        example: "readme: Add contribution guidelines and code of conduct",
        priority: 2,
    },
];

fn format_commit_types_for_prompt() -> String {
    let mut s = String::new();

    let mut sorted_commit_types: Vec<CommitType> = COMMIT_TYPES.to_vec();

    sorted_commit_types.sort_by(|a, b| {
        b.priority.cmp(&a.priority) 
            .then_with(|| a.name.cmp(b.name)) 
    });

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
        "CRITICAL: Type Selection Hierarchy - When determining the commit type, follow this decision process:\n\
        1. If creating new functionality, features, or initial project setup → use 'feat'\n\
        2. If fixing bugs, errors, or security issues → use 'fix'\n\
        3. If improving performance without adding features → use 'perf'\n\
        4. If restructuring code without changing behavior → use 'refactor'\n\
        5. If changing build configuration or dependencies → use 'build'\n\
        6. If modifying CI/CD pipelines → use 'ci'\n\
        7. If only adding/updating tests → use 'test'\n\
        8. If only updating documentation → use 'docs'\n\
        9. If only formatting/style changes → use 'style'\n\
        10. If maintenance tasks or dependency updates → use 'chore'\n\n\
        IMPORTANT: Even if individual files (like README.md, package.json, etc.) are part of a larger change, \
        choose the type that represents the PRIMARY PURPOSE of the entire commit. \
        For example: Initial project setup that includes README.md, package.json, and source files should be 'feat', \
        not 'docs' or 'chore', because the primary purpose is creating new functionality."
    )
}

pub fn build_prompt(
    diff_content: &str,
    changes_summary: &StagedChangesSummary,
    num_suggestions: u32,
    previous_message: Option<&str>,
) -> String {
    let commit_types_formatted = format_commit_types_for_prompt();
    let type_selection_guidance = build_type_selection_guidance();
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
        consistency_instruction,
        MIN_COMMIT_DESCRIPTION_CHARS, 
        MAX_COMMIT_DESCRIPTION_CHARS
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
            prev_msg,
            num_variations_str
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
        assert!(prompt.contains("Generate 1 Git commit message."));
        // This assertion should still pass as 'feat' will be first after sorting by priority
        assert!(prompt.contains("- feat: A new feature or significant functionality addition (e.g., adding new endpoints, UI components, initial project setup). (Example: \"feat: Implement user authentication via OAuth\")"));
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
        // Corrected assertion:
        assert!(prompt.contains("Use the provided examples and hierarchy guidance above"));
    }

    #[test]
    fn test_build_prompt_multiple_suggestions() {
        let diff = "diff --git a/file.txt b/file.txt\nindex 123..456 100644\n--- a/file.txt\n+++ b/file.txt\n@@ -1 +1 @@\n-old\n+new";
        let summary = StagedChangesSummary::default();
        let prompt = build_prompt(diff, &summary, 5, None);
        assert!(prompt.contains("Your task is to generate 5 *alternative* Git commit messages."));
        assert!(prompt.contains(
            "Each of these 5 messages must be a complete and valid commit message that summarizes *all* the changes provided below."
        ));
        assert!(prompt.contains(
            "They should represent different ways of phrasing a *single* commit for the *entirety* of these changes"
        ));
        assert!(prompt.contains(
            "Do not generate messages for individual files or sub-tasks within the diff if they are part of the same logical change."
        ));
        assert!(prompt.contains("All 5 variations should use the SAME commit type"));
        assert!(prompt.contains("- fix: A bug fix (e.g., correcting calculation errors, addressing crashes, security vulnerabilities). (Example: \"fix: Correct off-by-one error in pagination\")"));
        assert!(prompt.contains("CRITICAL: Type Selection Hierarchy"));
    }

    #[test]
    fn test_build_prompt_with_amend_single_suggestion() {
        let diff = "diff --git a/another.txt b/another.txt\n--- a/another.txt\n+++ b/another.txt\n@@ -1 +1 @@\n-old content\n+new content";
        let summary = StagedChangesSummary::default();
        let prev_msg = "fix: did a thing wrong";
        let prompt = build_prompt(diff, &summary, 1, Some(prev_msg));
        assert!(prompt.contains("Generate 1 Git commit message."));
        assert!(prompt.contains(&format!("The previous commit message was: '{}'. Please generate a new, improved message (or it if multiple are requested) based on the changes, considering why the previous one might have been suboptimal. Ensure the <type> is appropriate for the changes, guided by the hierarchy and examples provided above. If generating multiple variations, they should all use the same improved type.", prev_msg)));
        assert!(prompt.contains(diff));
        assert!(prompt.contains("Binary file changes:\n\nNo binary file changes detected.\n\n---"));
        assert!(
            prompt.contains(
                "Folder structure changes:\n\nNo folder structure changes detected.\n\n---"
            )
        );
    }

    #[test]
    fn test_build_prompt_with_amend_multiple_suggestions() {
        let diff = "diff --git a/another.txt b/another.txt\n--- a/another.txt\n+++ b/another.txt\n@@ -1 +1 @@\n-old content\n+new content";
        let summary = StagedChangesSummary::default();
        let prev_msg = "fix: did a thing wrong";
        let prompt = build_prompt(diff, &summary, 3, Some(prev_msg));
        assert!(prompt.contains("Your task is to generate 3 *alternative* Git commit messages."));
        assert!(prompt.contains(&format!("The previous commit message was: '{}'. Please generate a new, improved message (or 3 variations of it if multiple are requested) based on the changes, considering why the previous one might have been suboptimal. Ensure the <type> is appropriate for the changes, guided by the hierarchy and examples provided above. If generating multiple variations, they should all use the same improved type.", prev_msg)));
        assert!(prompt.contains(diff));
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
        assert!(formatted_types.contains("- feat: A new feature or significant functionality addition (e.g., adding new endpoints, UI components, initial project setup). (Example: \"feat: Implement user authentication via OAuth\")"));
        assert!(formatted_types.contains("- fix: A bug fix (e.g., correcting calculation errors, addressing crashes, security vulnerabilities). (Example: \"fix: Correct off-by-one error in pagination\")"));
        assert!(formatted_types.ends_with(")\n"));
        assert_eq!(formatted_types.lines().count(), COMMIT_TYPES.len());
    }

    #[test]
    fn test_type_selection_guidance_generation() {
        let guidance = build_type_selection_guidance();
        assert!(guidance.contains("CRITICAL: Type Selection Hierarchy"));
        assert!(guidance.contains("If creating new functionality, features, or initial project setup → use 'feat'"));
        assert!(guidance.contains("choose the type that represents the PRIMARY PURPOSE"));
        assert!(guidance.contains("Initial project setup that includes README.md, package.json, and source files should be 'feat'"));
    }
}
