use crate::backend::Backend;
use crate::error::Result;
use crate::types::BackupPlan;

/// Backup/export a volume to a stream or image file.
///
/// Implementations may use stream-based send (Btrfs `send`, ZFS `send`) or
/// block-level copy (LVM `dd`-style, VSS snapshot + copy). The trait name
/// reflects the execution role, not the underlying mechanism.
///
/// # Errors
///
/// - [`Error::UnsupportedOperation`](crate::Error::UnsupportedOperation) if the backend
///   does not support backup.
/// - [`Error::InvalidArgument`](crate::Error::InvalidArgument) if the target type is
///   not supported (e.g. device target for stream-based backends).
/// - [`Error::CommandFailed`](crate::Error::CommandFailed) if the underlying tool fails.
/// - [`Error::Io`](crate::Error::Io) if file I/O fails during block copy.
pub trait BackupExecutor: Backend {
    /// Execute a backup according to the given plan.
    fn backup_volume(&self, plan: &BackupPlan) -> Result<()>;
}
