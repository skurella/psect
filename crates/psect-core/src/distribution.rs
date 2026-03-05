use std::fmt::Debug;

pub trait Distribution<Outcome>: Debug {
    fn p(&self, outcome: Outcome) -> f64;
}

#[derive(Debug)]
pub struct Bernoulli {
    pub prior: f64,
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
    pub old: Box<dyn Distribution<T>>,
    pub new: Box<dyn Distribution<T>>,
}
