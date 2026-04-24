use std::path::{Path, PathBuf};
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
    SnapshotInfo, SnapshotKind, SnapshotPolicy, SnapshotRef, SnapshotRequest, VolumeRef,
};

const CAPABILITIES: &[Capability] = &[
    Capability::CrashConsistentSnapshot,
    Capability::ReadOnlySnapshotMount,
    Capability::WritableSnapshotMount,
    Capability::BlockLevelBackup,
    Capability::BlockLevelRestore,
    Capability::IncrementalSend,
];

const BTRFS_BIN: &str = "btrfs";
const MOUNT_BIN: &str = "mount";
const UMOUNT_BIN: &str = "umount";
const TEMP_MOUNT_PREFIX: &str = "vpt-btrfs-mount-";

#[derive(Debug, Clone)]
pub struct BtrfsBackend(StubBackend);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BtrfsCommand {
    pub program: &'static str,
    pub args: Vec<String>,
}

impl BtrfsCommand {
    fn new(args: impl IntoIterator<Item = String>) -> Self {
        Self {
            program: BTRFS_BIN,
            args: args.into_iter().collect(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BtrfsSnapshotPlan {
    pub source: PathBuf,
    pub snapshot_path: PathBuf,
    pub read_only: bool,
    pub command: BtrfsCommand,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BtrfsSendPlan {
    pub source: PathBuf,
    pub target: PathBuf,
    pub parent: Option<PathBuf>,
    pub temporary_snapshot: Option<BtrfsSnapshotPlan>,
    pub command: BtrfsCommand,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BtrfsReceivePlan {
    pub stream: PathBuf,
    pub destination_dir: PathBuf,
    pub command: BtrfsCommand,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BtrfsMountPlan {
    pub snapshot_path: PathBuf,
    pub mount_point: PathBuf,
    pub auto_created_target: bool,
    pub commands: Vec<BtrfsCommand>,
}

impl BtrfsBackend {
    pub fn new() -> Self {
        Self(StubBackend::new("linux-btrfs", CAPABILITIES))
    }

    pub fn backend_name(&self) -> &'static str {
        self.0.backend_name()
    }

    pub fn plan_create_snapshot(&self, request: &SnapshotRequest) -> Result<BtrfsSnapshotPlan> {
        self.validate_snapshot_request(request)?;

        let source = PathBuf::from(&request.source.id);
        let snapshot_path = self.derive_snapshot_path(request, &source)?;
        let mut args = vec!["subvolume".to_string(), "snapshot".to_string()];
        if request.read_only {
            args.push("-r".to_string());
        }
        args.push(source.display().to_string());
        args.push(snapshot_path.display().to_string());

        Ok(BtrfsSnapshotPlan {
            source,
            snapshot_path,
            read_only: request.read_only,
            command: BtrfsCommand::new(args),
        })
    }

    pub fn plan_delete_snapshot(&self, snapshot: &SnapshotHandle) -> Result<BtrfsCommand> {
        let path = self.snapshot_handle_path(snapshot)?;
        Ok(BtrfsCommand::new(vec![
            "subvolume".to_string(),
            "delete".to_string(),
            path.display().to_string(),
        ]))
    }

    pub fn plan_list_snapshots(&self, source: &VolumeRef) -> Result<BtrfsCommand> {
        let path = self.volume_path(source)?;
        Ok(BtrfsCommand::new(vec![
            "subvolume".to_string(),
            "list".to_string(),
            "-s".to_string(),
            path.display().to_string(),
        ]))
    }

    pub fn plan_backup(&self, plan: &BackupPlan) -> Result<BtrfsSendPlan> {
        let target = match &plan.target {
            crate::types::BackupTarget::ImageFile(path) => path.clone(),
            crate::types::BackupTarget::Device(path) => {
                return Err(Error::InvalidArgument {
                    message: format!(
                        "btrfs send backup requires an image file target, got device `{}`",
                        path.display()
                    ),
                });
            }
        };

        let (source, temporary_snapshot) = match (&plan.source, &plan.snapshot_policy) {
            (BackupSource::Snapshot(snapshot), _) => (self.snapshot_ref_path(snapshot)?, None),
            (BackupSource::Volume(volume), SnapshotPolicy::Disabled) => {
                let source = self.volume_path(volume)?;
                if !source.exists() {
                    return Err(Error::MissingPath { path: source });
                }
                (source, None)
            }
            (
                BackupSource::Volume(volume),
                SnapshotPolicy::Temporary {
                    kind,
                    label,
                    read_only,
                },
            ) => {
                let request = SnapshotRequest {
                    source: volume.clone(),
                    kind: *kind,
                    label: label.clone(),
                    read_only: *read_only,
                };
                let snapshot_plan = self.plan_create_snapshot(&request)?;
                (snapshot_plan.snapshot_path.clone(), Some(snapshot_plan))
            }
        };

        let parent = match &plan.parent_snapshot {
            Some(snapshot) => Some(self.snapshot_ref_path(snapshot)?),
            None => None,
        };

        let mut args = vec!["send".to_string()];
        if let Some(parent) = &parent {
            args.push("-p".to_string());
            args.push(parent.display().to_string());
        }
        args.push(source.display().to_string());
        let command = BtrfsCommand::new(args);

        Ok(BtrfsSendPlan {
            source,
            target,
            parent,
            temporary_snapshot,
            command,
        })
    }

    pub fn plan_restore(&self, plan: &RestorePlan) -> Result<BtrfsReceivePlan> {
        let stream = match &plan.source {
            crate::types::BackupTarget::ImageFile(path) => path.clone(),
            crate::types::BackupTarget::Device(path) => {
                return Err(Error::InvalidArgument {
                    message: format!(
                        "btrfs receive restore requires an image file source, got device `{}`",
                        path.display()
                    ),
                });
            }
        };

        if !stream.exists() {
            return Err(Error::MissingPath { path: stream });
        }

        let destination_dir = self.volume_path(&plan.destination)?;
        if !destination_dir.exists() {
            return Err(Error::MissingPath {
                path: destination_dir,
            });
        }

        let command = BtrfsCommand::new(vec![
            "receive".to_string(),
            destination_dir.display().to_string(),
        ]);

        Ok(BtrfsReceivePlan {
            stream,
            destination_dir,
            command,
        })
    }

    pub fn plan_mount_snapshot(&self, request: &MountRequest) -> Result<BtrfsMountPlan> {
        let snapshot_path = self.snapshot_handle_path(&request.snapshot)?;
        if !snapshot_path.exists() {
            return Err(Error::MissingPath {
                path: snapshot_path.clone(),
            });
        }

        let (mount_point, auto_created_target) = match &request.target {
            Some(target) => (target.clone(), false),
            None => (
                std::env::temp_dir().join(format!(
                    "{TEMP_MOUNT_PREFIX}{}",
                    sanitize_label(
                        snapshot_path
                            .file_name()
                            .and_then(|name| name.to_str())
                            .unwrap_or("snapshot")
                    )
                )),
                true,
            ),
        };

        let mut commands = vec![BtrfsCommand {
            program: MOUNT_BIN,
            args: vec![
                "--bind".to_string(),
                snapshot_path.display().to_string(),
                mount_point.display().to_string(),
            ],
        }];

        if matches!(request.mode, crate::types::MountMode::ReadOnly) {
            commands.push(BtrfsCommand {
                program: MOUNT_BIN,
                args: vec![
                    "-o".to_string(),
                    "remount,bind,ro".to_string(),
                    mount_point.display().to_string(),
                ],
            });
        }

        Ok(BtrfsMountPlan {
            snapshot_path,
            mount_point,
            auto_created_target,
            commands,
        })
    }

    pub fn plan_unmount(&self, handle: &MountHandle) -> Result<(PathBuf, BtrfsCommand)> {
        if handle.mount_point.as_os_str().is_empty() {
            return Err(Error::InvalidArgument {
                message: "mount point must not be empty".to_string(),
            });
        }

        let mount_point = handle.mount_point.clone();
        Ok((
            mount_point.clone(),
            BtrfsCommand {
                program: UMOUNT_BIN,
                args: vec![mount_point.display().to_string()],
            },
        ))
    }

    fn validate_snapshot_request(&self, request: &SnapshotRequest) -> Result<()> {
        if matches!(request.kind, SnapshotKind::ApplicationConsistent) {
            return Err(Error::MissingCapability {
                capability: Capability::ApplicationConsistentSnapshot.as_str(),
                backend: self.backend_name(),
            });
        }

        let source = self.volume_path(&request.source)?;
        if !source.exists() {
            return Err(Error::MissingPath { path: source });
        }

        Ok(())
    }

    fn volume_path(&self, source: &VolumeRef) -> Result<PathBuf> {
        if source.id.trim().is_empty() {
            return Err(Error::InvalidVolume {
                volume: source.id.clone(),
            });
        }

        let path = PathBuf::from(&source.id);
        if !path.is_absolute() {
            return Err(Error::InvalidArgument {
                message: format!(
                    "btrfs provider expects an absolute subvolume path, got `{}`",
                    source.id
                ),
            });
        }

        Ok(path)
    }

    fn derive_snapshot_path(&self, request: &SnapshotRequest, source: &Path) -> Result<PathBuf> {
        let parent = source.parent().ok_or_else(|| Error::InvalidArgument {
            message: format!("cannot derive snapshot path from `{}`", source.display()),
        })?;
        let snapshot_root = parent.join(".vb-snapshots");
        let name = match &request.label {
            Some(label) => sanitize_label(label),
            None => default_snapshot_name(source),
        };

        Ok(snapshot_root.join(name))
    }

    fn snapshot_handle_path(&self, snapshot: &SnapshotHandle) -> Result<PathBuf> {
        if snapshot.id.trim().is_empty() {
            return Err(Error::InvalidArgument {
                message: "snapshot id must not be empty".to_string(),
            });
        }

        Ok(PathBuf::from(&snapshot.id))
    }

    fn snapshot_ref_path(&self, snapshot: &SnapshotRef) -> Result<PathBuf> {
        if snapshot.id.trim().is_empty() {
            return Err(Error::InvalidArgument {
                message: "snapshot reference must not be empty".to_string(),
            });
        }

        let path = PathBuf::from(&snapshot.id);
        if !path.is_absolute() {
            return Err(Error::InvalidArgument {
                message: format!(
                    "btrfs snapshot reference expects an absolute subvolume path, got `{}`",
                    snapshot.id
                ),
            });
        }

        Ok(path)
    }

    fn run_command(&self, command: &BtrfsCommand) -> Result<std::process::Output> {
        process::run_command(
            self.backend_name(),
            "run_command",
            command.program,
            &command.args,
            CommandIo::default(),
        )
    }

    fn run_send(&self, plan: &BtrfsSendPlan) -> Result<()> {
        if let Some(snapshot_plan) = &plan.temporary_snapshot {
            if let Some(parent) = snapshot_plan.snapshot_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            self.run_command(&snapshot_plan.command)?;
        }

        let result = process::run_command(
            self.backend_name(),
            "backup_volume",
            plan.command.program,
            &plan.command.args,
            CommandIo {
                stdin_file: None,
                stdout_file: Some(plan.target.clone()),
            },
        );

        if let Some(snapshot_plan) = &plan.temporary_snapshot {
            let _ = self.run_command(&BtrfsCommand::new(vec![
                "subvolume".to_string(),
                "delete".to_string(),
                snapshot_plan.snapshot_path.display().to_string(),
            ]));
        }

        result.map(|_| ())
    }

    fn run_receive(&self, plan: &BtrfsReceivePlan) -> Result<()> {
        process::run_command(
            self.backend_name(),
            "restore_volume",
            plan.command.program,
            &plan.command.args,
            CommandIo {
                stdin_file: Some(plan.stream.clone()),
                stdout_file: None,
            },
        )?;
        Ok(())
    }

    fn parse_list_output(&self, source: &VolumeRef, stdout: &[u8]) -> Vec<SnapshotInfo> {
        let source_path = PathBuf::from(&source.id);
        let parent = source_path.parent().map(Path::to_path_buf);
        let mut snapshots = Vec::new();

        for line in String::from_utf8_lossy(stdout).lines() {
            let Some(path_part) = line.split(" path ").nth(1) else {
                continue;
            };

            let raw_path = PathBuf::from(path_part.trim());
            let path_hint = if raw_path.is_absolute() {
                raw_path
            } else {
                parent
                    .as_ref()
                    .map(|base| base.join(&raw_path))
                    .unwrap_or(raw_path.clone())
            };

            snapshots.push(SnapshotInfo {
                handle: SnapshotHandle {
                    id: path_hint.display().to_string(),
                    source: source.clone(),
                },
                backend: self.backend_name(),
                path_hint: Some(path_hint),
                read_only: true,
            });
        }

        snapshots
    }
}

impl SnapshotProvider for BtrfsBackend {
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
            if let Some(parent) = plan.snapshot_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            self.run_command(&plan.command)?;

            Ok(SnapshotInfo {
                handle: SnapshotHandle {
                    id: plan.snapshot_path.display().to_string(),
                    source: request.source.clone(),
                },
                backend: self.backend_name(),
                path_hint: Some(plan.snapshot_path),
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
            self.run_command(&command)?;
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
            let command = self.plan_list_snapshots(source)?;
            let output = self.run_command(&command)?;
            Ok(self.parse_list_output(source, &output.stdout))
        })();
        if let Err(error) = &result {
            error!(backend = self.backend_name(), source = %source, error = %error, "list_snapshots failed");
        }
        result
    }
}

impl BlockDeviceCopier for BtrfsBackend {
    fn backend_name(&self) -> &'static str {
        self.0.backend_name()
    }

    fn capabilities(&self) -> &'static [Capability] {
        self.0.capabilities()
    }

    fn backup_volume(&self, plan: &BackupPlan) -> Result<()> {
        info!(backend = self.backend_name(), source = %plan.source, "backup_volume called");
        let result = (|| {
            let send_plan = self.plan_backup(plan)?;
            self.run_send(&send_plan)
        })();
        if let Err(error) = &result {
            error!(backend = self.backend_name(), source = %plan.source, error = %error, "backup_volume failed");
        }
        result
    }
}

impl RestorePlanner for BtrfsBackend {
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
            self.run_receive(&receive_plan)
        })();
        if let Err(error) = &result {
            error!(backend = self.backend_name(), destination = %plan.destination, error = %error, "restore_volume failed");
        }
        result
    }
}

