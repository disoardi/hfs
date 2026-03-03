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

    #[error("NameNode unavailable: {0}")]
    NameNodeUnavailable(String),

    #[error("Operation not supported by this backend: {0}")]
    NotSupported(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

/// Map an hdfs-native error to HfsError.
pub fn map_native_error(e: hdfs_native::HdfsError, path: &str) -> HfsError {
    match e {
        hdfs_native::HdfsError::FileNotFound(_) => HfsError::NotFound(path.to_string()),
        hdfs_native::HdfsError::AlreadyExists(_) => {
            HfsError::Hdfs(format!("already exists: {}", path))
        }
        hdfs_native::HdfsError::SASLError(msg) => {
            HfsError::Auth(msg_or_default(msg, "SASL authentication failed"))
        }
        hdfs_native::HdfsError::NoSASLMechanism => {
            HfsError::Auth("no valid SASL mechanism".to_string())
        }
        hdfs_native::HdfsError::RPCError(cls, msg)
        | hdfs_native::HdfsError::FatalRPCError(cls, msg) => {
            if cls.contains("AccessControlException") {
                HfsError::Permission(path.to_string())
            } else if cls.contains("StandbyException") || cls.contains("RetriableException") {
                HfsError::NameNodeUnavailable(msg)
            } else {
                HfsError::Hdfs(format!("{}: {}", cls, msg))
            }
        }
        other => HfsError::Connection(other.to_string()),
    }
}

fn msg_or_default(msg: String, default: &str) -> String {
    if msg.is_empty() {
        default.to_string()
    } else {
        msg
    }
}
