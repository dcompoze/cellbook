pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Any(#[from] Box<dyn std::error::Error + Send + Sync + 'static>),
    #[error(transparent)]
    Database(#[from] duckdb::Error),
    #[error(transparent)]
    Anyhow(#[from] anyhow::Error),
    #[error(transparent)]
    InputOutput(#[from] std::io::Error),
}
