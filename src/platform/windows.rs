#[cfg(feature = "windows-vss")]
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
        Self(StubBackend::new("windows-vss", CAPABILITIES))
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

// ── SnapshotProvider ───────────────────────────────────────────────────────

#[cfg(feature = "windows-vss")]
impl SnapshotProvider for WindowsBackend {
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

#[cfg(not(feature = "windows-vss"))]
impl SnapshotProvider for WindowsBackend {
    fn backend_name(&self) -> &'static str {
        self.0.backend_name()
    }

    fn capabilities(&self) -> &'static [Capability] {
        self.0.capabilities()
    }

    fn create_snapshot(
        &self,
        _request: &crate::types::SnapshotRequest,
    ) -> crate::error::Result<crate::types::SnapshotInfo> {
        Err(crate::error::Error::UnsupportedOperation {
            operation: "create_snapshot",
            backend: self.backend_name(),
        })
    }

    fn delete_snapshot(&self, _snapshot: &crate::types::SnapshotHandle) -> crate::error::Result<()> {
        Err(crate::error::Error::UnsupportedOperation {
            operation: "delete_snapshot",
            backend: self.backend_name(),
        })
    }

    fn list_snapshots(
        &self,
        _source: &crate::types::VolumeRef,
    ) -> crate::error::Result<Vec<crate::types::SnapshotInfo>> {
        Err(crate::error::Error::UnsupportedOperation {
            operation: "list_snapshots",
            backend: self.backend_name(),
        })
    }
}

// ── BlockDeviceCopier ──────────────────────────────────────────────────────

#[cfg(feature = "windows-vss")]
impl crate::backup::BlockDeviceCopier for WindowsBackend {
    fn backend_name(&self) -> &'static str {
        self.0.backend_name()
    }

    fn capabilities(&self) -> &'static [Capability] {
        self.0.capabilities()
    }

    fn backup_volume(&self, plan: &crate::types::BackupPlan) -> crate::error::Result<()> {
        use tracing::{error, info};

        let source_display = plan.source.to_string();
        info!(backend = self.backend_name(), source = %source_display, "backup_volume called");

        let result = (|| -> crate::error::Result<()> {
            use crate::types::{BackupSource, SnapshotPolicy, SnapshotRequest};

            // Determine copy source: snapshot device path or live volume
            let (copy_src, temp_snapshot_id) = match &plan.source {
                BackupSource::Snapshot(snapshot) => {
                    let device_path = vss::ffi::get_snapshot_device_path(&snapshot.id)?;
                    (std::path::PathBuf::from(device_path), None)
                }
                BackupSource::Volume(volume) => {
                    match &plan.snapshot_policy {
                        SnapshotPolicy::Temporary { kind, label, .. } => {
                            // Try VSS snapshot; fall back to direct volume copy
                            // when VSS is unavailable (e.g. VHD volumes on desktop).
                            let provider = vss::VssSnapshotProvider::new();
                            match provider.create_snapshot(&SnapshotRequest {
                                source: volume.clone(),
                                kind: *kind,
                                label: label.clone(),
                                read_only: true,
                            }) {
                                Ok(info) if info.path_hint.is_some() => {
                                    let device_path = info
                                        .path_hint
                                        .as_ref()
                                        .unwrap()
                                        .to_string_lossy()
                                        .to_string();
                                    if !device_path.trim().is_empty() {
                                        (
                                            std::path::PathBuf::from(&device_path),
                                            Some(info.handle.id),
                                        )
                                    } else {
                                        info!("VSS snapshot created but has empty device path, falling back to direct copy");
                                        (
                                            std::path::PathBuf::from(volume_path_for_device(
                                                &volume.id,
                                            )),
                                            None,
                                        )
                                    }
                                }
                                Ok(_) => {
                                    info!("VSS snapshot created but has no device path, falling back to direct copy");
                                    (
                                        std::path::PathBuf::from(volume_path_for_device(
                                            &volume.id,
                                        )),
                                        None,
                                    )
                                }
                                Err(e) => {
                                    info!(
                                        "VSS snapshot failed ({}), falling back to direct volume copy",
                                        e
                                    );
                                    let volume_path = volume_path_for_device(&volume.id);
                                    (std::path::PathBuf::from(volume_path), None)
                                }
                            }
                        }
                        _ => {
                            let volume_path = volume_path_for_device(&volume.id);
                            (std::path::PathBuf::from(volume_path), None)
                        }
                    }
                }
            };

            let copy_dst = match &plan.target {
                crate::types::BackupTarget::ImageFile(path) => path.clone(),
                crate::types::BackupTarget::Device(path) => {
                    return Err(crate::error::Error::InvalidArgument {
                        message: format!(
                            "vss backup currently supports only image-file targets, got `{}`",
                            path.display()
                        ),
                    });
                }
            };

            let block_size = plan.block_size.unwrap_or(crate::copy::DEFAULT_BLOCK_SIZE);

            let copy_result =
                crate::copy::copy_blocks(&copy_src, &copy_dst, block_size).map(|_| ());

            // Clean up temporary snapshot
            let cleanup_result = if let Some(snapshot_id) = &temp_snapshot_id {
                vss::ffi::delete_snapshot(snapshot_id).map(|_| ())
            } else {
                Ok(())
            };

            match (copy_result, cleanup_result) {
                (Ok(_), Ok(())) => Ok(()),
                (Err(error), Ok(())) => Err(error),
                (Ok(_), Err(error)) => Err(error),
                (Err(error), Err(cleanup_error)) => {
                    error!(
                        backend = self.backend_name(),
                        source = %source_display,
                        cleanup_error = %cleanup_error,
                        "backup cleanup failed after copy error"
                    );
                    Err(error)
                }
            }
        })();

        if let Err(ref error) = result {
            error!(backend = self.backend_name(), source = %source_display, error = %error, "backup_volume failed");
        }
        result
    }
}