impl MountManager for BtrfsBackend {
    fn backend_name(&self) -> &'static str {
        self.0.backend_name()
    }

    fn capabilities(&self) -> &'static [Capability] {
        self.0.capabilities()
    }

    fn mount_snapshot(&self, request: &MountRequest) -> Result<MountHandle> {
        info!(backend = self.backend_name(), snapshot = %request.snapshot.id, "mount_snapshot called");
        let result = (|| {
            let plan = self.plan_mount_snapshot(request)?;
            std::fs::create_dir_all(&plan.mount_point)?;
            for command in &plan.commands {
                self.run_command(command)?;
            }
            Ok(MountHandle {
                id: plan.mount_point.display().to_string(),
                mount_point: plan.mount_point,
            })
        })();
        if let Err(error) = &result {
            error!(backend = self.backend_name(), snapshot = %request.snapshot.id, error = %error, "mount_snapshot failed");
        }
        result
    }

    fn unmount(&self, handle: &MountHandle) -> Result<()> {
        info!(backend = self.backend_name(), mount_point = %handle.mount_point.display(), "unmount called");
        let result = (|| {
            let (mount_point, command) = self.plan_unmount(handle)?;
            self.run_command(&command)?;
            if is_temporary_mount_path(&mount_point) {
                let _ = std::fs::remove_dir(&mount_point);
            }
            Ok(())
        })();
        if let Err(error) = &result {
            error!(backend = self.backend_name(), mount_point = %handle.mount_point.display(), error = %error, "unmount failed");
        }
        result
    }
}

