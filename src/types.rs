use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct VolumeRef {
    pub id: String,
}

impl VolumeRef {
    pub fn new(id: impl Into<String>) -> Self {
        Self { id: id.into() }
    }
}

impl From<&str> for VolumeRef {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for VolumeRef {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl std::fmt::Display for VolumeRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.id)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Capability {
    CrashConsistentSnapshot,
    ApplicationConsistentSnapshot,
    WritableSnapshotMount,
    ReadOnlySnapshotMount,
    BlockLevelBackup,
    BlockLevelRestore,
    IncrementalSend,
    DirectDeviceAccess,
}

impl Capability {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CrashConsistentSnapshot => "crash_consistent_snapshot",
            Self::ApplicationConsistentSnapshot => "application_consistent_snapshot",
            Self::WritableSnapshotMount => "writable_snapshot_mount",
            Self::ReadOnlySnapshotMount => "read_only_snapshot_mount",
            Self::BlockLevelBackup => "block_level_backup",
            Self::BlockLevelRestore => "block_level_restore",
            Self::IncrementalSend => "incremental_send",
            Self::DirectDeviceAccess => "direct_device_access",
        }
    }
}

impl std::fmt::Display for Capability {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SnapshotKind {
    CrashConsistent,
    ApplicationConsistent,
}

impl SnapshotKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CrashConsistent => "crash-consistent",
            Self::ApplicationConsistent => "application-consistent",
        }
    }
}

impl std::fmt::Display for SnapshotKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for SnapshotKind {
    type Err = crate::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "crash" | "crash-consistent" => Ok(Self::CrashConsistent),
            "app" | "application" | "application-consistent" => Ok(Self::ApplicationConsistent),
            _ => Err(crate::Error::InvalidArgument {
                message: format!(
                    "unknown snapshot kind `{value}`; expected `crash` or `application`"
                ),
            }),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotRequest {
    pub source: VolumeRef,
    pub kind: SnapshotKind,
    pub label: Option<String>,
    pub read_only: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotHandle {
    pub id: String,
    pub source: VolumeRef,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotInfo {
    pub handle: SnapshotHandle,
    pub backend: &'static str,
    pub path_hint: Option<PathBuf>,
    pub read_only: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BackupTarget {
    ImageFile(PathBuf),
    Device(PathBuf),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackupPlan {
    pub source: VolumeRef,
    pub target: BackupTarget,
    pub use_snapshot: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RestorePlan {
    pub source: BackupTarget,
    pub destination: VolumeRef,
    pub force: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MountMode {
    ReadOnly,
    ReadWrite,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MountRequest {
    pub snapshot: SnapshotHandle,
    pub mode: MountMode,
    pub target: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MountHandle {
    pub id: String,
    pub mount_point: PathBuf,
}
