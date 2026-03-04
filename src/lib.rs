pub mod core {
    use std::fmt::Debug;

    fn normalize(p_v: &mut Vec<f64>) {
        match p_v.iter().sum() {
            // Impossible outcome. Values don't matter. Don't normalize.
            0.0 => {}
            sum => {
                for p in p_v {
                    *p /= sum;
                }
            }
        }
    }

    trait Distribution<Outcome>: Debug {
        // Probability of a given outcome.
        fn p(&self, outcome: Outcome) -> f64;
    }

    #[derive(Debug)]
    pub struct Bernoulli {
        prior: f64,
    }

    impl Distribution<bool> for Bernoulli {
        fn p(&self, outcome: bool) -> f64 {
            match outcome {
                true => self.prior,
                false => 1.0 - self.prior,
            }
        }
    }

    #[derive(Debug)]
    pub struct TestOutcomeDistributions<T> {
        old: Box<dyn Distribution<T>>,
        new: Box<dyn Distribution<T>>,
    }

    /// How likely a given test outcome is given:
    /// - our assumption about which revisions belong to which distribution
    /// - current priors on those distributions
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
        /// Probability that a revision k is the source of the regression,
        /// i.e. is the first to follow the new distribution, is `ps[k]`.
        ps: Vec<f64>,
    }

    impl Debug for RegressionProbabilities {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("RegressionProbabilities")
                .field("entropy", &format_args!("{:.3}", self.entropy()))
                .field("ps", &format_args!("{:.3?}", self.ps))
                .finish()
        }
    }

    impl RegressionProbabilities {
        fn initialize(num_revisions: usize) -> RegressionProbabilities {
            // Revision 0 cannot be the source of the regression, by assumption.
            let mut ps = vec![0.0];
            // Remaining revisions initially have a uniform distribution.
            // In the future, we'll accept priors, e.g. to prioritise revisions touching specific
            // directories or committed by specific authors.
            ps.append(&mut vec![
                1.0 / (num_revisions - 1) as f64;
                num_revisions - 1
            ]);
            RegressionProbabilities { ps }
        }

        fn update_with_sample(
            &mut self,
            distributions: &TestOutcomeDistributions<bool>,
            sample_revision: usize,
            sample_outcome: bool,
        ) -> &RegressionProbabilities {
            for (curr_revision, curr_p_regression) in self.ps.iter_mut().enumerate() {
                // p(C_i | E) = p(E | C_i) * P(C_i) / (P(E)
                // Here, we skip division by P(E) and will later normalize.

                // If the sample was drawn from revision r <= i,
                // we expect it to fit the old distribution.
                // The old distribution is, for now, Bernoulli with p=old_prior.
                // Later, as we gather samples, we need to factor them
                // into the expected old distribution. (TODO yes/no)
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

        /// Calculates the Shannon entropy of the set of probabilities.
        /// Assumes probabilities are normalized, i.e. sum to 1.
        fn entropy(&self) -> f64 {
            self.ps
                .iter()
                .filter(|p| **p != 0.0)
                .map(|p| p * -p.log2())
                .sum()
        }

        /// The highest probability of any individual revision being the source of the regression.
        fn confidence(&self) -> f64 {
            *self.ps.iter().max_by(|a, b| a.total_cmp(b)).unwrap()
        }

        fn most_likely_regression_revision(&self) -> usize {
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
        // The likelihood of the outcome depends on the chance the regression has already happened for that sample.
        // Let's calculate the probability with which the sample is drawn from each distribution.
        let p_sample_before_regression: f64 = regression_ps.ps[sample_revision + 1..].iter().sum();

        log::debug!(
            "Hypothesis: if we were to sample revision {} ({:.1}% likely pre-regression)...",
            sample_revision,
            p_sample_before_regression * 100.0
        );

        // If we were to sample the given revision, we'd expect one of the following outcomes.
        let possible_sample_outcomes = vec![false, true];
        struct ResultPerOutcome {
            likelihood: f64,
            new_entropy: f64,
        }

        let expected_entropy: f64 = possible_sample_outcomes
            .iter()
            .map(|&sample_outcome| {
                // How likely is this outcome if we tested this revision?
                // It's either drawn from the old distribution or the new one,
                // and we weigh it by how likely it is to have been drawn from either of them.
                let p_sample_outcome = probability_of_outcome(
                    sample_outcome,
                    distributions,
                    p_sample_before_regression,
                );

                // What would the regression probability look like after seeing this outcome?
                let mut new_regression_ps = regression_ps.clone();
                new_regression_ps.update_with_sample(
                    distributions,
                    sample_revision,
                    sample_outcome,
                );

                // What would the overall entropy be?
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

    pub fn wip() {
        let num_revisions = 9;

        // --- Per-revision regression prior ---
        let mut regression_ps = RegressionProbabilities::initialize(num_revisions);
        log::debug!("Got {} revisions, {:?}", num_revisions, regression_ps);

        // --- Test outcome priors ---
        // Expected distributions of test outcomes before and after the regression.
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
            let next_revision_to_test = next_revision_to_test(&regression_ps, &priors);
            log::info!("Next revision to test: {next_revision_to_test}");

            let sample_outcome = match next_revision_to_test < actual_regression_revision {
                true => old_rng.next().unwrap(),
                false => new_rng.next().unwrap(),
            };
            log::info!(
                "Iteration {iteration}: testing revision {next_revision_to_test} resulted in outcome {sample_outcome}"
            );
            regression_ps.update_with_sample(&priors, next_revision_to_test, sample_outcome);
            log::info!("Updated regression probabilities: {regression_ps:?}");
            iteration += 1;
        }
        println!(
            "After {iteration} iterations, we're {:.1}% confident that the regression was introduced in revision {}.",
            regression_ps.confidence() * 100.0,
            regression_ps.most_likely_regression_revision()
        );
    }
}