fn sanitize_label(label: &str) -> String {
    let sanitized: String = label
        .chars()
        .map(|ch| match ch {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' | '.' => ch,
            _ => '-',
        })
        .collect();

    if sanitized.trim_matches('-').is_empty() {
        "snapshot".to_string()
    } else {
        sanitized
    }
}

fn default_snapshot_name(source: &Path) -> String {
    let stem = source
        .file_name()
        .and_then(|segment| segment.to_str())
        .filter(|segment| !segment.is_empty())
        .unwrap_or("volume");

    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);

    format!("{stem}-{ts}")
}

fn is_temporary_mount_path(path: &Path) -> bool {
    path.parent() == Some(std::env::temp_dir().as_path())
        && path
            .file_name()
            .and_then(|name| name.to_str())
            .map(|name| name.starts_with(TEMP_MOUNT_PREFIX))
            .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::MountMode;

    #[test]
    fn create_plan_uses_hidden_snapshot_directory() {
        let backend = BtrfsBackend::new();
        let root = std::env::temp_dir().join(format!("vpt-rs-btrfs-plan-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let source = root.join("subvol");
        std::fs::create_dir_all(&source).unwrap();

        let plan = backend
            .plan_create_snapshot(&SnapshotRequest {
                source: VolumeRef::new(source.display().to_string()),
                kind: SnapshotKind::CrashConsistent,
                label: Some("nightly backup".to_string()),
                read_only: true,
            })
            .unwrap();

        assert_eq!(
            plan.snapshot_path,
            root.join(".vb-snapshots").join("nightly-backup")
        );
        assert_eq!(
            plan.command.args,
            vec![
                "subvolume",
                "snapshot",
                "-r",
                source.to_string_lossy().as_ref(),
                root.join(".vb-snapshots")
                    .join("nightly-backup")
                    .to_string_lossy()
                    .as_ref(),
            ]
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn application_consistent_requests_are_rejected() {
        let backend = BtrfsBackend::new();
        let source = std::env::temp_dir();

        let error = backend
            .plan_create_snapshot(&SnapshotRequest {
                source: VolumeRef::new(source.display().to_string()),
                kind: SnapshotKind::ApplicationConsistent,
                label: None,
                read_only: true,
            })
            .unwrap_err();

        assert!(matches!(error, Error::MissingCapability { .. }));
    }

    #[test]
    fn list_output_parses_paths() {
        let backend = BtrfsBackend::new();
        let source = VolumeRef::new("/mnt/data/subvol");
        let snapshots = backend.parse_list_output(
            &source,
            br#"ID 258 gen 301 top level 5 path .vb-snapshots/snap-1
ID 259 gen 302 top level 5 path /mnt/data/.vb-snapshots/snap-2
"#,
        );

        assert_eq!(snapshots.len(), 2);
        assert_eq!(
            snapshots[0].path_hint.as_ref().unwrap(),
            &PathBuf::from("/mnt/data/.vb-snapshots/snap-1")
        );
        assert_eq!(
            snapshots[1].path_hint.as_ref().unwrap(),
            &PathBuf::from("/mnt/data/.vb-snapshots/snap-2")
        );
    }

    #[test]
    fn backup_plan_uses_btrfs_send_to_image_file() {
        let backend = BtrfsBackend::new();
        let root = std::env::temp_dir().join(format!("vpt-rs-btrfs-send-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let source = root.join("subvol");
        std::fs::create_dir_all(&source).unwrap();
        let target = root.join("backup.stream");

        let plan = backend
            .plan_backup(&BackupPlan {
                source: BackupSource::Volume(VolumeRef::new(source.display().to_string())),
                target: crate::types::BackupTarget::ImageFile(target.clone()),
                snapshot_policy: SnapshotPolicy::temporary(
                    SnapshotKind::CrashConsistent,
                    Some("tmp".to_string()),
                    true,
                ),
                parent_snapshot: None,
            })
            .unwrap();

        assert_eq!(plan.source, root.join(".vb-snapshots").join("tmp"));
        assert_eq!(plan.target, target);
        assert_eq!(
            plan.command.args,
            vec!["send", plan.source.to_string_lossy().as_ref()]
        );
        assert!(plan.temporary_snapshot.is_some());

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn backup_plan_uses_parent_snapshot_for_incremental_send() {
        let backend = BtrfsBackend::new();
        let root = std::env::temp_dir().join(format!("vpt-rs-btrfs-parent-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let source = root.join("subvol");
        let parent = root.join(".vb-snapshots").join("base");
        std::fs::create_dir_all(&source).unwrap();

        let plan = backend
            .plan_backup(&BackupPlan {
                source: BackupSource::Snapshot(
                    SnapshotRef::new(source.display().to_string())
                        .with_origin(VolumeRef::new(source.display().to_string())),
                ),
                target: crate::types::BackupTarget::ImageFile(root.join("backup.stream")),
                snapshot_policy: SnapshotPolicy::disabled(),
                parent_snapshot: Some(
                    SnapshotRef::new(parent.display().to_string())
                        .with_origin(VolumeRef::new(source.display().to_string())),
                ),
            })
            .unwrap();

        assert_eq!(
            plan.command.args,
            vec![
                "send",
                "-p",
                parent.to_string_lossy().as_ref(),
                source.to_string_lossy().as_ref(),
            ]
        );
        assert_eq!(plan.parent, Some(parent));

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn restore_plan_uses_btrfs_receive_from_stream() {
        let backend = BtrfsBackend::new();
        let root =
            std::env::temp_dir().join(format!("vpt-rs-btrfs-receive-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let stream = root.join("backup.stream");
        std::fs::write(&stream, b"stream").unwrap();
        let destination = root.join("restore-root");
        std::fs::create_dir_all(&destination).unwrap();

        let plan = backend
            .plan_restore(&RestorePlan {
                source: crate::types::BackupTarget::ImageFile(stream.clone()),
                destination: VolumeRef::new(destination.display().to_string()),
                force: false,
                base_snapshot: None,
            })
            .unwrap();

        assert_eq!(plan.stream, stream);
        assert_eq!(plan.destination_dir, destination);
        assert_eq!(
            plan.command.args,
            vec!["receive", plan.destination_dir.to_string_lossy().as_ref()]
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn mount_plan_uses_bind_mount_and_read_only_remount() {
        let backend = BtrfsBackend::new();
        let root =
            std::env::temp_dir().join(format!("vpt-rs-btrfs-mount-{}", std::process::id()));
        let snapshot = root.join("snap");
        std::fs::create_dir_all(&snapshot).unwrap();

        let plan = backend
            .plan_mount_snapshot(&MountRequest {
                snapshot: SnapshotHandle {
                    id: snapshot.display().to_string(),
                    source: VolumeRef::new("/tmp/source"),
                },
                mode: MountMode::ReadOnly,
                target: Some(PathBuf::from("/mnt/btrfs-snapshot")),
            })
            .unwrap();

        assert_eq!(plan.commands.len(), 2);
        assert_eq!(plan.commands[0].program, MOUNT_BIN);
        assert_eq!(
            plan.commands[0].args,
            vec![
                "--bind",
                snapshot.to_string_lossy().as_ref(),
                "/mnt/btrfs-snapshot"
            ]
        );
        assert_eq!(
            plan.commands[1].args,
            vec!["-o", "remount,bind,ro", "/mnt/btrfs-snapshot"]
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn mount_plan_generates_temporary_target_when_missing() {
        let backend = BtrfsBackend::new();
        let root =
            std::env::temp_dir().join(format!("vpt-rs-btrfs-temp-mount-{}", std::process::id()));
        let snapshot = root.join("nightly snapshot");
        std::fs::create_dir_all(&snapshot).unwrap();

        let plan = backend
            .plan_mount_snapshot(&MountRequest {
                snapshot: SnapshotHandle {
                    id: snapshot.display().to_string(),
                    source: VolumeRef::new("/tmp/source"),
                },
                mode: MountMode::ReadWrite,
                target: None,
            })
            .unwrap();

        assert!(plan.auto_created_target);
        assert!(is_temporary_mount_path(&plan.mount_point));
        assert_eq!(plan.commands.len(), 1);

        let _ = std::fs::remove_dir_all(&root);
    }
}
