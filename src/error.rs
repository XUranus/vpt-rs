use std::path::PathBuf;

/// A convenience alias for `std::result::Result<T, Error>`.
pub type Result<T> = std::result::Result<T, Error>;

/// Errors returned by volume backup operations.
///
/// Each variant carries structured context (backend name, operation, path) so
/// callers can present meaningful diagnostics without parsing error strings.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("operation `{operation}` is not supported by backend `{backend}`")]
    UnsupportedOperation {
        operation: &'static str,
        backend: &'static str,
    },

    #[error("capability `{capability}` is not available on backend `{backend}`")]
    MissingCapability {
        capability: &'static str,
        backend: &'static str,
    },

    #[error("invalid volume reference `{volume}`")]
    InvalidVolume { volume: String },

    #[error("path does not exist: {path}")]
    MissingPath { path: PathBuf },

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("invalid argument: {message}")]
    InvalidArgument { message: String },

    #[error("command `{command}` failed with status {status}: {stderr}")]
    CommandFailed {
        command: String,
        status: i32,
        stderr: String,
    },

    #[error("operation `{operation}` on backend `{backend}` timed out after {timeout_secs}s")]
    Timeout {
        operation: &'static str,
        backend: &'static str,
        timeout_secs: u64,
    },

    #[error("{message}")]
    Message { message: String },
}

impl Error {
    pub fn timeout_secs(&self) -> Option<u64> {
        match self {
            Self::Timeout { timeout_secs, .. } => Some(*timeout_secs),
            _ => None,
        }
    }
}
