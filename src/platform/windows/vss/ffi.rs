use crate::error::{Error, Result};
use crate::types::{SnapshotInfo, VolumeRef};

use super::{BACKEND_NAME, VssSnapshotSpec, VssTimeouts};

#[derive(Debug, Clone)]
pub struct RawSnapshotSet {
    pub snapshot_set_id: String,
    pub snapshot_id: String,
}

pub fn initialize_requestor() -> Result<()> {
    unsupported("initialize_requestor")
}

pub fn create_snapshot_set(
    _spec: &VssSnapshotSpec,
    _timeouts: VssTimeouts,
) -> Result<RawSnapshotSet> {
    unsupported("create_snapshot_set")
}

pub fn commit_snapshot_set(_raw: &RawSnapshotSet, _timeouts: VssTimeouts) -> Result<()> {
    unsupported("commit_snapshot_set")
}

pub fn abort_snapshot_set(_raw: &RawSnapshotSet) -> Result<()> {
    unsupported("abort_snapshot_set")
}

pub fn delete_snapshot(_snapshot_id: &str) -> Result<()> {
    unsupported("delete_snapshot")
}

pub fn list_snapshots(source: &VolumeRef, backend: &'static str) -> Result<Vec<SnapshotInfo>> {
    let _ = source;
    let _ = backend;
    unsupported("list_snapshots")
}

fn unsupported<T>(operation: &'static str) -> Result<T> {
    Err(Error::UnsupportedOperation {
        operation,
        backend: BACKEND_NAME,
    })
}

#[allow(dead_code)]
#[cfg(all(target_os = "windows", feature = "windows-vss"))]
mod windows_bindings {
    use super::*;

    pub fn initialize_requestor() -> Result<()> {
        unsupported("initialize_requestor")
    }

    pub fn create_snapshot_set(
        _spec: &VssSnapshotSpec,
        _timeouts: VssTimeouts,
    ) -> Result<RawSnapshotSet> {
        unsupported("create_snapshot_set")
    }

    pub fn commit_snapshot_set(_raw: &RawSnapshotSet, _timeouts: VssTimeouts) -> Result<()> {
        unsupported("commit_snapshot_set")
    }

    pub fn abort_snapshot_set(_raw: &RawSnapshotSet) -> Result<()> {
        unsupported("abort_snapshot_set")
    }

    pub fn delete_snapshot(_snapshot_id: &str) -> Result<()> {
        unsupported("delete_snapshot")
    }

    pub fn list_snapshots(
        _source: &VolumeRef,
        _backend: &'static str,
    ) -> Result<Vec<SnapshotInfo>> {
        unsupported("list_snapshots")
    }
}
