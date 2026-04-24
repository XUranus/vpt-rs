pub mod vss;

use super::StubBackend;
use crate::snapshot::SnapshotProvider;
use crate::types::Capability;

const CAPABILITIES: &[Capability] = &[
    Capability::CrashConsistentSnapshot,
    Capability::ApplicationConsistentSnapshot,
    Capability::ReadOnlySnapshotMount,
    Capability::BlockLevelBackup,
    Capability::BlockLevelRestore,
    Capability::DirectDeviceAccess,
];

#[derive(Debug, Clone)]
pub struct WindowsBackend(StubBackend);

impl WindowsBackend {
    pub fn new() -> Self {
        Self(StubBackend::new(vss::BACKEND_NAME, CAPABILITIES))
    }

    pub fn backend_name(&self) -> &'static str {
        self.0.backend_name()
    }
}

impl Default for WindowsBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl crate::snapshot::SnapshotProvider for WindowsBackend {
    fn backend_name(&self) -> &'static str {
        self.0.backend_name()
    }

    fn capabilities(&self) -> &'static [Capability] {
        self.0.capabilities()
    }

    fn create_snapshot(
        &self,
        request: &crate::types::SnapshotRequest,
    ) -> crate::error::Result<crate::types::SnapshotInfo> {
        vss::VssSnapshotProvider::new().create_snapshot(request)
    }

    fn delete_snapshot(&self, snapshot: &crate::types::SnapshotHandle) -> crate::error::Result<()> {
        vss::VssSnapshotProvider::new().delete_snapshot(snapshot)
    }

    fn list_snapshots(
        &self,
        source: &crate::types::VolumeRef,
    ) -> crate::error::Result<Vec<crate::types::SnapshotInfo>> {
        vss::VssSnapshotProvider::new().list_snapshots(source)
    }
}

impl crate::backup::BlockDeviceCopier for WindowsBackend {
    fn backend_name(&self) -> &'static str {
        self.0.backend_name()
    }

    fn capabilities(&self) -> &'static [Capability] {
        self.0.capabilities()
    }

    fn backup_volume(&self, plan: &crate::types::BackupPlan) -> crate::error::Result<()> {
        self.0.backup_volume(plan)
    }
}

impl crate::restore::RestorePlanner for WindowsBackend {
    fn backend_name(&self) -> &'static str {
        self.0.backend_name()
    }

    fn capabilities(&self) -> &'static [Capability] {
        self.0.capabilities()
    }

    fn restore_volume(&self, plan: &crate::types::RestorePlan) -> crate::error::Result<()> {
        self.0.restore_volume(plan)
    }
}

impl crate::mount::MountManager for WindowsBackend {
    fn backend_name(&self) -> &'static str {
        self.0.backend_name()
    }

    fn capabilities(&self) -> &'static [Capability] {
        self.0.capabilities()
    }

    fn mount_snapshot(
        &self,
        request: &crate::types::MountRequest,
    ) -> crate::error::Result<crate::types::MountHandle> {
        self.0.mount_snapshot(request)
    }

    fn unmount(&self, handle: &crate::types::MountHandle) -> crate::error::Result<()> {
        self.0.unmount(handle)
    }
}
