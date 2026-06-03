use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use tracing::{error, info};

use crate::backup::BlockDeviceCopier;
use crate::error::{Error, Result};
use crate::mount::MountManager;
use crate::platform::StubBackend;
use crate::process::{self, CommandIo};
use crate::restore::RestorePlanner;
use crate::snapshot::SnapshotProvider;
use crate::types::{
    BackupPlan, BackupSource, Capability, MountHandle, MountRequest, RestorePlan, SnapshotHandle,
    SnapshotInfo, SnapshotKind, SnapshotPolicy, SnapshotRequest, VolumeRef,
    sanitize_snapshot_label,
};

const CAPABILITIES: &[Capability] = &[
    Capability::CrashConsistentSnapshot,
    Capability::BlockLevelBackup,
    Capability::BlockLevelRestore,
    Capability::IncrementalSend,
    Capability::DirectDeviceAccess,
];

const ZFS_BIN: &str = "zfs";

#[derive(Debug, Clone)]
pub struct ZfsBackend(StubBackend);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZfsDatasetRef {
    pub name: String,
    pub mount_point: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZfsCommand {
    pub program: &'static str,
    pub args: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZfsSnapshotPlan {
    pub dataset: ZfsDatasetRef,
    pub snapshot_name: String,
    pub snapshot_id: String,
    pub read_only: bool,
    pub command: ZfsCommand,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZfsSnapshotRef {
    pub dataset: String,
    pub snapshot: String,
    pub snapshot_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZfsSendPlan {
    pub snapshot: ZfsSnapshotRef,
    pub target: PathBuf,
    pub parent_snapshot: Option<ZfsSnapshotRef>,
    pub temporary_snapshot: Option<ZfsSnapshotPlan>,
    pub command: ZfsCommand,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZfsReceivePlan {
    pub stream: PathBuf,
    pub destination_dataset: String,
    pub command: ZfsCommand,
}

impl ZfsCommand {
    fn new(args: impl IntoIterator<Item = String>) -> Self {
        Self {
            program: ZFS_BIN,
            args: args.into_iter().collect(),
        }
    }
}

impl ZfsBackend {
    pub fn new() -> Self {
        Self(StubBackend::new("linux-zfs", CAPABILITIES))
    }

    pub fn backend_name(&self) -> &'static str {
        self.0.backend_name()
    }

    pub fn parse_dataset_ref(&self, source: &VolumeRef) -> Result<ZfsDatasetRef> {
        let raw = source.id.trim();
        if raw.is_empty() {
            return Err(Error::InvalidVolume {
                volume: source.id.clone(),
            });
        }

        if raw.contains('@') {
            return Err(Error::InvalidArgument {
                message: format!(
                    "zfs provider expects a dataset name or mount path, not a snapshot id: `{raw}`"
                ),
            });
        }

        if raw.starts_with('/') {
            Ok(ZfsDatasetRef {
                name: raw.to_string(),
                mount_point: Some(PathBuf::from(raw)),
            })
        } else {
            Ok(ZfsDatasetRef {
                name: raw.to_string(),
                mount_point: None,
            })
        }
    }

    pub fn plan_create_snapshot(&self, request: &SnapshotRequest) -> Result<ZfsSnapshotPlan> {
        if matches!(request.kind, SnapshotKind::ApplicationConsistent) {
            return Err(Error::MissingCapability {
                capability: Capability::ApplicationConsistentSnapshot.as_str(),
                backend: self.backend_name(),
            });
        }

        let dataset = self.parse_dataset_ref(&request.source)?;
        let snapshot_name = derive_snapshot_name(request.label.as_deref());
        let snapshot_id = format!("{}@{}", dataset.name, snapshot_name);
        let mut args = vec!["snapshot".to_string()];
        if request.read_only {
            args.push("-r".to_string());
        }
        args.push(snapshot_id.clone());

        Ok(ZfsSnapshotPlan {
            dataset,
            snapshot_name,
            snapshot_id,
            read_only: request.read_only,
            command: ZfsCommand::new(args),
        })
    }

    pub fn plan_delete_snapshot(&self, snapshot: &SnapshotHandle) -> Result<ZfsCommand> {
        if snapshot.id.trim().is_empty() {
            return Err(Error::InvalidArgument {
                message: "snapshot id must not be empty".to_string(),
            });
        }

        Ok(ZfsCommand::new(vec![
            "destroy".to_string(),
            snapshot.id.clone(),
        ]))
    }

    pub fn plan_list_snapshots(&self, source: &VolumeRef) -> Result<(ZfsDatasetRef, ZfsCommand)> {
        let dataset = self.parse_dataset_ref(source)?;
        let command = ZfsCommand::new(vec![
            "list".to_string(),
            "-H".to_string(),
            "-t".to_string(),
            "snapshot".to_string(),
            "-o".to_string(),
            "name,mountpoint".to_string(),
            "-r".to_string(),
            dataset.name.clone(),
        ]);
        Ok((dataset, command))
    }

    pub fn parse_snapshot_ref(&self, source: &VolumeRef) -> Result<ZfsSnapshotRef> {
        let raw = source.id.trim();
        if raw.is_empty() {
            return Err(Error::InvalidVolume {
                volume: source.id.clone(),
            });
        }

        let Some((dataset, snapshot)) = raw.split_once('@') else {
            return Err(Error::InvalidArgument {
                message: format!(
                    "zfs send requires a snapshot source like `pool/fs@snap`, got `{raw}`"
                ),
            });
        };

        if dataset.is_empty() || snapshot.is_empty() {
            return Err(Error::InvalidArgument {
                message: format!("invalid zfs snapshot identifier `{raw}`"),
            });
        }

        Ok(ZfsSnapshotRef {
            dataset: dataset.to_string(),
            snapshot: snapshot.to_string(),
            snapshot_id: raw.to_string(),
        })
    }

    pub fn plan_backup(&self, plan: &BackupPlan) -> Result<ZfsSendPlan> {
        // For temporary snapshot policy, create the plan once and reuse it
        // for both the send snapshot reference and the temporary_snapshot field.
        let temporary_snapshot = match (&plan.source, &plan.snapshot_policy) {
            (
                BackupSource::Volume(volume),
                SnapshotPolicy::Temporary {
                    kind,
                    label,
                    read_only,
                },
            ) => Some(self.plan_create_snapshot(&SnapshotRequest {
                source: volume.clone(),
                kind: *kind,
                label: label.clone(),
                read_only: *read_only,
            })?),
            _ => None,
        };

        let snapshot = match (&plan.source, &plan.snapshot_policy) {
            (BackupSource::Snapshot(snapshot), _) => {
                self.parse_snapshot_ref(&VolumeRef::new(snapshot.id.clone()))?
            }
            (BackupSource::Volume(_), SnapshotPolicy::Temporary { .. }) => {
                // Reuse the already-created temporary snapshot plan
                self.parse_snapshot_ref(&VolumeRef::new(
                    temporary_snapshot.as_ref().unwrap().snapshot_id.clone(),
                ))?
            }
            (BackupSource::Volume(volume), SnapshotPolicy::Disabled) => {
                return Err(Error::InvalidArgument {
                    message: format!(
                        "zfs send backup requires a snapshot source or temporary snapshot policy for `{}`",
                        volume
                    ),
                });
            }
        };
        let target = match &plan.target {
            crate::types::BackupTarget::ImageFile(path) => path.clone(),
            crate::types::BackupTarget::Device(path) => {
                return Err(Error::InvalidArgument {
                    message: format!(
                        "zfs send backup requires an image file target, got device `{}`",
                        path.display()
                    ),
                });
            }
        };

        let parent_snapshot = match &plan.parent_snapshot {
            Some(snapshot) => Some(self.parse_snapshot_ref(&VolumeRef::new(snapshot.id.clone()))?),
            None => None,
        };

        let mut args = vec!["send".to_string()];
        if let Some(parent) = &parent_snapshot {
            args.push("-i".to_string());
            args.push(parent.snapshot_id.clone());
        }
        args.push(snapshot.snapshot_id.clone());
        let command = ZfsCommand::new(args);
        Ok(ZfsSendPlan {
            snapshot,
            target,
            parent_snapshot,
            temporary_snapshot,
            command,
        })
    }

    pub fn plan_restore(&self, plan: &RestorePlan) -> Result<ZfsReceivePlan> {
        let stream = match &plan.source {
            crate::types::BackupTarget::ImageFile(path) => path.clone(),
            crate::types::BackupTarget::Device(path) => {
                return Err(Error::InvalidArgument {
                    message: format!(
                        "zfs receive restore requires an image file source, got device `{}`",
                        path.display()
                    ),
                });
            }
        };

        if !stream.exists() {
            return Err(Error::MissingPath { path: stream });
        }

        let destination_dataset = self.parse_receive_destination(&plan.destination)?;
        let mut args = vec!["receive".to_string()];
        if plan.force {
            args.push("-F".to_string());
        }
        args.push(destination_dataset.clone());

        Ok(ZfsReceivePlan {
            stream,
            destination_dataset,
            command: ZfsCommand::new(args),
        })
    }

    fn parse_receive_destination(&self, destination: &VolumeRef) -> Result<String> {
        let raw = destination.id.trim();
        if raw.is_empty() {
            return Err(Error::InvalidVolume {
                volume: destination.id.clone(),
            });
        }

        if raw.contains('@') {
            return Err(Error::InvalidArgument {
                message: format!(
                    "zfs receive expects a dataset destination, not a snapshot id: `{raw}`"
                ),
            });
        }

        if raw.starts_with('/') {
            return Err(Error::InvalidArgument {
                message: format!(
                    "zfs receive expects a dataset name like `pool/fs`, not a mount path: `{raw}`"
                ),
            });
        }

        Ok(raw.to_string())
    }

    fn run_command(
        &self,
        operation: &'static str,
        command: &ZfsCommand,
    ) -> Result<std::process::Output> {
        process::run_command(
            self.backend_name(),
            operation,
            command.program,
            &command.args,
            CommandIo::default(),
        )
    }

    fn parse_list_output(&self, dataset: &ZfsDatasetRef, stdout: &[u8]) -> Vec<SnapshotInfo> {
        let mut snapshots = Vec::new();

        for line in String::from_utf8_lossy(stdout).lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            let mut parts = line.split('\t');
            let Some(name) = parts.next() else {
                continue;
            };
            let mountpoint = parts.next().unwrap_or("-");

            if !name.starts_with(&format!("{}@", dataset.name)) {
                continue;
            }

            let path_hint = match mountpoint {
                "-" | "legacy" | "none" => None,
                value => Some(PathBuf::from(value)),
            };

            snapshots.push(SnapshotInfo {
                handle: SnapshotHandle {
                    id: name.to_string(),
                    source: Some(VolumeRef::new(dataset.name.clone())),
                },
                backend: self.backend_name(),
                path_hint,
                read_only: true,
            });
        }

        snapshots
    }
}

impl SnapshotProvider for ZfsBackend {
    fn backend_name(&self) -> &'static str {
        self.0.backend_name()
    }

    fn capabilities(&self) -> &'static [Capability] {
        self.0.capabilities()
    }

    fn create_snapshot(&self, request: &SnapshotRequest) -> Result<SnapshotInfo> {
        info!(backend = self.backend_name(), source = %request.source, read_only = request.read_only, "create_snapshot called");
        let result = (|| {
            let plan = self.plan_create_snapshot(request)?;
            self.run_command("create_snapshot", &plan.command)?;

            Ok(SnapshotInfo {
                handle: SnapshotHandle {
                    id: plan.snapshot_id,
                    source: Some(request.source.clone()),
                },
                backend: self.backend_name(),
                path_hint: plan.dataset.mount_point,
                read_only: plan.read_only,
            })
        })();
        if let Err(error) = &result {
            error!(backend = self.backend_name(), source = %request.source, error = %error, "create_snapshot failed");
        }
        result
    }

    fn delete_snapshot(&self, snapshot: &SnapshotHandle) -> Result<()> {
        info!(backend = self.backend_name(), snapshot = %snapshot.id, "delete_snapshot called");
        let result = (|| {
            let command = self.plan_delete_snapshot(snapshot)?;
            self.run_command("delete_snapshot", &command)?;
            Ok(())
        })();
        if let Err(error) = &result {
            error!(backend = self.backend_name(), snapshot = %snapshot.id, error = %error, "delete_snapshot failed");
        }
        result
    }

    fn list_snapshots(&self, source: &VolumeRef) -> Result<Vec<SnapshotInfo>> {
        info!(backend = self.backend_name(), source = %source, "list_snapshots called");
        let result = (|| {
            let (dataset, command) = self.plan_list_snapshots(source)?;
            let output = self.run_command("list_snapshots", &command)?;
            Ok(self.parse_list_output(&dataset, &output.stdout))
        })();
        if let Err(error) = &result {
            error!(backend = self.backend_name(), source = %source, error = %error, "list_snapshots failed");
        }
        result
    }
}

impl BlockDeviceCopier for ZfsBackend {
    fn backend_name(&self) -> &'static str {
        self.0.backend_name()
    }

