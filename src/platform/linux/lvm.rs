use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{error, info};

use crate::backup::BlockDeviceCopier;
use crate::copy;
use crate::error::{Error, Result};
use crate::mount::MountManager;
use crate::platform::StubBackend;
use crate::process::{self, CommandIo};
use crate::restore::RestorePlanner;
use crate::snapshot::SnapshotProvider;
use crate::types::{
    BackupPlan, Capability, MountHandle, MountRequest, RestorePlan, SnapshotHandle, SnapshotInfo,
    SnapshotKind, SnapshotRequest, VolumeRef,
};

const CAPABILITIES: &[Capability] = &[
    Capability::CrashConsistentSnapshot,
    Capability::ReadOnlySnapshotMount,
    Capability::WritableSnapshotMount,
    Capability::BlockLevelBackup,
    Capability::BlockLevelRestore,
    Capability::DirectDeviceAccess,
];

const LVCREATE_BIN: &str = "lvcreate";
const LVREMOVE_BIN: &str = "lvremove";
const LVCHANGE_BIN: &str = "lvchange";
const LVS_BIN: &str = "lvs";
const DEFAULT_SNAPSHOT_SIZE: &str = "20%ORIGIN";

#[derive(Debug, Clone)]
pub struct LvmBackend(StubBackend);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LvmVolumeRef {
    pub vg_name: String,
    pub lv_name: String,
    pub lv_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LvmCommand {
    pub program: &'static str,
    pub args: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LvmSnapshotPlan {
    pub source: LvmVolumeRef,
    pub snapshot_name: String,
    pub snapshot_path: PathBuf,
    pub read_only: bool,
    pub commands: Vec<LvmCommand>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LvmBackupPlan {
    pub source: LvmVolumeRef,
    pub target: PathBuf,
    pub temporary_snapshot: Option<LvmSnapshotPlan>,
    pub copy_src: PathBuf,
    pub copy_dst: PathBuf,
    pub block_size: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LvmRestorePlan {
    pub source: PathBuf,
    pub destination: LvmVolumeRef,
    pub force: bool,
    pub copy_src: PathBuf,
    pub copy_dst: PathBuf,
    pub block_size: usize,
}

impl LvmCommand {
    fn new(program: &'static str, args: impl IntoIterator<Item = String>) -> Self {
        Self {
            program,
            args: args.into_iter().collect(),
        }
    }
}

impl LvmBackend {
    pub fn new() -> Self {
        Self(StubBackend::new("linux-lvm", CAPABILITIES))
    }

    pub fn backend_name(&self) -> &'static str {
        self.0.backend_name()
    }

    pub fn parse_volume_ref(&self, source: &VolumeRef) -> Result<LvmVolumeRef> {
        let raw = source.id.trim();
        if raw.is_empty() {
            return Err(Error::InvalidVolume {
                volume: source.id.clone(),
            });
        }

        let path = Path::new(raw);
        if !path.is_absolute() {
            return Err(Error::InvalidArgument {
                message: format!("lvm provider expects an absolute LV path, got `{raw}`"),
            });
        }

        let components: Vec<_> = path
            .iter()
            .map(|segment| segment.to_string_lossy().to_string())
            .collect();

        if components.len() != 4 || components[1] != "dev" {
            return Err(Error::InvalidArgument {
                message: format!(
                    "lvm provider expects `/dev/<vg>/<lv>` paths, got `{}`",
                    path.display()
                ),
            });
        }

        Ok(LvmVolumeRef {
            vg_name: components[2].clone(),
            lv_name: components[3].clone(),
            lv_path: path.to_path_buf(),
        })
    }

    pub fn plan_create_snapshot(&self, request: &SnapshotRequest) -> Result<LvmSnapshotPlan> {
        if matches!(request.kind, SnapshotKind::ApplicationConsistent) {
            return Err(Error::MissingCapability {
                capability: Capability::ApplicationConsistentSnapshot.as_str(),
                backend: self.backend_name(),
            });
        }

        let source = self.parse_volume_ref(&request.source)?;
        let snapshot_name = derive_snapshot_name(&source.lv_name, request.label.as_deref());
        let snapshot_path = PathBuf::from(format!("/dev/{}/{}", source.vg_name, snapshot_name));

        let mut commands = vec![LvmCommand::new(
            LVCREATE_BIN,
            vec![
                "--snapshot".to_string(),
                "--extents".to_string(),
                DEFAULT_SNAPSHOT_SIZE.to_string(),
                "--name".to_string(),
                snapshot_name.clone(),
                source.lv_path.display().to_string(),
            ],
        )];

        if request.read_only {
            commands.push(LvmCommand::new(
                LVCHANGE_BIN,
                vec![
                    "--permission".to_string(),
                    "r".to_string(),
                    snapshot_path.display().to_string(),
                ],
            ));
        }

        Ok(LvmSnapshotPlan {
            source,
            snapshot_name,
            snapshot_path,
            read_only: request.read_only,
            commands,
        })
    }

    pub fn plan_delete_snapshot(&self, snapshot: &SnapshotHandle) -> Result<LvmCommand> {
        let snapshot_path = PathBuf::from(snapshot.id.trim());
        if snapshot.id.trim().is_empty() {
            return Err(Error::InvalidArgument {
                message: "snapshot id must not be empty".to_string(),
            });
        }

        Ok(LvmCommand::new(
            LVREMOVE_BIN,
            vec!["--yes".to_string(), snapshot_path.display().to_string()],
        ))
    }

    pub fn plan_list_snapshots(&self, source: &VolumeRef) -> Result<(LvmVolumeRef, LvmCommand)> {
        let volume = self.parse_volume_ref(source)?;
        let command = LvmCommand::new(
            LVS_BIN,
            vec![
                "--noheadings".to_string(),
                "--separator".to_string(),
                "|".to_string(),
                "--options".to_string(),
                "lv_name,origin,lv_path,lv_attr".to_string(),
                volume.vg_name.clone(),
            ],
        );

        Ok((volume, command))
    }

    pub fn plan_backup(&self, plan: &BackupPlan) -> Result<LvmBackupPlan> {
        let source = match &plan.source {
            crate::types::BackupSource::Volume(source) => self.parse_volume_ref(source)?,
            crate::types::BackupSource::Snapshot(snapshot) => {
                self.parse_volume_ref(&VolumeRef::new(snapshot.id.clone()))?
            }
        };

        let target = match &plan.target {
            crate::types::BackupTarget::ImageFile(path) => path.clone(),
            crate::types::BackupTarget::Device(path) => {
                return Err(Error::InvalidArgument {
                    message: format!(
                        "lvm backup currently supports only image-file targets, got `{}`",
                        path.display()
                    ),
                });
            }
        };

        let temporary_snapshot = match (&plan.source, &plan.snapshot_policy) {
            (crate::types::BackupSource::Volume(source), crate::types::SnapshotPolicy::Temporary { kind, label, .. }) => {
                Some(self.plan_create_snapshot(&SnapshotRequest {
                    source: source.clone(),
                    kind: *kind,
                    label: label.clone(),
                    read_only: true,
                })?)
            }
            _ => None,
        };

        let copy_src = temporary_snapshot
            .as_ref()
            .map(|snapshot| snapshot.snapshot_path.clone())
            .unwrap_or_else(|| source.lv_path.clone());

        let block_size = plan.block_size.unwrap_or(copy::DEFAULT_BLOCK_SIZE);

        Ok(LvmBackupPlan {
            copy_dst: target.clone(),
            source,
            target,
            temporary_snapshot,
            copy_src,
            block_size,
        })
    }

    pub fn plan_restore(&self, plan: &RestorePlan) -> Result<LvmRestorePlan> {
        let source = match &plan.source {
            crate::types::BackupTarget::ImageFile(path) => path.clone(),
            crate::types::BackupTarget::Device(path) => {
                return Err(Error::InvalidArgument {
                    message: format!(
                        "lvm restore currently supports only image-file sources, got `{}`",
                        path.display()
                    ),
                });
            }
        };

        if !plan.force {
            return Err(Error::InvalidArgument {
                message: "lvm restore requires `--force` because it overwrites the destination logical volume".to_string(),
            });
        }

        let destination = self.parse_volume_ref(&plan.destination)?;
        let block_size = plan.block_size.unwrap_or(copy::DEFAULT_BLOCK_SIZE);

        Ok(LvmRestorePlan {
            copy_src: source.clone(),
            copy_dst: destination.lv_path.clone(),
            source,
            destination,
            force: plan.force,
            block_size,
        })
    }

    fn run_command(&self, command: &LvmCommand) -> Result<std::process::Output> {
        process::run_command(
            self.backend_name(),
            "run_command",
            command.program,
            &command.args,
            CommandIo::default(),
        )
    }

    fn parse_list_output(&self, source: &LvmVolumeRef, stdout: &[u8]) -> Vec<SnapshotInfo> {
        let mut snapshots = Vec::new();

        for line in String::from_utf8_lossy(stdout).lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            let parts: Vec<_> = line.split('|').map(str::trim).collect();
            if parts.len() != 4 {
                continue;
            }

            let _lv_name = parts[0];
            let origin = parts[1];
            let lv_path = parts[2];
            let lv_attr = parts[3];

            if origin != source.lv_name {
                continue;
            }

            if !lv_attr.starts_with('s') && !lv_attr.starts_with('S') {
                continue;
            }

            let path = PathBuf::from(lv_path);
            snapshots.push(SnapshotInfo {
                handle: SnapshotHandle {
                    id: path.display().to_string(),
                    source: VolumeRef::new(source.lv_path.display().to_string()),
                },
                backend: self.backend_name(),
                path_hint: Some(path),
                read_only: lv_attr.contains('r'),
            });
        }

        snapshots
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
        info!(backend = self.backend_name(), source = %request.source, read_only = request.read_only, "create_snapshot called");
        let result = (|| {
            let plan = self.plan_create_snapshot(request)?;
            for command in &plan.commands {
                self.run_command(command)?;
            }

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
            let (parsed_source, command) = self.plan_list_snapshots(source)?;
            let output = self.run_command(&command)?;
            Ok(self.parse_list_output(&parsed_source, &output.stdout))
        })();
        if let Err(error) = &result {
            error!(backend = self.backend_name(), source = %source, error = %error, "list_snapshots failed");
        }
        result
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
        info!(backend = self.backend_name(), source = %plan.source, "backup_volume called");
        let result = (|| {
            let plan = self.plan_backup(plan)?;

            if let Some(snapshot) = &plan.temporary_snapshot {
                for command in &snapshot.commands {
                    self.run_command(command)?;
                }
            }

            let copy_result = copy::copy_blocks(&plan.copy_src, &plan.copy_dst, plan.block_size).map(|_| ());
            let cleanup_result = if let Some(snapshot) = &plan.temporary_snapshot {
                self.run_command(&LvmCommand::new(
                    LVREMOVE_BIN,
                    vec![
                        "--yes".to_string(),
                        snapshot.snapshot_path.display().to_string(),
                    ],
                ))
                .map(|_| ())
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
                        source = %plan.source.lv_path.display(),
                        cleanup_error = %cleanup_error,
                        "backup cleanup failed after copy error"
                    );
                    Err(error)
                }
            }
        })();
        if let Err(error) = &result {
            error!(backend = self.backend_name(), source = %plan.source, error = %error, "backup_volume failed");
        }
        result
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
        info!(backend = self.backend_name(), destination = %plan.destination, "restore_volume called");
        let result = (|| {
            let plan = self.plan_restore(plan)?;
            copy::copy_blocks(&plan.copy_src, &plan.copy_dst, plan.block_size)?;
            Ok(())
        })();
        if let Err(error) = &result {
            error!(backend = self.backend_name(), destination = %plan.destination, error = %error, "restore_volume failed");
        }
        result
    }
}

