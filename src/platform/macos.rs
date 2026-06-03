use super::StubBackend;
use crate::types::Capability;

const CAPABILITIES: &[Capability] = &[
    Capability::CrashConsistentSnapshot,
    Capability::BlockLevelBackup,
    Capability::BlockLevelRestore,
    Capability::DirectDeviceAccess,
];

#[derive(Debug, Clone)]
pub struct MacOsBackend(StubBackend);

impl MacOsBackend {
    pub fn new() -> Self {
        Self(StubBackend::new("macos-apfs", CAPABILITIES))
    }

    pub fn backend_name(&self) -> &'static str {
        self.0.backend_name()
    }
}

impl Default for MacOsBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl crate::snapshot::SnapshotProvider for MacOsBackend {
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
        self.0.create_snapshot(request)
    }

    fn delete_snapshot(&self, snapshot: &crate::types::SnapshotHandle) -> crate::error::Result<()> {
        self.0.delete_snapshot(snapshot)
    }

    fn list_snapshots(
        &self,
        source: &crate::types::VolumeRef,
    ) -> crate::error::Result<Vec<crate::types::SnapshotInfo>> {
        self.0.list_snapshots(source)
    }
}

impl crate::backup::BlockDeviceCopier for MacOsBackend {
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

impl crate::restore::RestorePlanner for MacOsBackend {
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

impl crate::mount::MountManager for MacOsBackend {
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
