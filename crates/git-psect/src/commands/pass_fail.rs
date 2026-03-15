use crate::{
    candidates,
    error::Error,
    repo,
    state::{self, Sample},
};

const CONFIDENCE_THRESHOLD: f64 = 0.95;

pub fn run(outcome: bool, comment: Option<String>) -> Result<(), Error> {
    let ctx = repo::open()?;
    let mut state = state::read(&ctx.state_dir)?;

    if state.old_revisions.is_empty() {
        return Err(Error::Validation("run 'git psect old <rev>' first".into()));
    }
    if state.new_revisions.is_empty() {
        return Err(Error::Validation("run 'git psect new <rev>' first".into()));
    }

    let head_sha = ctx.repo.head()?.peel_to_commit()?.id().to_string();

    state.samples.push(Sample {
        revision: head_sha.clone(),
        outcome,
        recorded_at: chrono::Utc::now().to_rfc3339(),
        comment,
    });
    state::write(&ctx.state_dir, &state)?;

    let verb = if outcome { "passed" } else { "failed" };
    println!("{} {verb}.", &head_sha[..10]);

    let candidates = candidates::build(&ctx.repo, &state)?;
    let distributions = candidates::build_distributions(&state);
    let ps = candidates::reconstruct(&ctx.repo, &state, &candidates, &distributions)?;

    let confidence = ps.confidence();
    let best = ps.most_likely_regression_revision().0.to_string();
    let best_oid = ctx.repo.revparse_single(&best)?.id();
    let best_summary = ctx
        .repo
        .find_commit(best_oid)?
        .summary()
        .unwrap_or("")
        .to_string();

    if confidence >= CONFIDENCE_THRESHOLD {
        println!(
            "{:.1}% chance of regression introduced in {}: {}",
            confidence * 100.0,
            &best[..10],
            best_summary
        );
        println!(
            "Run 'git psect reset' to clear the session or continue running tests to increase the confidence."
        );
    } else {
        if confidence > 0.5 {
            println!(
                "Current best guess: {} ({:.1}% confidence).",
                &best[..10],
                confidence * 100.0
            );
        }
    }
    let next_sha = candidates::checkout_next(&ctx.repo, &distributions, &ps)?;
    println!(
        "Checking out {}. Run your test then call 'git psect pass' or 'git psect fail'.",
        &next_sha[..10]
    );

    Ok(())
}
