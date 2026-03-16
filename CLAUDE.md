# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this project is

**psect** is a probabilistic alternative to `git bisect` for finding regressions that introduce flakiness. It models test outcomes with Bernoulli distributions and uses entropy minimization (maximum information gain) to select the next revision to test. See `README.md` for the full mathematical model.

## Commands

```bash
# Build
cargo build
cargo build --release

# Test (all tests)
cargo test

# Run a single test by name
cargo test converges_on_known_regression

# Install binary locally
cargo install --path crates/git-psect

# Lint
cargo clippy

# Format
cargo fmt
```

## Architecture

Two-crate workspace:

**`crates/psect-core`** — pure math, no git dependency. The core loop is:
1. `RegressionProbabilities::initialize()` — uniform prior over candidate revisions
2. `RegressionProbabilities::update_with_sample()` — Bayesian update per test result
3. `next_revision_to_test()` — picks revision minimizing expected posterior entropy

**`crates/git-psect`** — binary CLI. Key data flow per command invocation:
1. `repo.rs`: open git repo, resolve state dir to `$GIT_DIR/psect/`. `RepoContext::resolve_rev()` shells out to `git rev-parse --verify` to handle all revspecs libgit2 doesn't support (`@`, `@{upstream}`, `@{push}`, etc.).
2. `state.rs`: deserialize `state.toml` (TOML, atomic writes via tmp→rename)
3. `candidates.rs`: revwalk (new→old, oldest-first topological) to build candidate list, then **replay** all samples through `initialize` + `update_with_sample` to reconstruct current probabilities
4. Command logic in `commands/`: mutate state in memory, validate via `session::advance()`, then write and log only on success

Probabilities are **never persisted** — always replayed from the `[[samples]]` array on each invocation. This keeps state.toml as the single source of truth and ensures floating-point consistency across versions.

### `commands/session.rs`

Central coordination module. Two public functions:
- `mark(ctx, state, rev, bound)` — resolves rev, mutates state in memory, calls `advance` to validate, then writes state and prints output only on success. Never logs before validation passes.
- `advance(repo, state)` — validates ancestry and either prompts for missing bounds or checks out the next revision. Called by `pass_fail` and `start` too.

Priors (`old_pass_rate`, `new_pass_rate`) default to 0.95/0.5 and are materialized into state on first use via `get_or_insert`. Can be changed independently with `git psect set-prior <old|new> <rate>`.

## Key design decisions

- `RegressionProbabilities` uses struct-of-arrays (`revisions: &Vec<R>` + `ps: Vec<f64>`) for cache locality in the hot probability update loop.
- State lives at `$GIT_DIR/psect/state.toml` (per-worktree, like `git bisect`) to support parallel independent sessions.
- The `Revision` trait (`Debug + Eq + Hash + PartialOrd`) abstracts over git OIDs for testability — tests use a simple `TestRev` newtype.
- `build()` in `candidates.rs` pushes `new_revisions` and hides parents of `old_revisions` so the revwalk includes the old revision itself but stops before its ancestors.
- Avoid "good/bad" terminology — use "old/new" or "pre-/post-regression" throughout.
