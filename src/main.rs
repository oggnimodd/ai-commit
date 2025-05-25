use anyhow::Context;
use clap::Parser;
use inquire::{InquireError, Select};
use std::env;

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
            let prompt_str = prompt::build_prompt(
                &diff_text,
                &changes_summary,
                num_variations_to_request,
                None,
            );

            println!(
                "🤖 Generating {} commit message variations from AI...",
                num_variations_to_request
            );
            let suggestions = match ai::generate_text(&prompt_str, num_variations_to_request).await
            {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("Error generating commit messages from AI: {}", e);
                    return Err(e.into());
                }
            };

            if suggestions.is_empty() {
                eprintln!("❌ AI returned no suggestions. Please try again or use auto mode.");
                return Err(anyhow::anyhow!("AI returned no suggestions."));
            }

            let options: Vec<&str> = suggestions.iter().map(String::as_str).collect();

            match Select::new("Select a commit message:", options).prompt() {
                Ok(selected_message) => {
                    println!("✨ You selected: \"{}\"", selected_message);
                    if let Err(e) = git::commit_staged_files(&current_repo_path, selected_message) {
                        eprintln!("❌ Failed to commit staged files: {}", e);
                    }
                }
                Err(InquireError::OperationCanceled) => {
                    println!("❌ Commit message selection cancelled.");
                }
                Err(e) => {
                    eprintln!("Error during selection: {}", e);
                    return Err(e.into());
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
