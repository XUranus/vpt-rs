use crate::backend::Backend;
use crate::error::Result;
use crate::types::{SnapshotHandle, SnapshotInfo, SnapshotRequest, VolumeRef};

/// Snapshot lifecycle management: create, delete, and list provider-managed snapshots.
///
/// Each platform backend implements this trait for its native snapshot mechanism
/// (e.g. Btrfs subvolume snapshots, LVM snapshots, ZFS snapshots, Windows VSS).
///
/// # Errors
///
/// All methods return [`crate::Error`] on failure. Common errors include:
/// - [`Error::UnsupportedOperation`](crate::Error::UnsupportedOperation) if the backend
///   does not implement snapshots.
/// - [`Error::MissingCapability`](crate::Error::MissingCapability) if the requested
///   snapshot kind is not supported (e.g. application-consistent on Btrfs).
/// - [`Error::InvalidVolume`](crate::Error::InvalidVolume) if the volume reference is empty.
/// - [`Error::MissingPath`](crate::Error::MissingPath) if the source path does not exist.
/// - [`Error::CommandFailed`](crate::Error::CommandFailed) if the underlying tool fails.
pub trait SnapshotProvider: Backend {
    /// Create a new snapshot of the given volume.
    ///
    /// Returns a [`SnapshotInfo`] containing the snapshot handle and metadata.
    fn create_snapshot(&self, request: &SnapshotRequest) -> Result<SnapshotInfo>;

    /// Delete an existing snapshot by its handle.
    fn delete_snapshot(&self, snapshot: &SnapshotHandle) -> Result<()>;

    /// List all snapshots managed by this backend for the given volume.
    fn list_snapshots(&self, source: &VolumeRef) -> Result<Vec<SnapshotInfo>>;
}
