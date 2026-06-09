use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("database error: {0}")]
    Database(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("session not found: {0}")]
    SessionNotFound(String),
    #[error("group not found: {0}")]
    GroupNotFound(i64),
    #[error("scan error: {0}")]
    Scan(String),
    #[error("image error: {0}")]
    Image(String),
    #[error("invalid configuration: {0}")]
    Config(String),
    #[error("{0}")]
    Other(String),
}

pub type AppResult<T> = Result<T, AppError>;
