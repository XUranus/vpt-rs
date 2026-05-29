use crate::error::Result;
use crate::types::{SnapshotHandle, SnapshotInfo};

use super::{BACKEND_NAME, SnapshotContext, VssSnapshotSpec, VssTimeouts, ffi};

#[derive(Debug)]
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

    pub fn create_snapshot(mut self) -> Result<SnapshotInfo> {
        ffi::commit_snapshot_set(&mut self.raw, self.timeouts)?;

        let path_hint = if !self.raw.device_path.is_empty() {
            Some(std::path::PathBuf::from(&self.raw.device_path))
        } else {
            None
        };

        Ok(SnapshotInfo {
            handle: SnapshotHandle {
                id: self.raw.snapshot_id.clone(),
                source: self.spec.request.source,
            },
            backend: BACKEND_NAME,
            path_hint,
            read_only: self.spec.request.read_only,
        })
    }

    pub fn abort(mut self) -> Result<()> {
        ffi::abort_snapshot_set(&mut self.raw)
    }
}
