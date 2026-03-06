mod candidates;
mod commands;
mod error;
mod repo;
mod state;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "git-psect")]
#[command(about = "probabilistic regression search")]
#[command(version = env!("CARGO_PKG_VERSION"))]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize a new bisection session
    Start,
    /// Mark a revision as known-good
    Old { rev: String },
    /// Mark a revision as known-bad
    New { rev: String },
    /// Record that the current revision passed the test
    Pass {
        #[arg(long)]
        comment: Option<String>,
    },
    /// Record that the current revision failed the test
    Fail {
        #[arg(long)]
        comment: Option<String>,
    },
    /// Clear the current bisection session
    Reset,
}

fn main() {
    env_logger::init();

    let cli = Cli::parse();
    let result = match cli.command {
        Commands::Start => commands::start::run(),
        Commands::Old { rev } => commands::old::run(rev),
        Commands::New { rev } => commands::new::run(rev),
        Commands::Pass { comment } => commands::pass_fail::run(true, comment),
        Commands::Fail { comment } => commands::pass_fail::run(false, comment),
        Commands::Reset => commands::reset::run(),
    };

    if let Err(e) = result {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}
