mod btrfs;
mod lvm;
mod zfs;

use crate::backup::BlockDeviceCopier;
use crate::error::{Error, Result};
use crate::mount::MountManager;
use crate::platform::BackendDescriptor;
use crate::restore::RestorePlanner;
use crate::snapshot::SnapshotProvider;
use crate::types::{
    BackupPlan, Capability, MountHandle, MountRequest, RestorePlan, SnapshotHandle, SnapshotInfo,
    SnapshotRequest, VolumeRef,
};

pub use btrfs::BtrfsBackend;
pub use lvm::LvmBackend;
pub use zfs::ZfsBackend;

pub const DEFAULT_PROVIDER: &str = "btrfs";

#[derive(Debug, Clone)]
pub enum LinuxBackend {
    Btrfs(BtrfsBackend),
    Lvm(LvmBackend),
    Zfs(ZfsBackend),
}

impl LinuxBackend {
    pub fn named(name: &str) -> Result<Self> {
        match name {
            "btrfs" => Ok(Self::Btrfs(BtrfsBackend::new())),
            "lvm" => Ok(Self::Lvm(LvmBackend::new())),
            "zfs" => Ok(Self::Zfs(ZfsBackend::new())),
            _ => Err(Error::InvalidArgument {
                message: format!("unknown linux snapshot provider `{name}`"),
            }),
        }
    }

    pub fn available() -> [Self; 3] {
        [
            Self::Btrfs(BtrfsBackend::new()),
            Self::Lvm(LvmBackend::new()),
            Self::Zfs(ZfsBackend::new()),
        ]
    }

    pub fn backend_name(&self) -> &'static str {
        match self {
            Self::Btrfs(backend) => backend.backend_name(),
            Self::Lvm(backend) => backend.backend_name(),
            Self::Zfs(backend) => backend.backend_name(),
        }
    }

    pub fn provider_name(&self) -> &'static str {
        match self {
            Self::Btrfs(_) => "btrfs",
            Self::Lvm(_) => "lvm",
            Self::Zfs(_) => "zfs",
        }
    }

    pub fn descriptor(&self) -> BackendDescriptor {
        BackendDescriptor {
            platform: std::env::consts::OS,
            provider_name: Some(self.provider_name()),
            backend_name: self.backend_name(),
            capabilities: SnapshotProvider::capabilities(self),
        }
    }
}

impl Default for LinuxBackend {
    fn default() -> Self {
        Self::named(DEFAULT_PROVIDER).expect("default linux backend must be valid")
    }
}

impl SnapshotProvider for LinuxBackend {
    fn backend_name(&self) -> &'static str {
        self.backend_name()
    }

    fn capabilities(&self) -> &'static [Capability] {
        match self {
            Self::Btrfs(backend) => SnapshotProvider::capabilities(backend),
            Self::Lvm(backend) => SnapshotProvider::capabilities(backend),
            Self::Zfs(backend) => SnapshotProvider::capabilities(backend),
        }
    }

    fn create_snapshot(&self, request: &SnapshotRequest) -> Result<SnapshotInfo> {
        match self {
            Self::Btrfs(backend) => backend.create_snapshot(request),
            Self::Lvm(backend) => backend.create_snapshot(request),
            Self::Zfs(backend) => backend.create_snapshot(request),
        }
    }

    fn delete_snapshot(&self, snapshot: &SnapshotHandle) -> Result<()> {
        match self {
            Self::Btrfs(backend) => backend.delete_snapshot(snapshot),
            Self::Lvm(backend) => backend.delete_snapshot(snapshot),
            Self::Zfs(backend) => backend.delete_snapshot(snapshot),
        }
    }

    fn list_snapshots(&self, source: &VolumeRef) -> Result<Vec<SnapshotInfo>> {
        match self {
            Self::Btrfs(backend) => backend.list_snapshots(source),
            Self::Lvm(backend) => backend.list_snapshots(source),
            Self::Zfs(backend) => backend.list_snapshots(source),
        }
    }
}

impl BlockDeviceCopier for LinuxBackend {
    fn backend_name(&self) -> &'static str {
        self.backend_name()
    }

    fn capabilities(&self) -> &'static [Capability] {
        SnapshotProvider::capabilities(self)
    }

    fn backup_volume(&self, plan: &BackupPlan) -> Result<()> {
        match self {
            Self::Btrfs(backend) => backend.backup_volume(plan),
            Self::Lvm(backend) => backend.backup_volume(plan),
            Self::Zfs(backend) => backend.backup_volume(plan),
        }
    }
}

impl RestorePlanner for LinuxBackend {
    fn backend_name(&self) -> &'static str {
        self.backend_name()
    }

    fn capabilities(&self) -> &'static [Capability] {
        SnapshotProvider::capabilities(self)
    }

    fn restore_volume(&self, plan: &RestorePlan) -> Result<()> {
        match self {
            Self::Btrfs(backend) => backend.restore_volume(plan),
            Self::Lvm(backend) => backend.restore_volume(plan),
            Self::Zfs(backend) => backend.restore_volume(plan),
        }
    }
}

impl MountManager for LinuxBackend {
    fn backend_name(&self) -> &'static str {
        self.backend_name()
    }

    fn capabilities(&self) -> &'static [Capability] {
        SnapshotProvider::capabilities(self)
    }

    fn mount_snapshot(&self, request: &MountRequest) -> Result<MountHandle> {
        match self {
            Self::Btrfs(backend) => backend.mount_snapshot(request),
            Self::Lvm(backend) => backend.mount_snapshot(request),
            Self::Zfs(backend) => backend.mount_snapshot(request),
        }
    }

    fn unmount(&self, handle: &MountHandle) -> Result<()> {
        match self {
            Self::Btrfs(backend) => backend.unmount(handle),
            Self::Lvm(backend) => backend.unmount(handle),
            Self::Zfs(backend) => backend.unmount(handle),
        }
    }
}

pub fn available_descriptors() -> Vec<BackendDescriptor> {
    LinuxBackend::available()
        .into_iter()
        .map(|backend| backend.descriptor())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_named_linux_backends() {
        let names: Vec<_> = available_descriptors()
            .into_iter()
            .map(|descriptor| descriptor.provider_name.unwrap_or(""))
            .collect();

        assert_eq!(names, vec!["btrfs", "lvm", "zfs"]);
    }
}
