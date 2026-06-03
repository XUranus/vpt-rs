use crate::types::Capability;

/// Common interface shared by all backend traits.
///
/// Every backend must report its name and capabilities. The four operational traits
/// ([`SnapshotProvider`](crate::SnapshotProvider), [`BackupExecutor`](crate::BackupExecutor),
/// [`RestorePlanner`](crate::RestorePlanner), [`MountManager`](crate::MountManager)) all
/// extend this trait so callers can query capabilities without knowing which
/// specific trait a backend implements.
///
/// # Examples
///
/// ```ignore
/// use vpt_rs::{Backend, Capability};
///
/// let backend = vpt_rs::platform::current_backend();
/// println!("backend: {}", backend.backend_name());
/// assert!(backend.supports(Capability::CrashConsistentSnapshot));
/// ```
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
