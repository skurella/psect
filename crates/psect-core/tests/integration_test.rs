use rand::{SeedableRng, distr::Distribution, rngs::SmallRng};
use std::{collections::HashSet, fmt::Debug};

use psect_core::{
    Bernoulli, RegressionProbabilities, TestOutcomeDistributions, next_revision_to_test,
    regression::Revision,
};

/// Verify the algorithm converges on a known regression using a simple in-memory revision list.
/// Outcomes are deterministic because the RNG is seeded with a fixed value for each revision.
#[test]
fn converges_on_known_regression() {
    let _ = env_logger::builder().is_test(true).try_init(); 

    let num_revisions = 9;
    let actual_regression_revision = 6;

    let priors = TestOutcomeDistributions {
        old: Box::new(Bernoulli { prior: 0.05 }),
        new: Box::new(Bernoulli { prior: 0.5 }),
    };
    log::info!("Priors for test outcomes: {priors:?}");


    #[derive(Hash, Eq, PartialEq, PartialOrd)]
    struct TestRev {
        id: usize,
    }
    impl Debug for TestRev {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{}", self.id)
        }
    }
    impl Revision for TestRev {}

    let mut revisions = Vec::with_capacity(num_revisions);
    for id in 0..num_revisions {
        revisions.push(TestRev { id });
    }
    let mut known_old_revisions = HashSet::new();
    known_old_revisions.insert(TestRev { id: 0 });

    let mut regression_ps = RegressionProbabilities::initialize(&revisions, &known_old_revisions);
    log::info!("Got {} revisions, {:?}", num_revisions, regression_ps);

    let mut sample_iters = revisions
        .iter()
        .map(|revision| {
            let prior = if revision.id < actual_regression_revision {
                priors.old.p(true)
            } else {
                priors.new.p(true)
            };
            let distribution = rand::distr::Bernoulli::new(prior).unwrap();
            distribution.sample_iter(SmallRng::seed_from_u64(revision.id as u64))
        })
        .collect::<Vec<_>>();

    let mut iteration = 0;
    while regression_ps.confidence() < 0.97 {
        let next = next_revision_to_test(&regression_ps, &priors).id;
        log::info!("Next revision to test: {next}");

        let sample_outcome = sample_iters[next].next().unwrap();
        log::info!(
            "Iteration {iteration}: testing revision {next} resulted in outcome {sample_outcome}"
        );
        regression_ps.update_with_sample(&priors, next, sample_outcome);
        log::info!("Updated regression probabilities: {regression_ps:?}");
        iteration += 1;
    }
    log::info!(
        "After {iteration} iterations, we're {:.1}% confident that the regression was introduced in revision {:?}.",
        regression_ps.confidence() * 100.0,
        regression_ps.most_likely_regression_revision()
    );

    assert_eq!(regression_ps.most_likely_regression_revision().id, 6);
    assert!(regression_ps.confidence() >= 0.97);
    assert_eq!(iteration, 16);
}
