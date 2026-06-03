use crate::backend::Backend;
use crate::error::Result;
use crate::types::RestorePlan;

/// Restore/import a volume from a stream or image file.
///
/// Implementations may use stream-based receive (Btrfs `receive`, ZFS `receive`) or
/// block-level write (LVM, VSS). Destructive backends (LVM, VSS) require `--force`
/// in the plan.
///
/// # Errors
///
/// - [`Error::UnsupportedOperation`](crate::Error::UnsupportedOperation) if the backend
///   does not support restore.
/// - [`Error::InvalidArgument`](crate::Error::InvalidArgument) if `force` is required
///   but not set, or if the source type is unsupported.
/// - [`Error::MissingPath`](crate::Error::MissingPath) if the source file does not exist.
/// - [`Error::CommandFailed`](crate::Error::CommandFailed) if the underlying tool fails.
pub trait RestorePlanner: Backend {
    /// Execute a restore according to the given plan.
    fn restore_volume(&self, plan: &RestorePlan) -> Result<()>;
}
