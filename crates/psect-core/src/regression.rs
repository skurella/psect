use crate::distribution::TestOutcomeDistributions;

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

#[derive(Clone)]
pub struct RegressionProbabilities {
    /// Probability that revision k is the first to follow the new distribution.
    ps: Vec<f64>,
}

impl std::fmt::Debug for RegressionProbabilities {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RegressionProbabilities")
            .field("entropy", &format_args!("{:.3}", self.entropy()))
            .field("ps", &format_args!("{:.3?}", self.ps))
            .finish()
    }
}

impl RegressionProbabilities {
    pub fn initialize(num_revisions: usize) -> RegressionProbabilities {
        // Revision 0 cannot be the source of the regression, by assumption.
        let mut ps = vec![0.0];
        ps.append(&mut vec![
            1.0 / (num_revisions - 1) as f64;
            num_revisions - 1
        ]);
        RegressionProbabilities { ps }
    }

    pub fn update_with_sample(
        &mut self,
        distributions: &TestOutcomeDistributions<bool>,
        sample_revision: usize,
        sample_outcome: bool,
    ) -> &RegressionProbabilities {
        for (curr_revision, curr_p_regression) in self.ps.iter_mut().enumerate() {
            let p_of_sample_for_revision = match sample_revision < curr_revision {
                true => distributions.old.p(sample_outcome),
                false => distributions.new.p(sample_outcome),
            };
            let prior = *curr_p_regression;
            *curr_p_regression = p_of_sample_for_revision * prior;
        }
        normalize(&mut self.ps);
        self
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

    pub fn most_likely_regression_revision(&self) -> usize {
        self.ps
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.total_cmp(b))
            .map(|(i, _)| i)
            .unwrap()
    }
}

fn estimate_entropy_after_testing(
    regression_ps: &RegressionProbabilities,
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
            let p_sample_outcome = probability_of_outcome(
                sample_outcome,
                distributions,
                p_sample_before_regression,
            );

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

pub fn next_revision_to_test(
    regression_ps: &RegressionProbabilities,
    distributions: &TestOutcomeDistributions<bool>,
) -> usize {
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
    best_sample_revision
}
