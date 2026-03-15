use super::session::Bound;
use crate::{error::Error, repo, state};

pub fn run(bound: Bound, pass_rate: f64) -> Result<(), Error> {
    let ctx = repo::open()?;
    let mut state = state::read(&ctx.state_dir)?;
    match bound {
        Bound::Old => {
            state.priors.old_pass_rate = Some(pass_rate);
            println!(
                "Set: expecting the test to pass {:.0}% of the time before the regression.",
                pass_rate * 100.0
            );
        }
        Bound::New => {
            state.priors.new_pass_rate = Some(pass_rate);
            println!(
                "Set: expecting the test to pass {:.0}% of the time after the regression.",
                pass_rate * 100.0
            );
        }
    }
    state::write(&ctx.state_dir, &state)
}
