use crate::{error::Error, repo, state};

pub fn run(rev: Option<String>) -> Result<(), Error> {
    let ctx = repo::open()?;
    let mut state = state::read(&ctx.state_dir)?;
    super::session::mark(&ctx, &mut state, rev, super::session::Bound::New)
}
