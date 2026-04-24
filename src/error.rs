use std::path::PathBuf;

pub type Result<T> = std::result::Result<T, Error>;

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

    #[error("io error: {message}")]
    Io { message: String },

    #[error("invalid argument: {message}")]
    InvalidArgument { message: String },

    #[error("command `{command}` failed with status {status}: {stderr}")]
    CommandFailed {
        command: String,
        status: i32,
        stderr: String,
    },

    #[error("{message}")]
    Message { message: String },
}

impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Self {
        Self::Io {
            message: error.to_string(),
        }
    }
}
