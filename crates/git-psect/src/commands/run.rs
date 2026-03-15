use std::process::Command;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use crate::{
    candidates,
    error::Error,
    repo,
    state::{self, Sample},
};

pub fn run(script: Vec<String>, confidence_threshold: f64) -> Result<(), Error> {
    let ctx = repo::open()?;
    let mut state = state::read(&ctx.state_dir)?;

    if state.old_revisions.is_empty() {
        return Err(Error::Validation("run 'git psect old <rev>' first".into()));
    }
    if state.new_revisions.is_empty() {
        return Err(Error::Validation("run 'git psect new <rev>' first".into()));
    }

    let interrupted = Arc::new(AtomicBool::new(false));
    let flag = interrupted.clone();
    ctrlc::set_handler(move || {
        flag.store(true, Ordering::SeqCst);
    })
    .map_err(|e| Error::Validation(format!("failed to set Ctrl+C handler: {e}")))?;

    loop {
        let head_sha = ctx.repo.head()?.peel_to_commit()?.id().to_string();
        println!("{}: running test...", &head_sha[..10]);

        let mut child = Command::new(&script[0]).args(&script[1..]).spawn()?;
        let status = child.wait()?;

        if interrupted.load(Ordering::SeqCst) {
            println!("Interrupted.");
            break;
        }

        let outcome = status.success();
        let verb = if outcome { "passed" } else { "failed" };
        println!("{}: test {verb}", &head_sha[..10]);

        state.samples.push(Sample {
            revision: head_sha,
            outcome,
            recorded_at: chrono::Utc::now().to_rfc3339(),
            comment: None,
        });
        state::write(&ctx.state_dir, &state)?;

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

        if confidence >= confidence_threshold {
            candidates::checkout(&ctx.repo, best_oid)?;
            println!(
                "{:.1}% chance the regression was introduced in {}: {}.",
                confidence * 100.0,
                &best[..10],
                best_summary
            );
            println!(
                "Run 'git psect reset' to clear the session or continue running tests to increase confidence."
            );
            break;
        }

        if confidence > 0.5 {
            println!(
                "Current best guess: {} \"{}\" ({:.1}% confidence).",
                &best[..10],
                best_summary,
                confidence * 100.0
            );
        }

        let next_sha = candidates::checkout_next(&ctx.repo, &distributions, &ps)?;
        let next_summary = ctx
            .repo
            .find_commit(ctx.repo.revparse_single(&next_sha)?.id())?
            .summary()
            .unwrap_or("")
            .to_string();
        println!("{}: checking out \"{}\"", &next_sha[..10], next_summary);
    }

    Ok(())
}
