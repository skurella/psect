use git2::{Oid, Repository};
use git_psect::{candidates, commands::session, repo, state};
use serial_test::serial;
use tempfile::TempDir;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Create a temporary git repo with `n` linear commits.
/// Returns `(tmpdir, repo, oids)` where `oids[0]` is the oldest commit.
fn make_repo(n: usize) -> (TempDir, Repository, Vec<Oid>) {
    let tmp = tempfile::tempdir().unwrap();
    let repo = Repository::init(tmp.path()).unwrap();
    {
        let mut cfg = repo.config().unwrap();
        cfg.set_str("user.name", "Test").unwrap();
        cfg.set_str("user.email", "test@example.com").unwrap();
    }
    let sig = git2::Signature::now("Test", "test@example.com").unwrap();
    let tree_oid = {
        let tree_oid = repo.treebuilder(None).unwrap().write().unwrap();
        let tree = repo.find_tree(tree_oid).unwrap();
        // Commit just using the tree_oid; drop tree before the loop so repo is not borrowed.
        drop(tree);
        tree_oid
    };
    let mut oids = Vec::new();
    for i in 0..n {
        let tree = repo.find_tree(tree_oid).unwrap();
        let parents: Vec<git2::Commit> = if oids.is_empty() {
            vec![]
        } else {
            vec![repo.find_commit(*oids.last().unwrap()).unwrap()]
        };
        let parent_refs: Vec<&git2::Commit> = parents.iter().collect();
        let oid = repo
            .commit(
                Some("HEAD"),
                &sig,
                &sig,
                &format!("commit {i}"),
                &tree,
                &parent_refs,
            )
            .unwrap();
        drop(tree);
        oids.push(oid);
    }
    (tmp, repo, oids)
}

/// Build a `RepoContext` pointing at a repo opened from the working directory.
fn open_ctx(tmp: &TempDir) -> repo::RepoContext {
    let repo = Repository::open(tmp.path()).unwrap();
    repo::RepoContext {
        state_dir: tmp.path().join(".git").join("psect"),
        repo,
    }
}

fn make_state_with_bounds(old_sha: &str, new_sha: &str) -> state::State {
    state::State {
        meta: state::Meta {
            tool_version: "0.1.0".into(),
            started_at: "2024-01-01T00:00:00Z".into(),
        },
        priors: state::Priors {
            old_pass_rate: Some(0.95),
            new_pass_rate: Some(0.5),
        },
        old_revisions: vec![old_sha.to_string()],
        new_revisions: vec![new_sha.to_string()],
        samples: vec![],
    }
}

fn empty_state() -> state::State {
    state::State {
        meta: state::Meta {
            tool_version: "0.1.0".into(),
            started_at: "2024-01-01T00:00:00Z".into(),
        },
        priors: state::Priors::default(),
        old_revisions: vec![],
        new_revisions: vec![],
        samples: vec![],
    }
}

// ---------------------------------------------------------------------------
// repo::open
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn open_uses_git_dir_env() {
    let (tmp, _repo, _oids) = make_repo(1);
    let git_dir = tmp.path().join(".git");
    // SAFETY: tests using GIT_DIR are serialized via #[serial]
    unsafe { std::env::set_var("GIT_DIR", &git_dir) };
    let result = repo::open();
    unsafe { std::env::remove_var("GIT_DIR") };
    let ctx = result.unwrap();
    assert_eq!(ctx.state_dir, git_dir.join("psect"));
}

#[test]
#[serial]
fn open_rejects_invalid_git_dir() {
    let tmp = tempfile::tempdir().unwrap();
    // tmp.path() has no HEAD file → not a git directory
    // SAFETY: tests using GIT_DIR are serialized via #[serial]
    unsafe { std::env::set_var("GIT_DIR", tmp.path()) };
    let result = repo::open();
    unsafe { std::env::remove_var("GIT_DIR") };
    assert!(matches!(result, Err(git_psect::error::Error::Validation(_))));
}

// ---------------------------------------------------------------------------
// repo::RepoContext::resolve_rev
// ---------------------------------------------------------------------------

#[test]
fn resolve_rev_head() {
    let (tmp, _repo, _oids) = make_repo(1);
    let ctx = open_ctx(&tmp);
    let sha = ctx.resolve_rev("HEAD").unwrap();
    assert_eq!(sha.len(), 40);
    assert!(sha.chars().all(|c| c.is_ascii_hexdigit()));
}

#[test]
fn resolve_rev_full_sha() {
    let (tmp, _repo, oids) = make_repo(2);
    let ctx = open_ctx(&tmp);
    let full = oids[0].to_string();
    assert_eq!(ctx.resolve_rev(&full).unwrap(), full);
}

#[test]
fn resolve_rev_invalid_ref() {
    let (tmp, _repo, _oids) = make_repo(1);
    let ctx = open_ctx(&tmp);
    assert!(ctx.resolve_rev("no-such-branch").is_err());
}

// ---------------------------------------------------------------------------
// candidates::build
// ---------------------------------------------------------------------------

#[test]
fn build_candidates_linear_history() {
    let (_tmp, repo, oids) = make_repo(5);
    // old = oids[1], new = oids[4]; oids[0] is hidden (it's commit1's parent)
    let st = make_state_with_bounds(&oids[1].to_string(), &oids[4].to_string());
    let list = candidates::build(&repo, &st).unwrap();
    assert_eq!(list.len(), 4, "expected commits 1..=4");
    assert_eq!(list[0].0, oids[1], "oldest-first ordering");
    assert_eq!(list[3].0, oids[4], "newest last");
}

// ---------------------------------------------------------------------------
// candidates::checkout
// ---------------------------------------------------------------------------

#[test]
fn checkout_detaches_head() {
    let (_tmp, repo, oids) = make_repo(3);
    candidates::checkout(&repo, oids[0]).unwrap();
    assert!(repo.head_detached().unwrap());
    assert_eq!(repo.head().unwrap().target().unwrap(), oids[0]);
}

// ---------------------------------------------------------------------------
// commands::session::advance
// ---------------------------------------------------------------------------

#[test]
fn advance_prompt_when_no_bounds() {
    let (_tmp, repo, _oids) = make_repo(1);
    let st = empty_state();
    let prompt = session::advance(&repo, &st).unwrap();
    // The prompt should mention both old and new since neither is set
    assert!(prompt.contains("old") && prompt.contains("new"));
}

#[test]
fn advance_rejects_non_ancestor_new() {
    let (_tmp, repo, oids) = make_repo(4);
    // Intentionally reversed: old is newer than new → ancestry check must fail
    let st = make_state_with_bounds(&oids[3].to_string(), &oids[1].to_string());
    let err = session::advance(&repo, &st).unwrap_err();
    assert!(matches!(err, git_psect::error::Error::Validation(_)));
}

#[test]
fn advance_checks_out_next_commit() {
    let (_tmp, repo, oids) = make_repo(5);
    let st = make_state_with_bounds(&oids[0].to_string(), &oids[4].to_string());
    let prompt = session::advance(&repo, &st).unwrap();
    assert!(repo.head_detached().unwrap(), "HEAD should be detached after advance");
    assert!(prompt.contains("Checking out"), "prompt should name the checked-out commit");
}