#[cfg(not(feature = "windows-vss"))]
impl crate::backup::BlockDeviceCopier for WindowsBackend {
    fn backend_name(&self) -> &'static str {
        self.0.backend_name()
    }

    fn capabilities(&self) -> &'static [Capability] {
        self.0.capabilities()
    }

    fn backup_volume(&self, _plan: &crate::types::BackupPlan) -> crate::error::Result<()> {
        Err(crate::error::Error::UnsupportedOperation {
            operation: "backup_volume",
            backend: self.backend_name(),
        })
    }
}

// ── RestorePlanner ─────────────────────────────────────────────────────────

#[cfg(feature = "windows-vss")]
impl crate::restore::RestorePlanner for WindowsBackend {
    fn backend_name(&self) -> &'static str {
        self.0.backend_name()
    }

    fn capabilities(&self) -> &'static [Capability] {
        self.0.capabilities()
    }

    fn restore_volume(&self, plan: &crate::types::RestorePlan) -> crate::error::Result<()> {
        use tracing::{error, info};

        info!(backend = self.backend_name(), destination = %plan.destination, "restore_volume called");

        let result = (|| -> crate::error::Result<()> {
            let source = match &plan.source {
                crate::types::BackupTarget::ImageFile(path) => path.clone(),
                crate::types::BackupTarget::Device(path) => {
                    return Err(crate::error::Error::InvalidArgument {
                        message: format!(
                            "vss restore currently supports only image-file sources, got `{}`",
                            path.display()
                        ),
                    });
                }
            };

            if !plan.force {
                return Err(crate::error::Error::InvalidArgument {
                    message: "vss restore requires `--force` because it overwrites the destination volume".to_string(),
                });
            }

            let destination = volume_path_for_device(&plan.destination.id);
            let block_size = plan.block_size.unwrap_or(crate::copy::DEFAULT_BLOCK_SIZE);

            crate::copy::copy_blocks(&source, std::path::Path::new(&destination), block_size)?;
            Ok(())
        })();

        if let Err(ref error) = result {
            error!(backend = self.backend_name(), destination = %plan.destination, error = %error, "restore_volume failed");
        }
        result
    }
}

