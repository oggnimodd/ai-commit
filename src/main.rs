use anyhow::Context;
use clap::Parser;
use inquire::{InquireError, Select};
use std::env;
use std::path::PathBuf;

mod ai;
mod git;
mod prompt;

#[derive(Parser, Debug)]
#[command(
    version,
    about = "ai-commit: A personal AI-powered Git commit tool.\n\nThis CLI tool uses the Google Gemini API to automate or assist in\n generating Git commit messages by analyzing staged code changes.\nIt prioritizes speed and a tight feedback loop for the solo developer."
)]
struct Args {
    #[arg(short, long)]
    interactive: bool,
    #[arg(short = 'a', long)]
    amend: bool,
}

#[derive(Debug, PartialEq)]
enum AiCommitMode {
    Auto,
    Interactive,
    AmendAuto,
    AmendInteractive,
}

impl Args {
    fn determine_mode(&self) -> AiCommitMode {
        match (self.interactive, self.amend) {
            (false, false) => AiCommitMode::Auto,
            (true, false) => AiCommitMode::Interactive,
            (false, true) => AiCommitMode::AmendAuto,
            (true, true) => AiCommitMode::AmendInteractive,
        }
    }
}

const REGENERATE_OPTION: &str = "🔄 Regenerate suggestions";
const CANCEL_OPTION: &str = "❌ Cancel and exit";

async fn interactive_commit_loop(
    current_repo_path: &PathBuf,
    diff_text: &str,
    changes_summary: &git::StagedChangesSummary,
    num_variations_to_request: u32,
) -> anyhow::Result<Option<String>> {
    loop {
        let prompt_str =
            prompt::build_prompt(diff_text, changes_summary, num_variations_to_request, None);
        println!(
            "🤖 Generating {} commit message variations from AI...",
            num_variations_to_request
        );

        let suggestions = match ai::generate_text(&prompt_str, num_variations_to_request).await {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Error generating commit messages from AI: {}", e);

                let error_options = vec![REGENERATE_OPTION, CANCEL_OPTION];
                match Select::new("AI failed. What would you like to do?", error_options).prompt() {
                    Ok(REGENERATE_OPTION) => continue,
                    Ok(CANCEL_OPTION) | Err(InquireError::OperationCanceled) => return Ok(None),
                    Ok(_) => unreachable!(),
                    Err(e) => return Err(e.into()),
                }
            }
        };

        if suggestions.is_empty() {
            eprintln!("❌ AI returned no suggestions.");
            let empty_options = vec![REGENERATE_OPTION, CANCEL_OPTION];
            match Select::new(
                "AI returned no suggestions. What would you like to do?",
                empty_options,
            )
            .prompt()
            {
                Ok(REGENERATE_OPTION) => continue,
                Ok(CANCEL_OPTION) | Err(InquireError::OperationCanceled) => return Ok(None),
                Ok(_) => unreachable!(),
                Err(e) => return Err(e.into()),
            }
        }

        let mut options: Vec<String> = suggestions.clone();
        options.push(REGENERATE_OPTION.to_string());
        options.push(CANCEL_OPTION.to_string());

        match Select::new("Select a commit message (or action):", options).prompt() {
            Ok(selected_item) => {
                if selected_item == REGENERATE_OPTION {
                    continue;
                } else if selected_item == CANCEL_OPTION {
                    println!("❌ Commit process cancelled by user.");
                    return Ok(None);
                } else {
                    return Ok(Some(selected_item));
                }
            }
            Err(InquireError::OperationCanceled) => {
                println!("❌ Commit message selection cancelled.");
                return Ok(None);
            }
            Err(e) => {
                eprintln!("Error during selection: {}", e);
                return Err(e.into());
            }
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let mode = args.determine_mode();
    println!("ai-commit is running in: {:?}", mode);

    let current_repo_path = env::current_dir().context("Failed to get current directory")?;

    match mode {
        AiCommitMode::Auto => {
            let diff_text = match git::get_staged_diff(&current_repo_path) {
                Ok(diff) => diff,
                Err(e) => {
                    eprintln!("{}", e);
                    return Ok(());
                }
            };
            let changes_summary = match git::get_staged_changes_summary(&current_repo_path) {
                Ok(summary) => summary,
                Err(e) => {
                    eprintln!("Error getting staged changes summary: {}", e);
                    return Err(e.into());
                }
            };

            let prompt_str = prompt::build_prompt(&diff_text, &changes_summary, 1, None);
            println!("🤖 Generating commit message from AI...");
            let suggestions = match ai::generate_text(&prompt_str, 1).await {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("Error generating commit message from AI: {}", e);
                    return Err(e.into());
                }
            };

            let commit_message = suggestions.get(0).map(String::as_str).unwrap_or("").trim();

            if commit_message.is_empty() {
                eprintln!("❌ AI returned an empty or invalid commit message. Cannot commit.");
                return Err(anyhow::anyhow!(
                    "AI returned an empty or invalid commit message."
                ));
            }
            println!("✨ AI Suggests: \"{}\"", commit_message);

            match git::commit_staged_files(&current_repo_path, commit_message) {
                Ok(commit_output) => {
                    println!("\n✅ Automatically committed with AI-generated message:");
                    println!("{}", commit_output);
                }
                Err(e) => {
                    eprintln!("\n❌ Failed to commit staged files: {}", e);
                    eprintln!("Generated message was: \"{}\"", commit_message);
                    eprintln!("Please commit manually or try again.");
                    return Err(e.into());
                }
            }
        }
        AiCommitMode::Interactive => {
            let diff_text = match git::get_staged_diff(&current_repo_path) {
                Ok(diff) => diff,
                Err(e) => {
                    eprintln!("{}", e);
                    return Ok(());
                }
            };
            let changes_summary = match git::get_staged_changes_summary(&current_repo_path) {
                Ok(summary) => summary,
                Err(e) => {
                    eprintln!("Error getting staged changes summary: {}", e);
                    return Err(e.into());
                }
            };

            let num_variations_to_request = 5;

            match interactive_commit_loop(
                &current_repo_path,
                &diff_text,
                &changes_summary,
                num_variations_to_request,
            )
            .await
            {
                Ok(Some(selected_message)) => {
                    println!("✨ You selected: \"{}\"", selected_message);
                    match git::commit_staged_files(&current_repo_path, &selected_message) {
                        Ok(commit_output) => {
                            println!("\n✅ Committed with selected message:");
                            println!("{}", commit_output);
                        }
                        Err(e) => {
                            eprintln!("\n❌ Failed to commit staged files: {}", e);
                            eprintln!("Selected message was: \"{}\"", selected_message);
                            return Err(e.into());
                        }
                    }
                }
                Ok(None) => {}
                Err(e) => {
                    eprintln!("An error occurred in the interactive loop: {}", e);
                    return Err(e);
                }
            }
        }
        AiCommitMode::AmendAuto => {
            println!("Executing Amend (Auto) mode logic (placeholder)...");
        }
        AiCommitMode::AmendInteractive => {
            println!("Executing Amend (Interactive) mode logic (placeholder)...");
        }
    }

    Ok(())
}
