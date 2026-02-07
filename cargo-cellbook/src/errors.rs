pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("Build failed: {0}")]
    Build(String),
    #[error("Library load error: {0}")]
    LibLoad(String),
    #[error("No Cargo.toml found in current directory")]
    NoCargoToml,
    #[error("Watch error: {0}")]
    Watch(String),
}
