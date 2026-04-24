use crate::backup::BlockDeviceCopier;
use crate::error::Result;
use crate::mount::MountManager;
use crate::platform::StubBackend;
use crate::restore::RestorePlanner;
use crate::snapshot::SnapshotProvider;
use crate::types::{
    BackupPlan, Capability, MountHandle, MountRequest, RestorePlan, SnapshotHandle, SnapshotInfo,
    SnapshotRequest, VolumeRef,
};

const CAPABILITIES: &[Capability] = &[
    Capability::CrashConsistentSnapshot,
    Capability::ReadOnlySnapshotMount,
    Capability::WritableSnapshotMount,
    Capability::BlockLevelBackup,
    Capability::BlockLevelRestore,
    Capability::DirectDeviceAccess,
];

#[derive(Debug, Clone)]
pub struct LvmBackend(StubBackend);

impl LvmBackend {
    pub fn new() -> Self {
        Self(StubBackend::new("linux-lvm", CAPABILITIES))
    }

    pub fn backend_name(&self) -> &'static str {
        self.0.backend_name()
    }
}

impl SnapshotProvider for LvmBackend {
    fn backend_name(&self) -> &'static str {
        self.0.backend_name()
    }

    fn capabilities(&self) -> &'static [Capability] {
        self.0.capabilities()
    }

    fn create_snapshot(&self, request: &SnapshotRequest) -> Result<SnapshotInfo> {
        self.0.create_snapshot(request)
    }

    fn delete_snapshot(&self, snapshot: &SnapshotHandle) -> Result<()> {
        self.0.delete_snapshot(snapshot)
    }

    fn list_snapshots(&self, source: &VolumeRef) -> Result<Vec<SnapshotInfo>> {
        self.0.list_snapshots(source)
    }
}

impl BlockDeviceCopier for LvmBackend {
    fn backend_name(&self) -> &'static str {
        self.0.backend_name()
    }

    fn capabilities(&self) -> &'static [Capability] {
        self.0.capabilities()
    }

    fn backup_volume(&self, plan: &BackupPlan) -> Result<()> {
        self.0.backup_volume(plan)
    }
}

impl RestorePlanner for LvmBackend {
    fn backend_name(&self) -> &'static str {
        self.0.backend_name()
    }

    fn capabilities(&self) -> &'static [Capability] {
        self.0.capabilities()
    }

    fn restore_volume(&self, plan: &RestorePlan) -> Result<()> {
        self.0.restore_volume(plan)
    }
}

impl MountManager for LvmBackend {
    fn backend_name(&self) -> &'static str {
        self.0.backend_name()
    }

    fn capabilities(&self) -> &'static [Capability] {
        self.0.capabilities()
    }

    fn mount_snapshot(&self, request: &MountRequest) -> Result<MountHandle> {
        self.0.mount_snapshot(request)
    }

    fn unmount(&self, handle: &MountHandle) -> Result<()> {
        self.0.unmount(handle)
    }
}