    fn capabilities(&self) -> &'static [Capability] {
        self.0.capabilities()
    }

    fn backup_volume(&self, plan: &BackupPlan) -> Result<()> {
        info!(backend = self.backend_name(), source = %plan.source, "backup_volume called");
        let send_plan = match self.plan_backup(plan) {
            Ok(plan) => plan,
            Err(error) => {
                error!(backend = self.backend_name(), source = %plan.source, error = %error, "backup_volume failed");
                return Err(error);
            }
        };

        if let Some(snapshot_plan) = &send_plan.temporary_snapshot
            && let Err(error) = self.run_command("create_snapshot", &snapshot_plan.command)
        {
            error!(backend = self.backend_name(), source = %plan.source, error = %error, "backup_volume failed");
            return Err(error);
        }

        let result = (|| {
            process::run_command(
                self.backend_name(),
                "backup_volume",
                send_plan.command.program,
                &send_plan.command.args,
                CommandIo {
                    stdin_file: None,
                    stdout_file: Some(send_plan.target.clone()),
                },
            )?;
            Ok(())
        })();
        if let Some(snapshot_plan) = &send_plan.temporary_snapshot
            && let Err(cleanup_err) = self.run_command(
                "delete_snapshot",
                &ZfsCommand::new(vec![
                    "destroy".to_string(),
                    snapshot_plan.snapshot_id.clone(),
                ]),
            )
        {
            tracing::warn!(
                backend = self.backend_name(),
                snapshot = %snapshot_plan.snapshot_id,
                error = %cleanup_err,
                "failed to clean up temporary snapshot"
            );
        }
        if let Err(error) = &result {
            error!(backend = self.backend_name(), source = %plan.source, error = %error, "backup_volume failed");
        }
        result
    }
}

