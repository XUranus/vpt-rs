use crate::error::Result;
use crate::types::{SnapshotHandle, SnapshotInfo};

use super::{BACKEND_NAME, SnapshotContext, VssSnapshotSpec, VssTimeouts, ffi};

#[derive(Debug, Clone)]
pub struct VssSession {
    spec: VssSnapshotSpec,
    timeouts: VssTimeouts,
    raw: ffi::RawSnapshotSet,
}

impl VssSession {
    pub(crate) fn new(
        spec: VssSnapshotSpec,
        timeouts: VssTimeouts,
        raw: ffi::RawSnapshotSet,
    ) -> Self {
        Self {
            spec,
            timeouts,
            raw,
        }
    }

    pub fn snapshot_set_id(&self) -> &str {
        &self.raw.snapshot_set_id
    }

    pub fn context(&self) -> SnapshotContext {
        self.spec.context
    }

    pub fn create_snapshot(self) -> Result<SnapshotInfo> {
        ffi::commit_snapshot_set(&self.raw, self.timeouts)?;

        Ok(SnapshotInfo {
            handle: SnapshotHandle {
                id: self.raw.snapshot_id,
                source: self.spec.request.source,
            },
            backend: BACKEND_NAME,
            path_hint: None,
            read_only: self.spec.request.read_only,
        })
    }

    pub fn abort(self) -> Result<()> {
        ffi::abort_snapshot_set(&self.raw)
    }
}
