pub mod ffi;
pub mod requestor;
pub mod session;

use crate::error::{Error, Result};
use crate::snapshot::SnapshotProvider;
use crate::types::{
    Capability, SnapshotHandle, SnapshotInfo, SnapshotKind, SnapshotRequest, VolumeRef,
};

pub const BACKEND_NAME: &str = "windows-vss";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WriterCoordination {
    Disabled,
    Enabled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapshotContext {
    Backup,
    FileShareBackup,
    ClientAccessible,
}

impl SnapshotContext {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Backup => "backup",
            Self::FileShareBackup => "file-share-backup",
            Self::ClientAccessible => "client-accessible",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VssTimeouts {
    pub gather_writer_metadata_ms: u32,
    pub prepare_for_backup_ms: u32,
    pub do_snapshot_set_ms: u32,
}

impl Default for VssTimeouts {
    fn default() -> Self {
        Self {
            gather_writer_metadata_ms: 15_000,
            prepare_for_backup_ms: 60_000,
            do_snapshot_set_ms: 60_000,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VssSnapshotSpec {
    pub request: SnapshotRequest,
    pub context: SnapshotContext,
    pub transportable: bool,
    pub auto_release: bool,
}

impl VssSnapshotSpec {
    pub fn new(request: SnapshotRequest) -> Self {
        Self {
            request,
            context: SnapshotContext::Backup,
            transportable: false,
            auto_release: true,
        }
    }

    pub fn with_context(mut self, context: SnapshotContext) -> Self {
        self.context = context;
        self
    }

    pub fn transportable(mut self, enabled: bool) -> Self {
        self.transportable = enabled;
        self
    }

    pub fn auto_release(mut self, enabled: bool) -> Self {
        self.auto_release = enabled;
        self
    }
}

#[derive(Debug, Clone)]
pub struct VssSnapshotProvider {
    writer_coordination: WriterCoordination,
    context: SnapshotContext,
    timeouts: VssTimeouts,
}

impl VssSnapshotProvider {
    pub fn new() -> Self {
        Self {
            writer_coordination: WriterCoordination::Enabled,
            context: SnapshotContext::Backup,
            timeouts: VssTimeouts::default(),
        }
    }

    pub fn with_writer_coordination(mut self, coordination: WriterCoordination) -> Self {
        self.writer_coordination = coordination;
        self
    }

    pub fn with_context(mut self, context: SnapshotContext) -> Self {
        self.context = context;
        self
    }

    pub fn with_timeouts(mut self, timeouts: VssTimeouts) -> Self {
        self.timeouts = timeouts;
        self
    }

    pub fn writer_coordination(&self) -> WriterCoordination {
        self.writer_coordination
    }

    pub fn context(&self) -> SnapshotContext {
        self.context
    }

    pub fn timeouts(&self) -> VssTimeouts {
        self.timeouts
    }

    pub fn build_spec(&self, request: SnapshotRequest) -> VssSnapshotSpec {
        VssSnapshotSpec::new(request).with_context(self.context)
    }

    pub fn validate_request(&self, request: &SnapshotRequest) -> Result<()> {
        if request.source.id.trim().is_empty() {
            return Err(Error::InvalidVolume {
                volume: request.source.id.clone(),
            });
        }

        if request.source.id.starts_with(r"\\.\") {
            return Err(Error::InvalidArgument {
                message: format!(
                    "VSS expects a volume GUID path or mounted volume path, got `{}`",
                    request.source.id
                ),
            });
        }

        if matches!(
            (self.writer_coordination, request.kind),
            (
                WriterCoordination::Disabled,
                SnapshotKind::ApplicationConsistent
            )
        ) {
            return Err(Error::MissingCapability {
                capability: Capability::ApplicationConsistentSnapshot.as_str(),
                backend: BACKEND_NAME,
            });
        }

        Ok(())
    }

    pub fn start_session(&self, spec: VssSnapshotSpec) -> Result<session::VssSession> {
        self.validate_request(&spec.request)?;
        requestor::VssRequestor::initialize(self.timeouts)?.start_session(spec)
    }
}

impl Default for VssSnapshotProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl SnapshotProvider for VssSnapshotProvider {
    fn backend_name(&self) -> &'static str {
        BACKEND_NAME
    }

    fn capabilities(&self) -> &'static [Capability] {
        &[
            Capability::CrashConsistentSnapshot,
            Capability::ApplicationConsistentSnapshot,
            Capability::BlockLevelBackup,
            Capability::BlockLevelRestore,
            Capability::DirectDeviceAccess,
        ]
    }

    fn create_snapshot(&self, request: &SnapshotRequest) -> Result<SnapshotInfo> {
        let spec = self.build_spec(request.clone());
        let session = self.start_session(spec)?;
        session.create_snapshot()
    }

    fn delete_snapshot(&self, snapshot: &SnapshotHandle) -> Result<()> {
        requestor::VssRequestor::initialize(self.timeouts)?.delete_snapshot(snapshot)
    }

    fn list_snapshots(&self, source: &VolumeRef) -> Result<Vec<SnapshotInfo>> {
        requestor::VssRequestor::initialize(self.timeouts)?.list_snapshots(source)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_app_consistent_request_without_writers() {
        let provider =
            VssSnapshotProvider::new().with_writer_coordination(WriterCoordination::Disabled);

        let error = provider
            .validate_request(&SnapshotRequest {
                source: VolumeRef::new(r"\\?\Volume{test}\"),
                kind: SnapshotKind::ApplicationConsistent,
                label: None,
                read_only: true,
            })
            .unwrap_err();

        assert!(matches!(error, Error::MissingCapability { .. }));
    }

    #[test]
    fn rejects_device_paths_as_vss_sources() {
        let provider = VssSnapshotProvider::new();
        let error = provider
            .validate_request(&SnapshotRequest {
                source: VolumeRef::new(r"\\.\PhysicalDrive0"),
                kind: SnapshotKind::CrashConsistent,
                label: None,
                read_only: true,
            })
            .unwrap_err();

        assert!(matches!(error, Error::InvalidArgument { .. }));
    }
}