impl RestorePlanner for ZfsBackend {
    fn backend_name(&self) -> &'static str {
        self.0.backend_name()
    }

    fn capabilities(&self) -> &'static [Capability] {
        self.0.capabilities()
    }

    fn restore_volume(&self, plan: &RestorePlan) -> Result<()> {
        info!(backend = self.backend_name(), destination = %plan.destination, "restore_volume called");
        let result = (|| {
            let receive_plan = self.plan_restore(plan)?;
            process::run_command(
                self.backend_name(),
                "restore_volume",
                receive_plan.command.program,
                &receive_plan.command.args,
                CommandIo {
                    stdin_file: Some(receive_plan.stream.clone()),
                    stdout_file: None,
                },
            )?;
            Ok(())
        })();
        if let Err(error) = &result {
            error!(backend = self.backend_name(), destination = %plan.destination, error = %error, "restore_volume failed");
        }
        result
    }
}

impl MountManager for ZfsBackend {
    fn backend_name(&self) -> &'static str {
        self.0.backend_name()
    }

    fn capabilities(&self) -> &'static [Capability] {
        self.0.capabilities()
    }

    fn mount_snapshot(&self, _request: &MountRequest) -> Result<MountHandle> {
        let error = Error::UnsupportedOperation {
            operation: "mount_snapshot",
            backend: self.backend_name(),
        };
        error!(backend = self.backend_name(), error = %error, "mount_snapshot failed");
        Err(error)
    }

    fn unmount(&self, _handle: &MountHandle) -> Result<()> {
        let error = Error::UnsupportedOperation {
            operation: "unmount",
            backend: self.backend_name(),
        };
        error!(backend = self.backend_name(), error = %error, "unmount failed");
        Err(error)
    }
}