#[cfg(not(feature = "windows-vss"))]
impl crate::restore::RestorePlanner for WindowsBackend {
    fn backend_name(&self) -> &'static str {
        self.0.backend_name()
    }

    fn capabilities(&self) -> &'static [Capability] {
        self.0.capabilities()
    }

    fn restore_volume(&self, _plan: &crate::types::RestorePlan) -> crate::error::Result<()> {
        Err(crate::error::Error::UnsupportedOperation {
            operation: "restore_volume",
            backend: self.backend_name(),
        })
    }
}

// ── MountManager ───────────────────────────────────────────────────────────

impl crate::mount::MountManager for WindowsBackend {
    fn backend_name(&self) -> &'static str {
        self.0.backend_name()
    }

    fn capabilities(&self) -> &'static [Capability] {
        self.0.capabilities()
    }

    fn mount_snapshot(
        &self,
        _request: &crate::types::MountRequest,
    ) -> crate::error::Result<crate::types::MountHandle> {
        let error = crate::error::Error::UnsupportedOperation {
            operation: "mount_snapshot",
            backend: self.backend_name(),
        };
        tracing::error!(backend = self.backend_name(), error = %error, "mount_snapshot failed");
        Err(error)
    }

    fn unmount(&self, _handle: &crate::types::MountHandle) -> crate::error::Result<()> {
        let error = crate::error::Error::UnsupportedOperation {
            operation: "unmount",
            backend: self.backend_name(),
        };
        tracing::error!(backend = self.backend_name(), error = %error, "unmount failed");
        Err(error)
    }
}

// ── Helpers ────────────────────────────────────────────────────────────────

/// Convert a volume reference to a path suitable for block-level I/O.
/// Drive letters (e.g. "C:" or "C:\") become `\\.\C:` for raw device access.
/// Volume GUID paths pass through unchanged.
#[cfg(feature = "windows-vss")]
fn volume_path_for_device(id: &str) -> String {
    let trimmed = id.trim_end_matches('\\').trim_end_matches('/');
    if trimmed.len() == 2 && trimmed.as_bytes()[1] == b':' {
        // Drive letter → raw device path
        format!(r"\\.\{}", trimmed)
    } else if trimmed.starts_with(r"\\?\Volume{") || trimmed.starts_with(r"\\.\Volume{") {
        trimmed.to_string()
    } else {
        trimmed.to_string()
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backend_has_expected_name() {
        let backend = WindowsBackend::new();
        assert_eq!(backend.backend_name(), "windows-vss");
    }

    #[test]
    fn backend_has_expected_capabilities() {
        let backend = WindowsBackend::new();
        assert!(backend
            .capabilities()
            .contains(&Capability::CrashConsistentSnapshot));
        assert!(backend
            .capabilities()
            .contains(&Capability::ApplicationConsistentSnapshot));
        assert!(backend
            .capabilities()
            .contains(&Capability::BlockLevelBackup));
        assert!(backend
            .capabilities()
            .contains(&Capability::BlockLevelRestore));
        assert!(backend
            .capabilities()
            .contains(&Capability::DirectDeviceAccess));
    }

    #[cfg(feature = "windows-vss")]
    #[test]
    fn volume_path_converts_drive_letter() {
        assert_eq!(volume_path_for_device("C:"), r"\\.\C");
        assert_eq!(volume_path_for_device("C:\\"), r"\\.\C");
        assert_eq!(volume_path_for_device("D:"), r"\\.\D");
    }

    #[cfg(feature = "windows-vss")]
    #[test]
    fn volume_path_preserves_guid_path() {
        let guid = r"\\?\Volume{12345678-abcd-ef01-1122-334455667788}\";
        assert_eq!(volume_path_for_device(guid), guid.trim_end_matches('\\'));
    }
}
