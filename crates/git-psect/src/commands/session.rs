use crate::{candidates, error::Error, repo::RepoContext, state, state::State};

const DEFAULT_OLD_PASS_RATE: f64 = 0.95;
const DEFAULT_NEW_PASS_RATE: f64 = 0.5;

#[derive(Clone, clap::ValueEnum)]
pub enum Bound {
    Old,
    New,
}

/// Mark a revision as an old (known-good) or new (known-bad) bound, persist
/// state, then call `advance`.
pub fn mark(
    ctx: &RepoContext,
    state: &mut State,
    rev: Option<String>,
    bound: Bound,
) -> Result<(), Error> {
    let refspec = rev.as_deref().unwrap_or("HEAD");
    let sha = ctx.resolve_rev(refspec)?;

    match bound {
        Bound::Old => {
            if !state.old_revisions.is_empty() {
                return Err(Error::Validation(
                    "only one old revision is supported for now".into(),
                ));
            }
            state.old_revisions.push(sha.clone());
        }
        Bound::New => {
            if !state.new_revisions.is_empty() {
                return Err(Error::Validation(
                    "only one new revision is supported for now".into(),
                ));
            }
            state.new_revisions.push(sha.clone());
        }
    };

    let rate = match bound {
        Bound::Old => *state
            .priors
            .old_pass_rate
            .get_or_insert(DEFAULT_OLD_PASS_RATE),
        Bound::New => *state
            .priors
            .new_pass_rate
            .get_or_insert(DEFAULT_NEW_PASS_RATE),
    };

    let next_prompt = advance(&ctx.repo, state)?;
    state::write(&ctx.state_dir, state)?;

    println!(
        "Marked {} as {}.",
        &sha[..10],
        match bound {
            Bound::Old => "old",
            Bound::New => "new",
        }
    );
    println!(
        "Expecting the test to pass {:.0}% of the time {} the regression.",
        rate * 100.0,
        match bound {
            Bound::Old => "before",
            Bound::New => "after",
        }
    );
    println!(
        "Change with 'git psect set-prior {} <rate>'.",
        match bound {
            Bound::Old => "old",
            Bound::New => "new",
        }
    );
    println!("{next_prompt}");
    Ok(())
}

/// Called after any config mutation. Either prompts the user for the next
/// required input or, once both bounds are set, kicks off the bisection.
/// Returns a trailing prompt string to be printed after the caller's own output.
pub fn advance(repo: &git2::Repository, state: &State) -> Result<String, Error> {
    if state.old_revisions.is_empty() && state.new_revisions.is_empty() {
        return Ok(concat!(
            "Waiting for reference pre-regression and post-regression revisions.\n",
            "Mark them with 'git psect old <rev>' and 'git psect new <rev>'."
        )
        .into());
    }
    if state.old_revisions.is_empty() {
        return Ok(
            "Now mark a reference pre-regression revision with 'git psect old <rev>'.".into(),
        );
    }
    if state.new_revisions.is_empty() {
        return Ok(
            "Now mark a reference post-regression revision with 'git psect new <rev>'.".into(),
        );
    }

    // Validate ancestry regardless of the order old/new were set.
    let old_sha = state.old_revisions.first().unwrap();
    let new_sha = state.new_revisions.first().unwrap();
    let old_oid = repo.revparse_single(old_sha)?.id();
    let new_oid = repo.revparse_single(new_sha)?.id();
    if !repo.graph_descendant_of(new_oid, old_oid)? {
        return Err(Error::Validation(format!(
            "'{}' is not a descendant of '{}'",
            &new_sha[..10],
            &old_sha[..10]
        )));
    }

    let candidates = candidates::build(repo, state)?;
    if candidates.is_empty() {
        return Err(Error::Validation(
            "no commits found between old and new revisions".into(),
        ));
    }
    let distributions = candidates::build_distributions(state);
    let ps = candidates::reconstruct(repo, state, &candidates, &distributions)?;
    let next_sha = candidates::checkout_next(repo, &distributions, &ps)?;
    let next_summary = repo
        .find_commit(repo.revparse_single(&next_sha)?.id())?
        .summary()
        .unwrap_or("")
        .to_string();
    Ok(format!(
        concat!(
            "Checking out {} \"{}\".\n",
            "Now either:\n",
            "- run your test and call 'git psect pass' or 'git psect fail', or\n",
            "- use 'git psect run <test>' to run on autopilot."
        ),
        &next_sha[..10],
        next_summary
    ))
}
