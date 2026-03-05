use clap::{Parser, Subcommand};
use psect_core::{Bernoulli, RegressionProbabilities, TestOutcomeDistributions, next_revision_to_test};

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
    let num_revisions = 9;

    let mut regression_ps = RegressionProbabilities::initialize(num_revisions);
    log::debug!("Got {} revisions, {:?}", num_revisions, regression_ps);

    let priors = TestOutcomeDistributions {
        old: Box::new(Bernoulli { prior: 0.05 }),
        new: Box::new(Bernoulli { prior: 0.5 }),
    };
    log::debug!("Priors for test outcomes: {priors:?}");

    let actual_regression_revision = 6;

    let mut old_rng = rand::distr::Distribution::sample_iter(
        rand::distr::Bernoulli::new(priors.old.p(true)).unwrap(),
        rand::rng(),
    );
    let mut new_rng = rand::distr::Distribution::sample_iter(
        rand::distr::Bernoulli::new(priors.new.p(true)).unwrap(),
        rand::rng(),
    );

    let mut iteration = 0;
    while regression_ps.confidence() < 0.97 {
        let next = next_revision_to_test(&regression_ps, &priors);
        log::info!("Next revision to test: {next}");

        let sample_outcome = match next < actual_regression_revision {
            true => old_rng.next().unwrap(),
            false => new_rng.next().unwrap(),
        };
        log::info!(
            "Iteration {iteration}: testing revision {next} resulted in outcome {sample_outcome}"
        );
        regression_ps.update_with_sample(&priors, next, sample_outcome);
        log::info!("Updated regression probabilities: {regression_ps:?}");
        iteration += 1;
    }
    println!(
        "After {iteration} iterations, we're {:.1}% confident that the regression was introduced in revision {}.",
        regression_ps.confidence() * 100.0,
        regression_ps.most_likely_regression_revision()
    );
}
