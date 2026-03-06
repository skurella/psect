use crate::{error::Error, repo};

pub fn run() -> Result<(), Error> {
    let ctx = repo::open()?;
    let state_path = ctx.state_dir.join("state.toml");

    if !state_path.exists() {
        println!("No active session.");
        return Ok(());
    }

    std::fs::remove_file(&state_path)?;

    // Remove the directory if it's now empty.
    let _ = std::fs::remove_dir(&ctx.state_dir);

    println!("Session cleared.");
    Ok(())
}
