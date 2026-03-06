use std::{collections::HashSet, fmt::Debug, hash::Hash};

use clap::{Parser, Subcommand};
use psect_core::{
    Bernoulli, RegressionProbabilities, TestOutcomeDistributions, next_revision_to_test,
    regression::Revision,
};

#[derive(Parser)]
#[command(name = "git-psect")]
#[command(about = "probabilistic regression search")]
#[command(version = env!("CARGO_PKG_VERSION"))]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Run the WIP function
    Wip {},
}

fn main() {
    env_logger::init();

    let cli = Cli::parse();

    match &cli.command {
        Commands::Wip {} => {
            wip();
        }
    }
}

fn wip() {
}
