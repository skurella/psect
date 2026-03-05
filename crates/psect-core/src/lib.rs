pub mod distribution;
pub mod regression;

pub use distribution::{Bernoulli, Distribution, TestOutcomeDistributions};
pub use regression::{next_revision_to_test, RegressionProbabilities};
