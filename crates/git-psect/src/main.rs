mod candidates;
mod commands;
mod error;
mod repo;
mod state;

use clap::{Parser, Subcommand};

fn parse_pass_rate(s: &str) -> Result<f64, String> {
    let v: f64 = s
        .parse()
        .map_err(|_| format!("'{s}' is not a valid number"))?;
    if (0.0..=1.0).contains(&v) {
        Ok(v)
    } else {
        Err(format!("pass rate must be between 0 and 1, got {v}"))
    }
}

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
    /// Mark a revision as the pre-regression reference
    Old { rev: Option<String> },
    /// Mark a revision as the post-regression reference
    New { rev: Option<String> },
    /// Set the expected pass rate for old or new revisions
    SetPrior {
        bound: commands::session::Bound,
        #[arg(value_parser = parse_pass_rate)]
        pass_rate: f64,
    },
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
    /// Print the path to the session state file
    State,
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
        Commands::SetPrior { bound, pass_rate } => commands::set_prior::run(bound, pass_rate),
        Commands::Pass { comment } => commands::pass_fail::run(true, comment),
        Commands::Fail { comment } => commands::pass_fail::run(false, comment),
        Commands::State => commands::state_cmd::run(),
        Commands::Reset => commands::reset::run(),
    };

    if let Err(e) = result {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}
