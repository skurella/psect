use crate::{
    error::Error,
    repo,
    state,
};

pub fn run(rev: String) -> Result<(), Error> {
    let ctx = repo::open()?;
    let mut state = state::read(&ctx.state_dir)?;

    if !state.new_revisions.is_empty() {
        return Err(Error::Validation(
            "only one new revision is supported for now".into(),
        ));
    }

    let sha = ctx.repo.revparse_single(&rev)?.id().to_string();

    // Verify ancestry if old is already set.
    if let Some(old_sha) = state.old_revisions.first() {
        let old_oid = ctx.repo.revparse_single(old_sha)?.id();
        let new_oid = ctx.repo.revparse_single(&sha)?.id();
        if !ctx.repo.graph_descendant_of(new_oid, old_oid)? {
            return Err(Error::Validation(format!(
                "'{}' is not a descendant of '{}'",
                &sha[..8],
                &old_sha[..8]
            )));
        }
    }

    state.new_revisions.push(sha.clone());
    state::write(&ctx.state_dir, &state)?;
    println!("Marked {} as known-bad.", &sha[..8]);

    if !state.old_revisions.is_empty() {
        super::old::begin_bisection(&ctx.repo, &ctx.state_dir, &state)?;
    }
    Ok(())
}
