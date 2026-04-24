use crate::error::Result;
use crate::types::{Capability, SnapshotHandle, SnapshotInfo, SnapshotRequest, VolumeRef};

pub trait SnapshotProvider: Send + Sync {
    fn backend_name(&self) -> &'static str;
    fn capabilities(&self) -> &'static [Capability];
    fn supports(&self, capability: Capability) -> bool {
        self.capabilities().contains(&capability)
    }
    fn create_snapshot(&self, request: &SnapshotRequest) -> Result<SnapshotInfo>;
    fn delete_snapshot(&self, snapshot: &SnapshotHandle) -> Result<()>;
    fn list_snapshots(&self, source: &VolumeRef) -> Result<Vec<SnapshotInfo>>;
}
