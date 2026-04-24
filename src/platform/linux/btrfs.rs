use std::io::{BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::backup::BlockDeviceCopier;
use crate::error::{Error, Result};
use crate::mount::MountManager;
use crate::platform::StubBackend;
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
    Capability::IncrementalSend,
];

const BTRFS_BIN: &str = "btrfs";

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

    fn display(&self) -> String {
        if self.args.is_empty() {
            self.program.to_string()
        } else {
            format!("{} {}", self.program, self.args.join(" "))
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
    pub command: BtrfsCommand,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BtrfsReceivePlan {
    pub stream: PathBuf,
    pub destination_dir: PathBuf,
    pub command: BtrfsCommand,
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
        let source = self.volume_path(&plan.source)?;
        if !source.exists() {
            return Err(Error::MissingPath { path: source });
        }

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

        let command = BtrfsCommand::new(vec!["send".to_string(), source.display().to_string()]);

        Ok(BtrfsSendPlan {
            source,
            target,
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

    fn run_command(&self, command: &BtrfsCommand) -> Result<std::process::Output> {
        let output = Command::new(command.program).args(&command.args).output()?;
        if output.status.success() {
            Ok(output)
        } else {
            Err(Error::CommandFailed {
                command: command.display(),
                status: output.status.code().unwrap_or(-1),
                stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
            })
        }
    }

    fn run_send(&self, plan: &BtrfsSendPlan) -> Result<()> {
        if let Some(parent) = plan.target.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let mut child = Command::new(plan.command.program)
            .args(&plan.command.args)
            .stdout(Stdio::piped())
            .spawn()?;

        let stdout = child.stdout.take().ok_or_else(|| Error::Io {
            message: "failed to capture btrfs send stdout".to_string(),
        })?;
        let mut reader = BufReader::new(stdout);
        let file = std::fs::File::create(&plan.target)?;
        let mut writer = BufWriter::new(file);
        std::io::copy(&mut reader, &mut writer)?;
        writer.flush()?;

        let status = child.wait()?;
        if status.success() {
            Ok(())
        } else {
            Err(Error::CommandFailed {
                command: plan.command.display(),
                status: status.code().unwrap_or(-1),
                stderr: String::new(),
            })
        }
    }

    fn run_receive(&self, plan: &BtrfsReceivePlan) -> Result<()> {
        let input = std::fs::File::open(&plan.stream)?;
        let mut child = Command::new(plan.command.program)
            .args(&plan.command.args)
            .stdin(Stdio::piped())
            .spawn()?;

        let stdin = child.stdin.take().ok_or_else(|| Error::Io {
            message: "failed to open btrfs receive stdin".to_string(),
        })?;
        let mut writer = BufWriter::new(stdin);
        let mut reader = BufReader::new(input);
        std::io::copy(&mut reader, &mut writer)?;
        writer.flush()?;
        drop(writer);

        let status = child.wait()?;
        if status.success() {
            Ok(())
        } else {
            Err(Error::CommandFailed {
                command: plan.command.display(),
                status: status.code().unwrap_or(-1),
                stderr: String::new(),
            })
        }
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
    }

    fn delete_snapshot(&self, snapshot: &SnapshotHandle) -> Result<()> {
        let command = self.plan_delete_snapshot(snapshot)?;
        self.run_command(&command)?;
        Ok(())
    }

    fn list_snapshots(&self, source: &VolumeRef) -> Result<Vec<SnapshotInfo>> {
        let command = self.plan_list_snapshots(source)?;
        let output = self.run_command(&command)?;
        Ok(self.parse_list_output(source, &output.stdout))
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
        let send_plan = self.plan_backup(plan)?;
        self.run_send(&send_plan)
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
        let receive_plan = self.plan_restore(plan)?;
        self.run_receive(&receive_plan)
    }
}

impl MountManager for BtrfsBackend {
    fn backend_name(&self) -> &'static str {
        self.0.backend_name()
    }

    fn capabilities(&self) -> &'static [Capability] {
        self.0.capabilities()
    }

    fn mount_snapshot(&self, _request: &MountRequest) -> Result<MountHandle> {
        Err(Error::UnsupportedOperation {
            operation: "mount_snapshot",
            backend: self.backend_name(),
        })
    }

    fn unmount(&self, _handle: &MountHandle) -> Result<()> {
        Err(Error::UnsupportedOperation {
            operation: "unmount",
            backend: self.backend_name(),
        })
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

#[cfg(test)]
mod tests {
    use super::*;

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
                source: VolumeRef::new(source.display().to_string()),
                target: crate::types::BackupTarget::ImageFile(target.clone()),
                use_snapshot: true,
            })
            .unwrap();

        assert_eq!(plan.source, source);
        assert_eq!(plan.target, target);
        assert_eq!(
            plan.command.args,
            vec!["send", plan.source.to_string_lossy().as_ref()]
        );

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
}
