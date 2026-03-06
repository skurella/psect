use crate::{
    candidates,
    error::Error,
    repo,
    state,
};

pub fn run(rev: String) -> Result<(), Error> {
    let ctx = repo::open()?;
    let mut state = state::read(&ctx.state_dir)?;

    if !state.old_revisions.is_empty() {
        return Err(Error::Validation(
            "only one old revision is supported for now".into(),
        ));
    }

    let sha = ctx.repo.revparse_single(&rev)?.id().to_string();
    state.old_revisions.push(sha.clone());
    state::write(&ctx.state_dir, &state)?;
    println!("Marked {} as known-good.", &sha[..8]);

    if !state.new_revisions.is_empty() {
        begin_bisection(&ctx.repo, &ctx.state_dir, &state)?;
    }
    Ok(())
}

pub fn begin_bisection(
    repo: &git2::Repository,
    state_dir: &std::path::Path,
    state: &state::State,
) -> Result<(), Error> {
    let candidates = candidates::build(repo, state)?;
    if candidates.is_empty() {
        return Err(Error::Validation(
            "no commits found between old and new revisions".into(),
        ));
    }
    let distributions = candidates::build_distributions(state);
    let ps = candidates::reconstruct(repo, state, &candidates, &distributions)?;
    let next_sha = candidates::checkout_next(repo, &distributions, &ps)?;
    println!(
        "Checking out {}. Run your test then call 'git psect pass' or 'git psect fail'.",
        &next_sha[..8]
    );
    // Persist the updated state (no change here, but ensures state is current)
    state::write(state_dir, state)?;
    Ok(())
}
