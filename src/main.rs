use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "psect")]
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
            psect::core::wip();
        }
    }
}
