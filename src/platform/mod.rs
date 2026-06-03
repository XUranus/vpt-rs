#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(all(
    target_family = "unix",
    not(target_os = "linux"),
    not(target_os = "macos")
))]
mod unix;
#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "linux")]
pub use linux::LinuxBackend as CurrentBackend;
#[cfg(target_os = "macos")]
pub use macos::MacOsBackend as CurrentBackend;
#[cfg(all(
    target_family = "unix",
    not(target_os = "linux"),
    not(target_os = "macos")
))]
pub use unix::UnixBackend as CurrentBackend;
#[cfg(target_os = "windows")]
pub use windows::WindowsBackend as CurrentBackend;
#[cfg(all(target_os = "windows", feature = "windows-vss"))]
pub use windows::vss::VssSnapshotProvider;

use crate::backend::Backend;
use crate::backup::BackupExecutor;
use crate::error::{Error, Result};
use crate::mount::MountManager;
use crate::restore::RestorePlanner;
use crate::snapshot::SnapshotProvider;
use crate::types::{
    BackupPlan, Capability, MountHandle, MountRequest, RestorePlan, SnapshotHandle, SnapshotInfo,
    SnapshotRequest, VolumeRef,
};
use tracing::error;

pub fn current_platform() -> &'static str {
    std::env::consts::OS
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BackendDescriptor {
    pub platform: &'static str,
    pub provider_name: Option<&'static str>,
    pub backend_name: &'static str,
    pub capabilities: &'static [Capability],
}

pub fn current_backend() -> CurrentBackend {
    CurrentBackend::default()
}

#[cfg(target_os = "linux")]
pub fn current_backend_descriptor() -> BackendDescriptor {
    current_backend().descriptor()
}

#[cfg(not(target_os = "linux"))]
pub fn current_backend_descriptor() -> BackendDescriptor {
    let backend = current_backend();
    BackendDescriptor {
        platform: current_platform(),
        provider_name: None,
        backend_name: backend.backend_name(),
        capabilities: backend.capabilities(),
    }
}

pub fn available_backend_descriptors() -> Vec<BackendDescriptor> {
    #[cfg(target_os = "linux")]
    {
        linux::available_descriptors()
    }
    #[cfg(not(target_os = "linux"))]
    {
        vec![current_backend_descriptor()]
    }
}

fn unsupported(operation: &'static str, backend: &'static str) -> Error {
    Error::UnsupportedOperation { operation, backend }
}

#[derive(Debug, Clone, Default)]
pub struct StubBackend {
    backend_name: &'static str,
    capabilities: &'static [Capability],
}

impl StubBackend {
    pub const fn new(backend_name: &'static str, capabilities: &'static [Capability]) -> Self {
        Self {
            backend_name,
            capabilities,
        }
    }

    pub fn descriptor(&self) -> BackendDescriptor {
        BackendDescriptor {
            platform: current_platform(),
            provider_name: None,
            backend_name: self.backend_name,
            capabilities: self.capabilities,
        }
    }
}

impl Backend for StubBackend {
    fn backend_name(&self) -> &'static str {
        self.backend_name
    }

    fn capabilities(&self) -> &'static [Capability] {
        self.capabilities
    }
}

impl SnapshotProvider for StubBackend {
    fn create_snapshot(&self, _request: &SnapshotRequest) -> Result<SnapshotInfo> {
        let error = unsupported("create_snapshot", self.backend_name);
        error!(backend = self.backend_name, error = %error, "create_snapshot failed");
        Err(error)
    }

    fn delete_snapshot(&self, _snapshot: &SnapshotHandle) -> Result<()> {
        let error = unsupported("delete_snapshot", self.backend_name);
        error!(backend = self.backend_name, error = %error, "delete_snapshot failed");
        Err(error)
    }

    fn list_snapshots(&self, _source: &VolumeRef) -> Result<Vec<SnapshotInfo>> {
        let error = unsupported("list_snapshots", self.backend_name);
        error!(backend = self.backend_name, error = %error, "list_snapshots failed");
        Err(error)
    }
}

impl BackupExecutor for StubBackend {
    fn backup_volume(&self, _plan: &BackupPlan) -> Result<()> {
        let error = unsupported("backup_volume", self.backend_name);
        error!(backend = self.backend_name, error = %error, "backup_volume failed");
        Err(error)
    }
}

impl RestorePlanner for StubBackend {
    fn restore_volume(&self, _plan: &RestorePlan) -> Result<()> {
        let error = unsupported("restore_volume", self.backend_name);
        error!(backend = self.backend_name, error = %error, "restore_volume failed");
        Err(error)
    }
}

impl MountManager for StubBackend {
    fn mount_snapshot(&self, _request: &MountRequest) -> Result<MountHandle> {
        let error = unsupported("mount_snapshot", self.backend_name);
        error!(backend = self.backend_name, error = %error, "mount_snapshot failed");
        Err(error)
    }

    fn unmount(&self, _handle: &MountHandle) -> Result<()> {
        let error = unsupported("unmount", self.backend_name);
        error!(backend = self.backend_name, error = %error, "unmount failed");
        Err(error)
    }
}
