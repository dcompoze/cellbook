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
    #[error("type mismatch for '{key}': expected '{expected}', found '{found}'")]
    TypeMismatch {
        key: String,
        expected: String,
        found: String,
    },
    #[error("failed to serialize '{key}': {message}")]
    Serialization { key: String, message: String },
    #[error("failed to deserialize '{key}': {message}")]
    Deserialization { key: String, message: String },
}
