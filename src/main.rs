use clap::Parser;
use std::env;

mod ai;
mod git;
mod prompt;

#[derive(Parser, Debug)]
#[command(
    version,
    about = "ai-commit: A personal AI-powered Git commit tool.\n\nThis CLI tool uses the Google Gemini API to automate or assist in\ngenerating Git commit messages by analyzing staged code changes.\nIt prioritizes speed and a tight feedback loop for the solo developer."
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

    let current_repo_path = env::current_dir()?;

    match mode {
        AiCommitMode::Auto => {
            println!("Executing Auto mode logic...");

            let diff_text = match git::get_staged_diff(&current_repo_path) {
                Ok(diff) => {
                    if diff.trim().is_empty() {
                        println!(
                            "\nNo textual diff detected for staged changes, but files might be staged (e.g. mode changes or only binary/structure changes)."
                        );
                    } else {
                        println!("\nStaged Diff:\n{}", diff);
                    }
                    diff
                }
                Err(e) => {
                    eprintln!("{}", e);
                    return Ok(());
                }
            };

            let changes_summary = match git::get_staged_changes_summary(&current_repo_path) {
                Ok(summary) => {
                    println!("\nStaged Changes Summary:");
                    if summary.binary_file_changes.is_empty()
                        && summary.structure_changes.is_empty()
                    {
                        println!(
                            "  No specific binary or structural changes detected by summary logic."
                        );
                    } else {
                        if !summary.binary_file_changes.is_empty() {
                            println!("  Binary file changes: {:?}", summary.binary_file_changes);
                        }
                        if !summary.structure_changes.is_empty() {
                            println!("  Structure changes: {:?}", summary.structure_changes);
                        }
                    }
                    summary
                }
                Err(e) => {
                    eprintln!("Error getting staged changes summary: {}", e);
                    return Err(e);
                }
            };

            if diff_text.trim().is_empty()
                && changes_summary.binary_file_changes.is_empty()
                && changes_summary.structure_changes.is_empty()
            {
                println!(
                    "Staged changes detected, but they appear to be non-textual and not categorized as binary/structural by current logic (e.g., mode changes only)."
                );
            }
        }
        AiCommitMode::Interactive => {
            println!("Executing Interactive mode logic (placeholder)...");
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
