//! Core library surface for the Rust volume backup project.
//!
//! The main public model is:
//!
//! - [`BackupPlan`] for export/send style backup flows
//! - [`RestorePlan`] for import/receive style restore flows
//! - [`SnapshotRequest`] and [`SnapshotProvider`] for snapshot lifecycle operations
//!
//! `BackupPlan` now distinguishes live volume sources from explicit snapshot sources so
//! providers like Btrfs and ZFS can support file-based send/receive workflows without
//! overloading plain strings.

pub mod backup;
pub mod copy;
pub mod error;
pub mod logging;
pub mod mount;
pub mod platform;
pub mod process;
pub mod restore;
pub mod snapshot;
pub mod types;

pub use backup::BlockDeviceCopier;
pub use error::{Error, Result};
pub use mount::MountManager;
pub use platform::BackendDescriptor;
pub use restore::RestorePlanner;
pub use snapshot::SnapshotProvider;
pub use types::{
    BackupPlan, BackupSource, BackupTarget, Capability, MountHandle, MountMode, MountRequest,
    RestorePlan, SnapshotHandle, SnapshotInfo, SnapshotKind, SnapshotPolicy, SnapshotRef,
    SnapshotRequest, VolumeRef,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_platform_descriptor_is_not_empty() {
        assert!(!platform::current_platform().is_empty());
    }

    #[test]
    fn current_backend_has_name() {
        let backend = platform::current_backend();
        assert!(!backend.backend_name().is_empty());
    }

    #[test]
    fn descriptor_matches_backend_name() {
        let backend = platform::current_backend();
        let descriptor = platform::current_backend_descriptor();
        assert_eq!(descriptor.backend_name, backend.backend_name());
        assert!(!descriptor.capabilities.is_empty());
    }

    #[test]
    fn available_backends_is_not_empty() {
        assert!(!platform::available_backend_descriptors().is_empty());
    }

    #[test]
    fn snapshot_kind_parsing_accepts_short_forms() {
        assert_eq!(
            "crash".parse::<SnapshotKind>().unwrap(),
            SnapshotKind::CrashConsistent
        );
        assert_eq!(
            "application".parse::<SnapshotKind>().unwrap(),
            SnapshotKind::ApplicationConsistent
        );
    }
}
