use tracing::{error, info};

use crate::error::Result;
use crate::mount::MountManager;
use crate::snapshot::SnapshotProvider;
use crate::types::{
    MountHandle, MountMode, MountRequest, SnapshotHandle, SnapshotInfo, SnapshotKind,
    SnapshotRequest, VolumeRef,
};

/// Request for creating a temporary snapshot and mounting it for browsing or copy-out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CopyMountRequest {
    pub source: VolumeRef,
    pub kind: SnapshotKind,
    pub label: Option<String>,
    pub mode: MountMode,
    pub target: Option<std::path::PathBuf>,
}

/// Result of a temporary snapshot + mount workflow.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CopyMountSession {
    pub snapshot: SnapshotInfo,
    pub mount: MountHandle,
}

/// Opens a temporary snapshot-backed mount through a backend that supports both traits.
pub fn open_copy_mount<T>(backend: &T, request: &CopyMountRequest) -> Result<CopyMountSession>
where
    T: SnapshotProvider + MountManager,
{
    info!(
        backend = SnapshotProvider::backend_name(backend),
        source = %request.source,
        mode = ?request.mode,
        "open_copy_mount called"
    );

    let result = (|| {
        let snapshot = backend.create_snapshot(&SnapshotRequest {
            source: request.source.clone(),
            kind: request.kind,
            label: request.label.clone(),
            read_only: matches!(request.mode, MountMode::ReadOnly),
        })?;

        let mount_result = backend.mount_snapshot(&MountRequest {
            snapshot: snapshot.handle.clone(),
            mode: request.mode,
            target: request.target.clone(),
        });

        match mount_result {
            Ok(mount) => Ok(CopyMountSession { snapshot, mount }),
            Err(error) => {
            let _ = backend.delete_snapshot(&snapshot.handle);
                Err(error)
            }
        }
    })();

    if let Err(error) = &result {
        error!(
            backend = SnapshotProvider::backend_name(backend),
            source = %request.source,
            error = %error,
            "open_copy_mount failed"
        );
    }

    result
}

/// Closes a temporary snapshot-backed mount by unmounting and deleting the snapshot.
pub fn close_copy_mount<T>(
    backend: &T,
    snapshot: &SnapshotHandle,
    mount: &MountHandle,
) -> Result<()>
where
    T: SnapshotProvider + MountManager,
{
    info!(
        backend = SnapshotProvider::backend_name(backend),
        snapshot = %snapshot.id,
        mount_point = %mount.mount_point.display(),
        "close_copy_mount called"
    );

    let result = (|| {
        backend.unmount(mount)?;
        backend.delete_snapshot(snapshot)?;
        Ok(())
    })();

    if let Err(error) = &result {
        error!(
            backend = SnapshotProvider::backend_name(backend),
            snapshot = %snapshot.id,
            mount_point = %mount.mount_point.display(),
            error = %error,
            "close_copy_mount failed"
        );
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::Error;
    use crate::types::Capability;

    #[derive(Debug, Default)]
    struct FakeBackend {
        fail_mount: bool,
        deleted: std::sync::Mutex<Vec<String>>,
        unmounted: std::sync::Mutex<Vec<String>>,
    }

    impl SnapshotProvider for FakeBackend {
        fn backend_name(&self) -> &'static str {
            "fake"
        }

        fn capabilities(&self) -> &'static [Capability] {
            &[]
        }

        fn create_snapshot(&self, request: &SnapshotRequest) -> Result<SnapshotInfo> {
            Ok(SnapshotInfo {
                handle: SnapshotHandle {
                    id: format!("snap:{}", request.source.id),
                    source: request.source.clone(),
                },
                backend: SnapshotProvider::backend_name(self),
                path_hint: None,
                read_only: request.read_only,
            })
        }

        fn delete_snapshot(&self, snapshot: &SnapshotHandle) -> Result<()> {
            self.deleted.lock().unwrap().push(snapshot.id.clone());
            Ok(())
        }

        fn list_snapshots(&self, _source: &VolumeRef) -> Result<Vec<SnapshotInfo>> {
            Ok(Vec::new())
        }
    }

    impl MountManager for FakeBackend {
        fn backend_name(&self) -> &'static str {
            "fake"
        }

        fn capabilities(&self) -> &'static [Capability] {
            &[]
        }

        fn mount_snapshot(&self, request: &MountRequest) -> Result<MountHandle> {
            if self.fail_mount {
                return Err(Error::Message {
                    message: "mount failed".to_string(),
                });
            }

            Ok(MountHandle {
                id: request.snapshot.id.clone(),
                mount_point: request
                    .target
                    .clone()
                    .unwrap_or_else(|| std::path::PathBuf::from("/tmp/fake-mount")),
            })
        }

        fn unmount(&self, handle: &MountHandle) -> Result<()> {
            self.unmounted
                .lock()
                .unwrap()
                .push(handle.mount_point.display().to_string());
            Ok(())
        }
    }

    #[test]
    fn open_copy_mount_returns_snapshot_and_mount() {
        let backend = FakeBackend::default();
        let session = open_copy_mount(
            &backend,
            &CopyMountRequest {
                source: VolumeRef::new("/dev/vg0/data"),
                kind: SnapshotKind::CrashConsistent,
                label: Some("copy".to_string()),
                mode: MountMode::ReadOnly,
                target: Some(std::path::PathBuf::from("/mnt/copy")),
            },
        )
        .unwrap();

        assert_eq!(session.snapshot.handle.id, "snap:/dev/vg0/data");
        assert_eq!(session.mount.mount_point, std::path::PathBuf::from("/mnt/copy"));
        assert!(session.snapshot.read_only);
    }

    #[test]
    fn open_copy_mount_deletes_snapshot_when_mount_fails() {
        let backend = FakeBackend {
            fail_mount: true,
            ..FakeBackend::default()
        };

        let error = open_copy_mount(
            &backend,
            &CopyMountRequest {
                source: VolumeRef::new("/dev/vg0/data"),
                kind: SnapshotKind::CrashConsistent,
                label: None,
                mode: MountMode::ReadOnly,
                target: None,
            },
        )
        .unwrap_err();

        assert!(matches!(error, Error::Message { .. }));
        assert_eq!(
            backend.deleted.lock().unwrap().as_slice(),
            &[String::from("snap:/dev/vg0/data")]
        );
    }

    #[test]
    fn close_copy_mount_unmounts_then_deletes_snapshot() {
        let backend = FakeBackend::default();
        close_copy_mount(
            &backend,
            &SnapshotHandle {
                id: "snap:/dev/vg0/data".to_string(),
                source: VolumeRef::new("/dev/vg0/data"),
            },
            &MountHandle {
                id: "snap:/dev/vg0/data".to_string(),
                mount_point: std::path::PathBuf::from("/mnt/copy"),
            },
        )
        .unwrap();

        assert_eq!(
            backend.unmounted.lock().unwrap().as_slice(),
            &[String::from("/mnt/copy")]
        );
        assert_eq!(
            backend.deleted.lock().unwrap().as_slice(),
            &[String::from("snap:/dev/vg0/data")]
        );
    }
}