fn derive_snapshot_name(label: Option<&str>) -> String {
    match label {
        Some(label) => sanitize_snapshot_segment(label),
        None => {
            let ts = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|duration| duration.as_secs())
                .unwrap_or(0);
            format!("snapshot-{ts}")
        }
    }
}

fn sanitize_snapshot_segment(value: &str) -> String {
    sanitize_snapshot_label(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::SnapshotRef;

    #[test]
    fn parses_dataset_name_without_mount_path() {
        let backend = ZfsBackend::new();
        let dataset = backend
            .parse_dataset_ref(&VolumeRef::new("tank/data"))
            .unwrap();

        assert_eq!(dataset.name, "tank/data");
        assert_eq!(dataset.mount_point, None);
    }

    #[test]
    fn parses_mount_path_as_dataset_reference() {
        let backend = ZfsBackend::new();
        let dataset = backend
            .parse_dataset_ref(&VolumeRef::new("/tank/data"))
            .unwrap();

        assert_eq!(dataset.name, "/tank/data");
        assert_eq!(dataset.mount_point, Some(PathBuf::from("/tank/data")));
    }

    #[test]
    fn create_plan_uses_zfs_snapshot_command() {
        let backend = ZfsBackend::new();
        let plan = backend
            .plan_create_snapshot(&SnapshotRequest {
                source: VolumeRef::new("tank/data"),
                kind: SnapshotKind::CrashConsistent,
                label: Some("nightly backup".to_string()),
                read_only: true,
            })
            .unwrap();

        assert_eq!(plan.snapshot_name, "nightly-backup");
        assert_eq!(plan.snapshot_id, "tank/data@nightly-backup");
        assert_eq!(
            plan.command.args,
            vec!["snapshot", "-r", "tank/data@nightly-backup"]
        );
    }

    #[test]
    fn parse_list_output_filters_matching_dataset_snapshots() {
        let backend = ZfsBackend::new();
        let dataset = ZfsDatasetRef {
            name: "tank/data".to_string(),
            mount_point: None,
        };

        let snapshots = backend.parse_list_output(
            &dataset,
            b"tank/data@snap1\t-\ntank/other@snapx\t-\ntank/data@snap2\t/mnt/tank/data/.zfs/snapshot/snap2\n",
        );

        assert_eq!(snapshots.len(), 2);
        assert_eq!(snapshots[0].handle.id, "tank/data@snap1");
        assert_eq!(snapshots[0].path_hint, None);
        assert_eq!(snapshots[1].handle.id, "tank/data@snap2");
        assert_eq!(
            snapshots[1].path_hint,
            Some(PathBuf::from("/mnt/tank/data/.zfs/snapshot/snap2"))
        );
    }

    #[test]
    fn application_consistent_requests_are_rejected() {
        let backend = ZfsBackend::new();
        let error = backend
            .plan_create_snapshot(&SnapshotRequest {
                source: VolumeRef::new("tank/data"),
                kind: SnapshotKind::ApplicationConsistent,
                label: None,
                read_only: true,
            })
            .unwrap_err();

        assert!(matches!(error, Error::MissingCapability { .. }));
    }

    #[test]
    fn backup_plan_uses_zfs_send_with_snapshot_source() {
        let backend = ZfsBackend::new();
        let plan = backend
            .plan_backup(&BackupPlan {
                source: BackupSource::Snapshot(SnapshotRef::new("tank/data@snap1")),
                target: crate::types::BackupTarget::ImageFile(PathBuf::from("/tmp/out.zfs")),
                snapshot_policy: SnapshotPolicy::disabled(),
                parent_snapshot: None,
                block_size: None,
            })
            .unwrap();

        assert_eq!(plan.snapshot.snapshot_id, "tank/data@snap1");
        assert_eq!(plan.command.args, vec!["send", "tank/data@snap1"]);
    }

    #[test]
    fn backup_plan_rejects_non_snapshot_source() {
        let backend = ZfsBackend::new();
        let error = backend
            .plan_backup(&BackupPlan {
                source: BackupSource::Volume(VolumeRef::new("tank/data")),
                target: crate::types::BackupTarget::ImageFile(PathBuf::from("/tmp/out.zfs")),
                snapshot_policy: SnapshotPolicy::disabled(),
                parent_snapshot: None,
                block_size: None,
            })
            .unwrap_err();

        assert!(matches!(error, Error::InvalidArgument { .. }));
    }

    #[test]
    fn backup_plan_uses_parent_snapshot_for_incremental_send() {
        let backend = ZfsBackend::new();
        let plan = backend
            .plan_backup(&BackupPlan {
                source: BackupSource::Snapshot(
                    SnapshotRef::new("tank/data@snap2").with_origin(VolumeRef::new("tank/data")),
                ),
                target: crate::types::BackupTarget::ImageFile(PathBuf::from("/tmp/out.zfs")),
                snapshot_policy: SnapshotPolicy::disabled(),
                parent_snapshot: Some(
                    SnapshotRef::new("tank/data@snap1").with_origin(VolumeRef::new("tank/data")),
                ),
                block_size: None,
            })
            .unwrap();

        assert_eq!(
            plan.command.args,
            vec!["send", "-i", "tank/data@snap1", "tank/data@snap2"]
        );
        assert_eq!(
            plan.parent_snapshot
                .as_ref()
                .map(|snapshot| snapshot.snapshot_id.as_str()),
            Some("tank/data@snap1")
        );
    }

    #[test]
    fn restore_plan_uses_zfs_receive_dataset_destination() {
        let backend = ZfsBackend::new();
        let root = std::env::temp_dir().join(format!("vpt-rs-zfs-receive-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let stream = root.join("backup.zfs");
        std::fs::write(&stream, b"stream").unwrap();

        let plan = backend
            .plan_restore(&RestorePlan {
                source: crate::types::BackupTarget::ImageFile(stream.clone()),
                destination: VolumeRef::new("tank/restore"),
                force: true,
                base_snapshot: None,
                block_size: None,
            })
            .unwrap();

        assert_eq!(plan.stream, stream);
        assert_eq!(plan.destination_dataset, "tank/restore");
        assert_eq!(plan.command.args, vec!["receive", "-F", "tank/restore"]);

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn restore_plan_rejects_mount_path_destination() {
        let backend = ZfsBackend::new();
        let root =
            std::env::temp_dir().join(format!("vpt-rs-zfs-restore-path-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let stream = root.join("backup.zfs");
        std::fs::write(&stream, b"stream").unwrap();

        let error = backend
            .plan_restore(&RestorePlan {
                source: crate::types::BackupTarget::ImageFile(stream),
                destination: VolumeRef::new("/tank/restore"),
                force: false,
                base_snapshot: None,
                block_size: None,
            })
            .unwrap_err();

        assert!(matches!(error, Error::InvalidArgument { .. }));
        let _ = std::fs::remove_dir_all(&root);
    }
}
