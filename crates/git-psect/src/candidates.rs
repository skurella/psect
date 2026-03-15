use std::collections::HashSet;

use git2::{Oid, Repository, Sort};
use psect_core::{
    Bernoulli, RegressionProbabilities, TestOutcomeDistributions, next_revision_to_test,
    regression::Revision,
};

use crate::{error::Error, state::State};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct GitRevision(pub Oid);

impl PartialOrd for GitRevision {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.0.as_bytes().partial_cmp(other.0.as_bytes())
    }
}

impl Revision for GitRevision {}

/// Walk commits reachable from new_revisions back through (and including)
/// old_revisions, in oldest-first topological order.
pub fn build(repo: &Repository, state: &State) -> Result<Vec<GitRevision>, Error> {
    let mut walk = repo.revwalk()?;
    for sha in &state.new_revisions {
        walk.push(repo.revparse_single(sha)?.id())?;
    }
    // Hide the *parents* of old_revisions so the walk includes old_revisions
    // themselves but stops before their ancestors.
    for sha in &state.old_revisions {
        let commit = repo.find_commit(repo.revparse_single(sha)?.id())?;
        for parent in commit.parents() {
            walk.hide(parent.id())?;
        }
    }
    walk.set_sorting(Sort::TOPOLOGICAL | Sort::REVERSE)?;
    walk.map(|r| r.map(GitRevision).map_err(Error::from))
        .collect()
}

pub fn build_distributions(state: &State) -> TestOutcomeDistributions<bool> {
    TestOutcomeDistributions {
        old: Box::new(Bernoulli {
            prior: state
                .priors
                .old_pass_rate
                .expect("old_pass_rate set by `git psect old`"),
        }),
        new: Box::new(Bernoulli {
            prior: state
                .priors
                .new_pass_rate
                .expect("new_pass_rate set by `git psect new`"),
        }),
    }
}

pub fn reconstruct<'a>(
    repo: &Repository,
    state: &State,
    candidates: &'a Vec<GitRevision>,
    distributions: &TestOutcomeDistributions<bool>,
) -> Result<RegressionProbabilities<'a, GitRevision>, Error> {
    let known_old: HashSet<GitRevision> = state
        .old_revisions
        .iter()
        .map(|sha| {
            repo.revparse_single(sha)
                .map(|o| GitRevision(o.id()))
                .map_err(Error::from)
        })
        .collect::<Result<_, _>>()?;

    let mut ps = RegressionProbabilities::initialize(candidates, &known_old);
    for sample in &state.samples {
        let oid = repo.revparse_single(&sample.revision)?.id();
        let idx = candidates.iter().position(|r| r.0 == oid).ok_or_else(|| {
            Error::Validation(format!(
                "sample {} not found in candidate list",
                &sample.revision[..10]
            ))
        })?;
        ps.update_with_sample(distributions, idx, sample.outcome);
    }
    Ok(ps)
}

pub fn checkout(repo: &Repository, oid: Oid) -> Result<(), Error> {
    let commit = repo.find_commit(oid)?;
    let tree = commit.tree()?;
    repo.checkout_tree(
        tree.as_object(),
        Some(&mut git2::build::CheckoutBuilder::default()),
    )?;
    repo.set_head_detached(oid)?;
    Ok(())
}

/// Pick the next revision to test and check it out. Returns its full SHA.
pub fn checkout_next<'a>(
    repo: &Repository,
    distributions: &TestOutcomeDistributions<bool>,
    ps: &RegressionProbabilities<'a, GitRevision>,
) -> Result<String, Error> {
    let next_oid = next_revision_to_test(ps, distributions).0;
    checkout(repo, next_oid)?;
    Ok(next_oid.to_string())
}