impl MountManager for LvmBackend {
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

fn derive_snapshot_name(lv_name: &str, label: Option<&str>) -> String {
    match label {
        Some(label) => sanitize_snapshot_segment(label),
        None => {
            let ts = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|duration| duration.as_secs())
                .unwrap_or(0);
            format!("{lv_name}-snap-{ts}")
        }
    }
}

fn sanitize_snapshot_segment(value: &str) -> String {
    let sanitized: String = value
        .chars()
        .map(|ch| match ch {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' | '+' | '.' => ch,
            _ => '-',
        })
        .collect();

    if sanitized.trim_matches('-').is_empty() {
        "snapshot".to_string()
    } else {
        sanitized
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{BackupSource, BackupTarget, SnapshotPolicy};

    #[test]
    fn parses_standard_lvm_volume_path() {
        let backend = LvmBackend::new();
        let volume = backend
            .parse_volume_ref(&VolumeRef::new("/dev/vg0/data"))
            .unwrap();

        assert_eq!(volume.vg_name, "vg0");
        assert_eq!(volume.lv_name, "data");
        assert_eq!(volume.lv_path, PathBuf::from("/dev/vg0/data"));
    }

    #[test]
    fn create_plan_uses_lvcreate_snapshot_commands() {
        let backend = LvmBackend::new();
        let plan = backend
            .plan_create_snapshot(&SnapshotRequest {
                source: VolumeRef::new("/dev/vg0/data"),
                kind: SnapshotKind::CrashConsistent,
                label: Some("nightly backup".to_string()),
                read_only: true,
            })
            .unwrap();

        assert_eq!(plan.snapshot_name, "nightly-backup");
        assert_eq!(plan.snapshot_path, PathBuf::from("/dev/vg0/nightly-backup"));
        assert_eq!(plan.commands.len(), 2);
        assert_eq!(
            plan.commands[0].args,
            vec![
                "--snapshot",
                "--extents",
                "20%ORIGIN",
                "--name",
                "nightly-backup",
                "/dev/vg0/data",
            ]
        );
        assert_eq!(
            plan.commands[1].args,
            vec!["--permission", "r", "/dev/vg0/nightly-backup"]
        );
    }

    #[test]
    fn parse_list_output_filters_origin_snapshots() {
        let backend = LvmBackend::new();
        let source = LvmVolumeRef {
            vg_name: "vg0".to_string(),
            lv_name: "data".to_string(),
            lv_path: PathBuf::from("/dev/vg0/data"),
        };

        let snapshots = backend.parse_list_output(
            &source,
            br#"
snap1|data|/dev/vg0/snap1|swi-a-s---
other|other-origin|/dev/vg0/other|swi-a-s---
snap2|data|/dev/vg0/snap2|Swi-a-r---
"#,
        );

        assert_eq!(snapshots.len(), 2);
        assert_eq!(snapshots[0].handle.id, "/dev/vg0/snap1");
        assert!(!snapshots[0].read_only);
        assert_eq!(snapshots[1].handle.id, "/dev/vg0/snap2");
        assert!(snapshots[1].read_only);
    }

    #[test]
    fn application_consistent_requests_are_rejected() {
        let backend = LvmBackend::new();
        let error = backend
            .plan_create_snapshot(&SnapshotRequest {
                source: VolumeRef::new("/dev/vg0/data"),
                kind: SnapshotKind::ApplicationConsistent,
                label: None,
                read_only: true,
            })
            .unwrap_err();

        assert!(matches!(error, Error::MissingCapability { .. }));
    }

    #[test]
    fn backup_plan_uses_temporary_snapshot_for_live_volume() {
        let backend = LvmBackend::new();
        let plan = backend
            .plan_backup(&BackupPlan {
                source: BackupSource::Volume(VolumeRef::new("/dev/vg0/data")),
                target: BackupTarget::ImageFile(PathBuf::from("/tmp/data.img")),
                snapshot_policy: SnapshotPolicy::temporary(
                    SnapshotKind::CrashConsistent,
                    Some("backup snap".to_string()),
                    true,
                ),
                parent_snapshot: None,
                block_size: None,
            })
            .unwrap();

        let snapshot = plan.temporary_snapshot.expect("temporary snapshot");
        assert_eq!(snapshot.snapshot_path, PathBuf::from("/dev/vg0/backup-snap"));
        assert_eq!(plan.copy_src, PathBuf::from("/dev/vg0/backup-snap"));
        assert_eq!(plan.copy_dst, PathBuf::from("/tmp/data.img"));
        assert_eq!(plan.block_size, copy::DEFAULT_BLOCK_SIZE);
    }

    #[test]
    fn backup_plan_uses_explicit_snapshot_source_without_temporary_snapshot() {
        let backend = LvmBackend::new();
        let plan = backend
            .plan_backup(&BackupPlan {
                source: BackupSource::Snapshot(
                    crate::types::SnapshotRef::new("/dev/vg0/snap1")
                        .with_origin(VolumeRef::new("/dev/vg0/data")),
                ),
                target: BackupTarget::ImageFile(PathBuf::from("/tmp/snap.img")),
                snapshot_policy: SnapshotPolicy::disabled(),
                parent_snapshot: None,
                block_size: None,
            })
            .unwrap();

        assert!(plan.temporary_snapshot.is_none());
        assert_eq!(plan.copy_src, PathBuf::from("/dev/vg0/snap1"));
    }

    #[test]
    fn restore_plan_requires_force_flag() {
        let backend = LvmBackend::new();
        let error = backend
            .plan_restore(&RestorePlan {
                source: BackupTarget::ImageFile(PathBuf::from("/tmp/data.img")),
                destination: VolumeRef::new("/dev/vg0/restore"),
                force: false,
                base_snapshot: None,
                block_size: None,
            })
            .unwrap_err();

        assert!(matches!(error, Error::InvalidArgument { .. }));
    }

    #[test]
    fn restore_plan_uses_copy_blocks_to_write_image_into_lv() {
        let backend = LvmBackend::new();
        let plan = backend
            .plan_restore(&RestorePlan {
                source: BackupTarget::ImageFile(PathBuf::from("/tmp/data.img")),
                destination: VolumeRef::new("/dev/vg0/restore"),
                force: true,
                base_snapshot: None,
                block_size: None,
            })
            .unwrap();

        assert_eq!(plan.destination.lv_path, PathBuf::from("/dev/vg0/restore"));
        assert_eq!(plan.copy_src, PathBuf::from("/tmp/data.img"));
        assert_eq!(plan.copy_dst, PathBuf::from("/dev/vg0/restore"));
        assert_eq!(plan.block_size, copy::DEFAULT_BLOCK_SIZE);
    }
}
