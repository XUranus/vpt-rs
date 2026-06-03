use std::path::PathBuf;

/// Sanitize a user-provided label into a safe snapshot name component.
///
/// Replaces characters outside `[a-zA-Z0-9\-_.+:]` with `-`.
/// Returns `"snapshot"` if the result would be empty or all dashes.
pub fn sanitize_snapshot_label(label: &str) -> String {
    let sanitized: String = label
        .chars()
        .map(|ch| match ch {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' | '.' | '+' | ':' => ch,
            _ => '-',
        })
        .collect();

    if sanitized.trim_matches('-').is_empty() {
        "snapshot".to_string()
    } else {
        sanitized
    }
}

/// Stable identifier for a live volume, filesystem, dataset, or provider-specific source.
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

/// Snapshot consistency intent shared across providers.
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

/// Generic request for creating a provider-managed snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotRequest {
    pub source: VolumeRef,
    pub kind: SnapshotKind,
    pub label: Option<String>,
    pub read_only: bool,
}

/// Concrete snapshot handle returned by snapshot providers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotHandle {
    pub id: String,
    pub source: Option<VolumeRef>,
}

/// Reference to an existing snapshot used by backup/restore planning.
///
/// This is separate from [`SnapshotHandle`] so plans can refer to snapshots that may
/// have been created outside the current process.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SnapshotRef {
    pub id: String,
    pub origin: Option<VolumeRef>,
}

impl SnapshotRef {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            origin: None,
        }
    }

    pub fn with_origin(mut self, origin: VolumeRef) -> Self {
        self.origin = Some(origin);
        self
    }
}

impl std::fmt::Display for SnapshotRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.id)
    }
}

/// Provider-reported snapshot metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotInfo {
    pub handle: SnapshotHandle,
    pub backend: &'static str,
    pub path_hint: Option<PathBuf>,
    pub read_only: bool,
}

/// Backup destination target.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BackupTarget {
    ImageFile(PathBuf),
    Device(PathBuf),
}

/// Backup source can be either a live volume or an explicit snapshot.
///
/// Providers may support different combinations:
///
/// - Btrfs supports live volume backup with optional temporary snapshots.
/// - ZFS requires a snapshot source for send-based backup unless a temporary snapshot
///   policy is provided.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BackupSource {
    Volume(VolumeRef),
    Snapshot(SnapshotRef),
}

impl std::fmt::Display for BackupSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Volume(volume) => volume.fmt(f),
            Self::Snapshot(snapshot) => snapshot.fmt(f),
        }
    }
}

/// Policy for how a provider should obtain a snapshot for backup.
///
/// `Disabled` means the provider should use the source as-is. `Temporary` tells the
/// provider to create a temporary snapshot first when that backend supports it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SnapshotPolicy {
    Disabled,
    Temporary {
        kind: SnapshotKind,
        label: Option<String>,
        read_only: bool,
    },
}

impl SnapshotPolicy {
    pub const fn disabled() -> Self {
        Self::Disabled
    }

    pub fn temporary(kind: SnapshotKind, label: Option<String>, read_only: bool) -> Self {
        Self::Temporary {
            kind,
            label,
            read_only,
        }
    }
}

/// Provider-neutral backup/export plan.
///
/// `parent_snapshot` is used by incremental-capable providers such as Btrfs and ZFS
/// when planning send-style backups.
///
/// `block_size` controls the I/O chunk size for block-level copy operations (e.g. LVM dd).
/// `None` uses the provider default (4 MiB).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackupPlan {
    pub source: BackupSource,
    pub target: BackupTarget,
    pub snapshot_policy: SnapshotPolicy,
    pub parent_snapshot: Option<SnapshotRef>,
    pub block_size: Option<usize>,
}

/// Provider-neutral restore/import plan.
///
/// `base_snapshot` is reserved for providers that need an explicit base reference during
/// incremental restore workflows.
///
/// `block_size` controls the I/O chunk size for block-level copy operations (e.g. LVM dd).
/// `None` uses the provider default (4 MiB).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RestorePlan {
    pub source: BackupTarget,
    pub destination: VolumeRef,
    pub force: bool,
    pub base_snapshot: Option<SnapshotRef>,
    pub block_size: Option<usize>,
}

/// Mount mode for snapshot browsing or copy-mount flows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MountMode {
    ReadOnly,
    ReadWrite,
}

/// Request to mount an existing snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MountRequest {
    pub snapshot: SnapshotHandle,
    pub mode: MountMode,
    pub target: Option<PathBuf>,
}

/// Handle returned by mount-capable providers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MountHandle {
    pub id: String,
    pub mount_point: PathBuf,
}
