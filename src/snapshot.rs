use crate::backend::Backend;
use crate::error::Result;
use crate::types::{SnapshotHandle, SnapshotInfo, SnapshotRequest, VolumeRef};

pub trait SnapshotProvider: Backend {
    fn create_snapshot(&self, request: &SnapshotRequest) -> Result<SnapshotInfo>;
    fn delete_snapshot(&self, snapshot: &SnapshotHandle) -> Result<()>;
    fn list_snapshots(&self, source: &VolumeRef) -> Result<Vec<SnapshotInfo>>;
}
