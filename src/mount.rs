use crate::backend::Backend;
use crate::error::Result;
use crate::types::{MountHandle, MountRequest};

/// Mount and unmount snapshots for browsing or copy-mount workflows.
///
/// # Errors
///
/// - [`Error::UnsupportedOperation`](crate::Error::UnsupportedOperation) if the backend
///   does not support mounting.
pub trait MountManager: Backend {
    /// Mount an existing snapshot at the requested location.
    fn mount_snapshot(&self, request: &MountRequest) -> Result<MountHandle>;

    /// Unmount a previously mounted snapshot.
    fn unmount(&self, handle: &MountHandle) -> Result<()>;
}
