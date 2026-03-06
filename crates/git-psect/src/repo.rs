use std::{env, path::PathBuf};

use git2::Repository;

use crate::error::Error;

pub struct RepoContext {
    pub repo: Repository,
    pub state_dir: PathBuf,
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
