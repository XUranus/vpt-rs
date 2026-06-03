//! VSS FFI layer with CLI (primary) and COM API (fallback for delete).
//!
//! On desktop Windows (Home/Pro), COM's `InitializeForBackup` fails due to
//! interface version mismatches.  The CLI path (`wmic` + `vssadmin`) works
//! reliably on all editions.

pub mod cli;
pub mod com;

use tracing::{info, warn};

use crate::error::{Error, Result};
use crate::types::{SnapshotInfo, VolumeRef};

use super::BACKEND_NAME;

/// Validate that a string looks like a VSS snapshot GUID (e.g. `{12345678-ABCD-EF01-1122-334455667788}`).
/// Prevents command injection when passing IDs to wmic/vssadmin.
fn validate_snapshot_id(id: &str) -> Result<()> {
    let trimmed = id.trim();
    if trimmed.starts_with('{')
        && trimmed.ends_with('}')
        && trimmed[1..trimmed.len() - 1].split('-').count() == 5
        && trimmed[1..trimmed.len() - 1]
            .chars()
            .all(|c| c.is_ascii_hexdigit() || c == '-')
    {
        Ok(())
    } else {
        Err(Error::InvalidArgument {
            message: format!(
                "invalid snapshot ID format (expected GUID like '{{12345678-ABCD-EF01-1122-334455667788}}'): `{id}`"
            ),
        })
    }
}

/// Snapshot set result shared between COM and CLI backends.
#[derive(Debug, Clone)]
pub struct RawSnapshotSet {
    pub snapshot_set_id: String,
    pub snapshot_id: String,
    pub device_path: String,
}

/// Initialize VSS infrastructure.
pub fn initialize_requestor() -> Result<()> {
    match com::initialize() {
        Ok(()) => {
            info!("VSS COM API initialized successfully");
            Ok(())
        }
        Err(e) => {
            warn!("COM init failed ({}), using CLI-only mode", e);
            cli::initialize()
        }
    }
}

/// Create a VSS snapshot.  Uses wmic (CLI) as primary.
pub fn create_snapshot_set(
    spec: &super::VssSnapshotSpec,
    _timeouts: super::VssTimeouts,
) -> Result<RawSnapshotSet> {
    let volume = &spec.request.source.id;
    cli::create_snapshot(volume)
}

/// Commit snapshot set (no-op for CLI path).
pub fn commit_snapshot_set(raw: &mut RawSnapshotSet, _timeouts: super::VssTimeouts) -> Result<()> {
    info!(snapshot_id = %raw.snapshot_id, "VSS snapshot committed");
    Ok(())
}

/// Abort snapshot set (no-op for CLI path).
pub fn abort_snapshot_set(_raw: &mut RawSnapshotSet) -> Result<()> {
    info!("VSS snapshot abort");
    Ok(())
}

/// Delete a VSS snapshot.  Tries COM coordinator first, falls back to wmic/vssadmin.
pub fn delete_snapshot(snapshot_id: &str) -> Result<()> {
    validate_snapshot_id(snapshot_id)?;
    match com::delete_snapshot(snapshot_id) {
        Ok(()) => return Ok(()),
        Err(e) => {
            warn!("COM delete failed ({}), trying CLI fallback", e);
        }
    }
    cli::delete_snapshot(snapshot_id)
}

/// List VSS snapshots for a volume.  Uses vssadmin (locale-independent parsing).
pub fn list_snapshots(source: &VolumeRef, backend: &'static str) -> Result<Vec<SnapshotInfo>> {
    cli::list_snapshots(source, backend)
}

/// Get the device path for a VSS snapshot.
pub fn get_snapshot_device_path(snapshot_id: &str) -> Result<String> {
    validate_snapshot_id(snapshot_id)?;
    cli::get_snapshot_device_path(snapshot_id)
}
