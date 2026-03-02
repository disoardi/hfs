use thiserror::Error;

#[derive(Error, Debug)]
pub enum HfsError {
    #[error("Connection failed: {0}")]
    Connection(String),

    #[error("Authentication failed: {0}")]
    Auth(String),

    #[error("Path not found: {0}")]
    NotFound(String),

    #[error("Permission denied: {0}")]
    Permission(String),

    #[error("HDFS error: {0}")]
    Hdfs(String),

    #[error("Config error: {0}")]
    Config(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}
