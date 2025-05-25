use anyhow::{Context, bail};
use clap::Parser;
use inquire::{InquireError, Select};
use std::env;
use std::io::{self, Write};
use std::path::PathBuf;

mod ai;
mod diff;
mod git;
mod prompt;

#[derive(Parser, Debug)]
#[command(
    version,
    about = "ai-commit: A personal AI-powered Git commit tool.\n\n\
             This CLI tool uses the Google Gemini API to automate or assist in\n \
             generating Git commit messages by analyzing staged code changes.\n\
             It prioritizes speed and a tight feedback loop for the solo developer."
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
    _repo_path: &PathBuf,
    preprocessed_diff_text: &str,
    changes_summary: &git::StagedChangesSummary,
    num_variations_to_request: u32,
    previous_message: Option<&str>,
    mode_description: &str,
) -> anyhow::Result<Option<String>> {
    loop {
        let prompt_str = prompt::build_prompt(
            preprocessed_diff_text,
            changes_summary,
            num_variations_to_request,
            previous_message,
        );

        if env::var("AI_COMMIT_LOG_PROMPT").is_ok() {
            println!("\n================ PROMPT SENT TO AI (INTERACTIVE) ================");
            println!("{}", prompt_str);
            println!("=================================================================\n");
        }

        print!(
            "🤖 Generating {} {}commit message variations from AI... ",
            num_variations_to_request,
            if mode_description.is_empty() {
                "".to_string()
            } else {
                format!("{} ", mode_description)
            }
        );
        io::stdout().flush()?;
        let suggestions_result = ai::generate_text(&prompt_str, num_variations_to_request).await;
        println!("\r \r");

        let suggestions = match suggestions_result {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Error generating commit messages from AI: {}", e);
                let error_options = vec![REGENERATE_OPTION, CANCEL_OPTION];
                match Select::new("AI failed. What would you like to do?", error_options).prompt() {
                    Ok(REGENERATE_OPTION) => continue,
                    Ok(CANCEL_OPTION) | Err(InquireError::OperationCanceled) => return Ok(None),
                    Ok(_) => unreachable!(),
                    Err(ie) => return Err(ie.into()),
                }
            }
        };

        if suggestions.is_empty() {
            eprintln!("❌ AI returned no valid suggestions after filtering.");
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
                Err(ie) => return Err(ie.into()),
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
    let repo_path = env::current_dir().context("Failed to get current directory")?;

    if !matches!(mode, AiCommitMode::Auto | AiCommitMode::Interactive) {
        if !git::has_staged_files(&repo_path).context("Failed to check for staged files")?
            && !matches!(
                mode,
                AiCommitMode::AmendAuto | AiCommitMode::AmendInteractive
            )
        {
            if !args.amend {
                println!("ℹ️ No files staged for commit. Nothing to do.");
                return Ok(());
            }
        }
    } else {
        if !git::has_staged_files(&repo_path).context("Failed to check for staged files")? {
            println!("ℹ️ No files staged for commit. Nothing to do.");
            return Ok(());
        }
    }

    match mode {
        AiCommitMode::Auto => {
            let raw_diff_text = match git::get_staged_diff(&repo_path) {
                Ok(diff) if !diff.is_empty() => diff,
                Ok(_) => {
                    println!(
                        "ℹ️ No textual diff detected for staged changes. The AI will rely on file names and types."
                    );
                    String::new()
                }
                Err(e) => {
                    eprintln!("Error getting staged diff: {}", e);
                    return Err(e);
                }
            };

            let preprocessed_diff_text = if !raw_diff_text.is_empty() {
                diff::preprocess_diff_for_ai(&raw_diff_text)
            } else {
                String::new()
            };

            let changes_summary = match git::get_staged_changes_summary(&repo_path) {
                Ok(summary) => summary,
                Err(e) => {
                    eprintln!("Error getting staged changes summary: {}", e);
                    return Err(e);
                }
            };

            let prompt_str =
                prompt::build_prompt(&preprocessed_diff_text, &changes_summary, 1, None);

            if env::var("AI_COMMIT_LOG_PROMPT").is_ok() {
                println!("\n================ PROMPT SENT TO AI (AUTO MODE) ================");
                println!("{}", prompt_str);
                println!("==============================================================\n");
            }

            print!("🤖 Generating commit message from AI... ");
            io::stdout().flush()?;
            let suggestions_result = ai::generate_text(&prompt_str, 1).await;
            println!("\r \r");

            let suggestions = match suggestions_result {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("Error generating commit message from AI: {}", e);
                    return Err(e.into());
                }
            };

            let commit_message = suggestions.get(0).map(String::as_str).unwrap_or("").trim();
            if commit_message.is_empty() {
                eprintln!(
                    "❌ AI returned an empty or invalid commit message after filtering. Cannot commit."
                );
                return Err(anyhow::anyhow!(
                    "AI returned an empty or invalid commit message."
                ));
            }
            println!("✨ AI Suggests: \"{}\"", commit_message);
            match git::commit_staged_files(&repo_path, commit_message) {
                Ok(commit_output) => {
                    println!("\n✅ Automatically committed with AI-generated message:");
                    println!("{}", commit_output);
                }
                Err(e) => {
                    eprintln!("\n❌ Failed to commit staged files: {}", e);
                    eprintln!("Generated message was: \"{}\"", commit_message);
                    eprintln!("Please commit manually or try again.");
                    return Err(e);
                }
            }
        }
        AiCommitMode::Interactive => {
            let raw_diff_text = match git::get_staged_diff(&repo_path) {
                Ok(diff) if !diff.is_empty() => diff,
                Ok(_) => {
                    println!(
                        "ℹ️ No textual diff detected for staged changes. The AI will rely on file names and types."
                    );
                    String::new()
                }
                Err(e) => {
                    eprintln!("Error getting staged diff: {}", e);
                    return Err(e);
                }
            };

            let preprocessed_diff_text = if !raw_diff_text.is_empty() {
                diff::preprocess_diff_for_ai(&raw_diff_text)
            } else {
                String::new()
            };

            let changes_summary = match git::get_staged_changes_summary(&repo_path) {
                Ok(summary) => summary,
                Err(e) => {
                    eprintln!("Error getting staged changes summary: {}", e);
                    return Err(e);
                }
            };
            let num_variations_to_request = 5;

            match interactive_commit_loop(
                &repo_path,
                &preprocessed_diff_text,
                &changes_summary,
                num_variations_to_request,
                None,
                "",
            )
            .await
            {
                Ok(Some(selected_message)) => {
                    println!("✨ You selected: \"{}\"", selected_message);
                    match git::commit_staged_files(&repo_path, &selected_message) {
                        Ok(commit_output) => {
                            println!("\n✅ Committed with selected message:");
                            println!("{}", commit_output);
                        }
                        Err(e) => {
                            eprintln!("\n❌ Failed to commit staged files: {}", e);
                            eprintln!("Selected message was: \"{}\"", selected_message);
                            return Err(e);
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
        AiCommitMode::AmendAuto | AiCommitMode::AmendInteractive => {
            let previous_commit_msg = match git::get_previous_commit_message(&repo_path)
                .context("Failed to get previous commit message for amend operation")?
            {
                Some(msg) => msg,
                None => {
                    bail!("❌ Cannot amend: No previous commit found in this repository.");
                }
            };
            println!(
                "💬 Previous commit message: \"{}\"",
                previous_commit_msg.lines().next().unwrap_or_default()
            );

            let raw_diff_text = match git::get_staged_diff(&repo_path) {
                Ok(diff) => diff,
                Err(e) => {
                    eprintln!("Error getting staged diff for amend: {}", e);
                    return Err(e);
                }
            };

            let preprocessed_diff_text = if !raw_diff_text.is_empty() {
                diff::preprocess_diff_for_ai(&raw_diff_text)
            } else {
                String::new()
            };

            let changes_summary = match git::get_staged_changes_summary(&repo_path) {
                Ok(summary) => summary,
                Err(e) => {
                    eprintln!("Error getting staged changes summary for amend: {}", e);
                    return Err(e);
                }
            };

            if mode == AiCommitMode::AmendAuto {
                let prompt_str = prompt::build_prompt(
                    &preprocessed_diff_text,
                    &changes_summary,
                    1,
                    Some(&previous_commit_msg),
                );

                if env::var("AI_COMMIT_LOG_PROMPT").is_ok() {
                    println!(
                        "\n================ PROMPT SENT TO AI (AMEND AUTO MODE) ================"
                    );
                    println!("{}", prompt_str);
                    println!(
                        "====================================================================\n"
                    );
                }

                print!("🤖 Generating new commit message for amend (auto)... ");
                io::stdout().flush()?;
                let suggestions_result = ai::generate_text(&prompt_str, 1).await;
                println!("\r \r");

                let suggestions = match suggestions_result {
                    Ok(s) => s,
                    Err(e) => {
                        eprintln!("Error generating commit message from AI for amend: {}", e);
                        return Err(e.into());
                    }
                };
                let new_commit_message =
                    suggestions.get(0).map(String::as_str).unwrap_or("").trim();

                if new_commit_message.is_empty() {
                    eprintln!(
                        "❌ AI returned an empty or invalid commit message for amend after filtering. Cannot amend."
                    );
                    return Err(anyhow::anyhow!(
                        "AI returned an empty or invalid commit message for amend."
                    ));
                }
                println!("✨ AI Suggests for amend: \"{}\"", new_commit_message);
                match git::amend_commit(&repo_path, new_commit_message) {
                    Ok(commit_output) => {
                        println!("\n✅ Successfully amended commit with AI-generated message:");
                        println!("{}", commit_output);
                    }
                    Err(e) => {
                        eprintln!("\n❌ Failed to amend commit: {}", e);
                        eprintln!("Generated message was: \"{}\"", new_commit_message);
                        return Err(e);
                    }
                }
            } else {
                let num_variations_to_request = 5;
                match interactive_commit_loop(
                    &repo_path,
                    &preprocessed_diff_text,
                    &changes_summary,
                    num_variations_to_request,
                    Some(&previous_commit_msg),
                    "amend",
                )
                .await
                {
                    Ok(Some(selected_message)) => {
                        println!("✨ You selected for amend: \"{}\"", selected_message);
                        match git::amend_commit(&repo_path, &selected_message) {
                            Ok(commit_output) => {
                                println!("\n✅ Successfully amended commit with selected message:");
                                println!("{}", commit_output);
                            }
                            Err(e) => {
                                eprintln!("\n❌ Failed to amend commit: {}", e);
                                eprintln!("Selected message was: \"{}\"", selected_message);
                                return Err(e);
                            }
                        }
                    }
                    Ok(None) => {}
                    Err(e) => {
                        eprintln!("An error occurred in the interactive amend loop: {}", e);
                        return Err(e);
                    }
                }
            }
        }
    }
    Ok(())
}
