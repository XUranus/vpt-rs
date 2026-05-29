use crate::error::{Error, Result};
use crate::types::{SnapshotHandle, SnapshotInfo, VolumeRef};

use super::{BACKEND_NAME, VssSnapshotSpec, VssTimeouts, ffi};

#[derive(Debug)]
pub struct VssRequestor {
    timeouts: VssTimeouts,
}

impl VssRequestor {
    pub fn initialize(timeouts: VssTimeouts) -> Result<Self> {
        ffi::initialize_requestor()?;
        Ok(Self { timeouts })
    }

    pub fn start_session(&self, spec: VssSnapshotSpec) -> Result<super::session::VssSession> {
        let raw = ffi::create_snapshot_set(&spec, self.timeouts)?;
        Ok(super::session::VssSession::new(spec, self.timeouts, raw))
    }

    pub fn delete_snapshot(&self, snapshot: &SnapshotHandle) -> Result<()> {
        if snapshot.id.trim().is_empty() {
            return Err(Error::InvalidArgument {
                message: "snapshot id must not be empty".to_string(),
            });
        }

        ffi::delete_snapshot(&snapshot.id)
    }

    pub fn list_snapshots(&self, source: &VolumeRef) -> Result<Vec<SnapshotInfo>> {
        if source.id.trim().is_empty() {
            return Err(Error::InvalidVolume {
                volume: source.id.clone(),
            });
        }

        ffi::list_snapshots(source, BACKEND_NAME)
    }
}
