use crate::{
    error::Error,
    repo,
    state::{self, Meta, State},
};

pub fn run() -> Result<(), Error> {
    let ctx = repo::open()?;
    let state_path = ctx.state_dir.join("state.toml");
    if state_path.exists() {
        return Err(Error::Validation(
            "session already exists — run 'git psect reset' first".into(),
        ));
    }

    let state = State {
        meta: Meta {
            tool_version: env!("CARGO_PKG_VERSION").into(),
            started_at: chrono::Utc::now().to_rfc3339(),
        },
        priors: Default::default(),
        old_revisions: vec![],
        new_revisions: vec![],
        samples: vec![],
    };

    state::write(&ctx.state_dir, &state)?;
    println!("Session initialized.");
    super::session::advance(&ctx.repo, &state)?;
    Ok(())
}
