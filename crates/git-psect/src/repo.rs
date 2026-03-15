use std::{env, path::PathBuf, process::Command};

use git2::Repository;

use crate::error::Error;

pub struct RepoContext {
    pub repo: Repository,
    pub state_dir: PathBuf,
}

impl RepoContext {
    /// Resolve any revspec git understands (including `@`, `@{upstream}`, etc.)
    /// to a full 40-character SHA by delegating to `git rev-parse`.
    pub fn resolve_rev(&self, refspec: &str) -> Result<String, Error> {
        let work_dir = self
            .repo
            .workdir()
            .unwrap_or_else(|| self.repo.path())
            .to_path_buf();
        let out = Command::new("git")
            .args(["rev-parse", "--verify", refspec])
            .current_dir(&work_dir)
            .output()?;
        if !out.status.success() {
            let msg = String::from_utf8_lossy(&out.stderr).trim().to_string();
            return Err(Error::Validation(msg));
        }
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
    }
}

pub fn open() -> Result<RepoContext, Error> {
    let (repo, git_dir) = if let Ok(val) = env::var("GIT_DIR") {
        let git_dir = PathBuf::from(&val);
        if !git_dir.join("HEAD").exists() {
            return Err(Error::Validation(format!(
                "GIT_DIR '{val}' does not look like a git directory"
            )));
        }
        let repo = Repository::open(&git_dir)?;
        (repo, git_dir)
    } else {
        let cwd = env::current_dir()?;
        let repo = Repository::discover(&cwd)?;
        let git_dir = repo.path().to_path_buf();
        (repo, git_dir)
    };

    Ok(RepoContext {
        state_dir: git_dir.join("psect"),
        repo,
    })
}
