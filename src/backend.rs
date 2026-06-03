use crate::types::Capability;

/// Common interface shared by all backend traits.
///
/// Every backend must report its name and capabilities. The four operational traits
/// (`SnapshotProvider`, `BackupExecutor`, `RestorePlanner`, `MountManager`) all
/// extend this trait so callers can query capabilities without knowing which
/// specific trait a backend implements.
pub trait Backend: Send + Sync {
    /// Return the canonical name of this backend (e.g. `"linux-btrfs"`, `"windows-vss"`).
    fn backend_name(&self) -> &'static str;

    /// Return the set of capabilities this backend supports.
    fn capabilities(&self) -> &'static [Capability];

    /// Check whether this backend supports a specific capability.
    fn supports(&self, capability: Capability) -> bool {
        self.capabilities().contains(&capability)
    }
}
