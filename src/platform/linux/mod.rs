mod btrfs;
mod lvm;
mod zfs;

use crate::backend::Backend;
use crate::backup::BackupExecutor;
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

/// Helper macro: delegate a trait method call to the inner backend variant.
macro_rules! delegate {
    ($self:expr, $method:ident $(, $arg:expr)*) => {
        match $self {
            Self::Btrfs(inner) => inner.$method($($arg),*),
            Self::Lvm(inner) => inner.$method($($arg),*),
            Self::Zfs(inner) => inner.$method($($arg),*),
        }
    };
}

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
            capabilities: self.capabilities(),
        }
    }
}

impl Default for LinuxBackend {
    fn default() -> Self {
        Self::named(DEFAULT_PROVIDER).expect("default linux backend must be valid")
    }
}

impl Backend for LinuxBackend {
    fn backend_name(&self) -> &'static str {
        delegate!(self, backend_name)
    }

    fn capabilities(&self) -> &'static [Capability] {
        delegate!(self, capabilities)
    }
}

impl SnapshotProvider for LinuxBackend {
    fn create_snapshot(&self, request: &SnapshotRequest) -> Result<SnapshotInfo> {
        delegate!(self, create_snapshot, request)
    }

    fn delete_snapshot(&self, snapshot: &SnapshotHandle) -> Result<()> {
        delegate!(self, delete_snapshot, snapshot)
    }

    fn list_snapshots(&self, source: &VolumeRef) -> Result<Vec<SnapshotInfo>> {
        delegate!(self, list_snapshots, source)
    }
}

impl BackupExecutor for LinuxBackend {
    fn backup_volume(&self, plan: &BackupPlan) -> Result<()> {
        delegate!(self, backup_volume, plan)
    }
}

impl RestorePlanner for LinuxBackend {
    fn restore_volume(&self, plan: &RestorePlan) -> Result<()> {
        delegate!(self, restore_volume, plan)
    }
}

impl MountManager for LinuxBackend {
    fn mount_snapshot(&self, request: &MountRequest) -> Result<MountHandle> {
        delegate!(self, mount_snapshot, request)
    }

    fn unmount(&self, handle: &MountHandle) -> Result<()> {
        delegate!(self, unmount, handle)
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
