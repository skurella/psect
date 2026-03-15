use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::Error;

#[derive(Debug, Serialize, Deserialize)]
pub struct State {
    pub meta: Meta,
    pub priors: Priors,
    #[serde(default)]
    pub old_revisions: Vec<String>,
    #[serde(default)]
    pub new_revisions: Vec<String>,
    #[serde(default)]
    pub samples: Vec<Sample>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Meta {
    pub tool_version: String,
    pub started_at: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Priors {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub old_pass_rate: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub new_pass_rate: Option<f64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Sample {
    pub revision: String,
    pub outcome: bool,
    pub recorded_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
}

pub fn read(state_dir: &Path) -> Result<State, Error> {
    let path = state_dir.join("state.toml");
    if !path.exists() {
        return Err(Error::Validation(
            "no psect session — run 'git psect start' first".into(),
        ));
    }
    let contents = std::fs::read_to_string(path)?;
    Ok(toml::from_str(&contents)?)
}

pub fn write(state_dir: &Path, state: &State) -> Result<(), Error> {
    std::fs::create_dir_all(state_dir)?;
    let tmp = state_dir.join("state.toml.tmp");
    std::fs::write(&tmp, toml::to_string_pretty(state)?)?;
    std::fs::rename(&tmp, state_dir.join("state.toml"))?;
    Ok(())
}
