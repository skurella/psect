use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("{0}")]
    Validation(String),

    #[error("git error: {0}")]
    Git(#[from] git2::Error),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("failed to parse state: {0}")]
    TomlDe(#[from] toml::de::Error),

    #[error("failed to write state: {0}")]
    TomlSer(#[from] toml::ser::Error),
}
