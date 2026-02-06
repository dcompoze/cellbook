pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Context(#[from] ContextError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum ContextError {
    #[error("context variable '{0}' not found")]
    NotFound(String),
    #[error("context variable '{key}' type mismatch: expected {expected}, found {actual}")]
    TypeMismatch {
        key: String,
        expected: &'static str,
        actual: &'static str,
    },
}
