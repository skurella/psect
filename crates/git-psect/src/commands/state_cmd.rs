use crate::{error::Error, repo};

pub fn run() -> Result<(), Error> {
    let ctx = repo::open()?;
    println!("{}", ctx.state_dir.join("state.toml").display());
    Ok(())
}
