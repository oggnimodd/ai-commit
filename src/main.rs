use clap::Parser;
use std::path::Path;

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

    match mode {
        AiCommitMode::Auto => {
            println!("Executing Auto mode logic...");
            match git::get_staged_diff(Path::new(".")) {
                Ok(diff) => {
                    if diff.is_empty() {
                        println!("No changes to commit or diff is empty.");
                    } else {
                        println!("\nStaged Diff:\n{}", diff);
                    }
                }
                Err(e) => {
                    eprintln!("Error getting staged diff: {}", e);
                    return Err(e);
                }
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
