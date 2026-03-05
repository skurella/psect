use std::{cmp::Ordering, collections::HashSet, fmt::Debug, hash::Hash, iter::zip};

use crate::distribution::TestOutcomeDistributions;

pub trait Revision: Debug + Eq + Hash + PartialOrd {}

fn normalize(p_v: &mut Vec<f64>) {
    match p_v.iter().sum() {
        0.0 => {}
        sum => {
            for p in p_v {
                *p /= sum;
            }
        }
    }
}

fn probability_of_outcome<T: Copy>(
    sample: T,
    distributions: &TestOutcomeDistributions<T>,
    p_sample_before_regression: f64,
) -> f64 {
    return p_sample_before_regression * distributions.old.p(sample)
        + (1.0 - p_sample_before_regression) * distributions.new.p(sample);
}

pub struct RegressionProbabilities<'a, R: Revision> {
    revisions: &'a Vec<R>,

    /// Probability that revision k is the first to follow the new distribution.
    ps: Vec<f64>,
}

impl<'a, R: Revision> Clone for RegressionProbabilities<'a, R> {
    fn clone(&self) -> Self {
        RegressionProbabilities {
            revisions: self.revisions,
            ps: self.ps.clone(),
        }
    }
}

impl<'a, R: Revision> Debug for RegressionProbabilities<'a, R> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut debug_struct = f.debug_struct("RegressionProbabilities");
        debug_struct.field("entropy", &format_args!("{:.3}", self.entropy()));
        for (revision, p) in zip(self.revisions, &self.ps) {
            debug_struct.field(&format!("{:.7?}", revision), &format_args!("{:.3?}", p));
        }
        debug_struct.finish()
    }
}

impl<'a, R: Revision> RegressionProbabilities<'a, R> {
    pub fn initialize(
        revisions: &'a Vec<R>,
        known_old: &HashSet<R>,
    ) -> RegressionProbabilities<'a, R> {
        let num_revisions = revisions.len();
        let num_known_old_revisions = known_old.len();

        // Known-old revisions cannot be the source of the regression, by assumption.
        let num_possible_regression_revisions = num_revisions - num_known_old_revisions;
        let initial_probability = 1.0 / num_possible_regression_revisions as f64;

        let ps: Vec<f64> = revisions
            .iter()
            .map(|r| {
                if known_old.contains(r) {
                    0.0
                } else {
                    initial_probability
                }
            })
            .collect();

        RegressionProbabilities { revisions, ps }
    }

    pub fn update_with_sample(
        &mut self,
        distributions: &TestOutcomeDistributions<bool>,
        sample_revision: usize,
        sample_outcome: bool,
    ) {
        for (curr_revision, curr_p_regression) in self.ps.iter_mut().enumerate() {
            let p_of_sample_for_revision = match sample_revision.partial_cmp(&curr_revision) {
                Some(Ordering::Less) => distributions.old.p(sample_outcome),
                Some(Ordering::Equal) | Some(Ordering::Greater) => {
                    distributions.new.p(sample_outcome)
                }
                None => panic!("DAGs are not yet supported, revisions must be totally ordered"),
            };
            let prior = *curr_p_regression;
            *curr_p_regression = p_of_sample_for_revision * prior;
        }
        normalize(&mut self.ps);
    }

    fn entropy(&self) -> f64 {
        self.ps
            .iter()
            .filter(|p| **p != 0.0)
            .map(|p| p * -p.log2())
            .sum()
    }

    pub fn confidence(&self) -> f64 {
        *self.ps.iter().max_by(|a, b| a.total_cmp(b)).unwrap()
    }

    pub fn most_likely_regression_revision(&self) -> &R {
        zip(self.revisions, &self.ps)
            .max_by(|(_, a), (_, b)| a.total_cmp(b))
            .map(|(revision, _)| revision)
            .unwrap()
    }
}

fn estimate_entropy_after_testing<R: Revision>(
    regression_ps: &RegressionProbabilities<R>,
    distributions: &TestOutcomeDistributions<bool>,
    sample_revision: usize,
) -> f64 {
    let p_sample_before_regression: f64 = regression_ps.ps[sample_revision + 1..].iter().sum();

    log::debug!(
        "Hypothesis: if we were to sample revision {} ({:.1}% likely pre-regression)...",
        sample_revision,
        p_sample_before_regression * 100.0
    );

    let possible_sample_outcomes = vec![false, true];
    struct ResultPerOutcome {
        likelihood: f64,
        new_entropy: f64,
    }

    let expected_entropy: f64 = possible_sample_outcomes
        .iter()
        .map(|&sample_outcome| {
            let p_sample_outcome =
                probability_of_outcome(sample_outcome, distributions, p_sample_before_regression);

            let mut new_regression_ps = regression_ps.clone();
            new_regression_ps.update_with_sample(distributions, sample_revision, sample_outcome);

            let entropy = new_regression_ps.entropy();
            log::debug!(
                "    {:.1}% chance it will test {}, resulting in {:?}",
                p_sample_outcome * 100.0,
                sample_outcome,
                new_regression_ps
            );

            ResultPerOutcome {
                likelihood: p_sample_outcome,
                new_entropy: entropy,
            }
        })
        .filter(|result| !result.new_entropy.is_nan())
        .map(|result| result.likelihood * result.new_entropy)
        .sum();

    log::debug!(
        "...we'd expect an information gain of {:.3} shannons.",
        regression_ps.entropy() - expected_entropy
    );

    expected_entropy
}

pub fn next_revision_to_test<'a, R: Revision>(
    regression_ps: &'a RegressionProbabilities<R>,
    distributions: &'a TestOutcomeDistributions<bool>,
) -> &'a R {
    let num_revisions = regression_ps.ps.len();
    let mut best_sample_revision = 0;
    let mut lowest_expected_entropy = f64::INFINITY;
    for sample_revision in 0..num_revisions {
        let expected_entropy =
            estimate_entropy_after_testing(regression_ps, distributions, sample_revision);
        if expected_entropy < lowest_expected_entropy {
            lowest_expected_entropy = expected_entropy;
            best_sample_revision = sample_revision;
        }
    }
    &regression_ps.revisions[best_sample_revision]
}
