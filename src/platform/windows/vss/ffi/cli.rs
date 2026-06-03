//! CLI-based VSS snapshot management (primary implementation for desktop Windows).
//!
//! Uses `wmic shadowcopy` for creation and device path retrieval.
//! Uses `vssadmin` for listing and deletion.
//! Works on all Windows editions (Home, Pro, Server).

use std::path::PathBuf;
use std::process::Command;

use tracing::{info, warn};

use crate::error::{Error, Result};
use crate::types::{SnapshotHandle, SnapshotInfo, VolumeRef};

// ── Public CLI functions ───────────────────────────────────────────────────

pub fn initialize() -> Result<()> {
    Ok(())
}

/// Create a VSS snapshot and return its ID and device path.
pub fn create_snapshot(volume_path: &str) -> Result<super::RawSnapshotSet> {
    let volume_wmic = format!(
        "{}\\\\",
        volume_path.trim_end_matches('\\').trim_end_matches('/')
    );

    info!(volume = %volume_path, "creating VSS snapshot via wmic");

    // Step 1: Create snapshot
    let output = Command::new("wmic")
        .args(["shadowcopy", "call", "create", &format!("Volume={volume_wmic}")])
        .output()
        .map_err(|e| Error::Message {
            message: format!("failed to run wmic: {e}"),
        })?;

    let stdout = decode_output(&output.stdout);

    if !output.status.success() || !stdout.contains("ReturnValue = 0") {
        warn!(stdout = %stdout.trim(), "wmic shadowcopy create failed");
        return Err(Error::Message {
            message: format!("wmic shadowcopy create failed: {}", stdout.trim()),
        });
    }

    let snapshot_id = parse_wmic_field(&stdout, "ShadowID")
        .ok_or_else(|| Error::Message {
            message: format!("could not parse ShadowID from wmic output:\n{stdout}"),
        })?;

    info!(snapshot_id = %snapshot_id, "VSS snapshot created via wmic");

    // Step 2: Get device path via wmic query
    let device_path = get_device_path_wmic(&snapshot_id).unwrap_or_default();

    info!(
        snapshot_id = %snapshot_id,
        device_path = %device_path,
        "VSS snapshot device path resolved"
    );

    Ok(super::RawSnapshotSet {
        snapshot_set_id: snapshot_id.clone(),
        snapshot_id,
        device_path,
    })
}

/// Delete a VSS snapshot by ID.
pub fn delete_snapshot(snapshot_id: &str) -> Result<()> {
    info!(snapshot_id = %snapshot_id, "deleting VSS snapshot via wmic");

    let output = Command::new("wmic")
        .args([
            "shadowcopy",
            "where",
            &format!("ID='{snapshot_id}'"),
            "delete",
        ])
        .output()
        .map_err(|e| Error::Message {
            message: format!("failed to run wmic: {e}"),
        })?;

    if !output.status.success() {
        let stderr = decode_output(&output.stderr);
        // Also try vssadmin as fallback
        let result = Command::new("vssadmin")
            .args([
                "delete", "shadows",
                &format!("/shadow={snapshot_id}"),
                "/quiet",
            ])
            .output();

        match result {
            Ok(o) if o.status.success() => {
                info!(snapshot_id = %snapshot_id, "VSS snapshot deleted via vssadmin fallback");
                return Ok(());
            }
            _ => {
                warn!(
                    snapshot_id = %snapshot_id,
                    wmic_err = %stderr.trim(),
                    "snapshot delete failed"
                );
                return Err(Error::Message {
                    message: format!("snapshot delete failed: {}", stderr.trim()),
                });
            }
        }
    }

    info!(snapshot_id = %snapshot_id, "VSS snapshot deleted via wmic");
    Ok(())
}

/// List VSS snapshots for a given volume.
pub fn list_snapshots(
    source: &VolumeRef,
    backend: &'static str,
) -> Result<Vec<SnapshotInfo>> {
    let volume_path = format!(
        "{}\\",
        source.id.trim_end_matches('\\').trim_end_matches('/')
    );

    let output = Command::new("vssadmin")
        .args(["list", "shadows", &format!("/for={volume_path}")])
        .output()
        .map_err(|e| Error::Message {
            message: format!("failed to run vssadmin: {e}"),
        })?;

    let stdout = decode_output(&output.stdout);
    let snapshots = parse_vssadmin_list_output(&stdout, &volume_path, backend);

    info!(volume = %volume_path, count = snapshots.len(), "VSS snapshots listed");
    Ok(snapshots)
}

/// Get the device path for a VSS snapshot.
pub fn get_snapshot_device_path(snapshot_id: &str) -> Result<String> {
    // Try wmic first (more reliable, returns DeviceObject directly)
    if let Some(path) = get_device_path_wmic(snapshot_id) {
        return Ok(path);
    }

    // Fallback to vssadmin pattern matching
    let output = Command::new("vssadmin")
        .args(["list", "shadows", &format!("/shadow={snapshot_id}")])
        .output()
        .map_err(|e| Error::Message {
            message: format!("failed to run vssadmin: {e}"),
        })?;

    let stdout = decode_output(&output.stdout);
    find_device_path(&stdout).ok_or_else(|| Error::Message {
        message: format!("could not find device path for snapshot {snapshot_id}"),
    })
}

// ── Internal helpers ───────────────────────────────────────────────────────

/// Get device path via `wmic shadowcopy where ID='{sid}' get DeviceObject`.
fn get_device_path_wmic(snapshot_id: &str) -> Option<String> {
    let output = Command::new("wmic")
        .args([
            "shadowcopy",
            "where",
            &format!("ID='{snapshot_id}'"),
            "get",
            "DeviceObject",
        ])
        .output()
        .ok()?;

    let stdout = decode_output(&output.stdout);
    for line in stdout.lines() {
        let s = line.trim();
        if s.starts_with(r"\\?\GLOBALROOT\") {
            return Some(s.to_string());
        }
    }
    None
}

fn decode_output(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).to_string()
}

fn parse_wmic_field(output: &str, field_name: &str) -> Option<String> {
    for line in output.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix(field_name) {
            let rest = rest.trim();
            if let Some(rest) = rest.strip_prefix('=') {
                let rest = rest.trim().trim_end_matches(';').trim();
                let rest = rest.trim_start_matches('"').trim_end_matches('"');
                return Some(rest.to_string());
            }
        }
    }
    None
}

/// Locale-independent vssadmin list output parser.
fn parse_vssadmin_list_output(
    output: &str,
    volume_path: &str,
    backend: &'static str,
) -> Vec<SnapshotInfo> {
    let mut snapshots = Vec::new();
    let mut current_id: Option<String> = None;
    let mut current_original: Option<String> = None;
    let mut current_device: Option<String> = None;

    for line in output.lines() {
        let line = line.trim();

        if let Some(guid) = extract_guid(line) {
            let is_indented = line.starts_with("   ") || line.starts_with("\t");
            if is_indented {
                if let Some(prev_id) = current_id.take() {
                    if matches_volume(&current_original, volume_path) {
                        snapshots.push(SnapshotInfo {
                            handle: SnapshotHandle {
                                id: prev_id,
                                source: Some(VolumeRef::new(volume_path)),
                            },
                            backend,
                            path_hint: current_device.as_ref().map(PathBuf::from),
                            read_only: true,
                        });
                    }
                }
                current_id = Some(guid);
                current_original = None;
                current_device = None;
            }
            continue;
        }

        if line.contains(r"\\?\GLOBALROOT\Device\") {
            if let Some(pos) = line.rfind(r"\\?\GLOBALROOT\Device\") {
                current_device = Some(line[pos..].trim_end().to_string());
            }
            continue;
        }

        if line.contains(r"\\?\Volume{") {
            if let Some(pos) = line.find(r"\\?\Volume{") {
                if let Some(paren_pos) = line[..pos].rfind('(') {
                    let drive = &line[paren_pos + 1..pos - 1];
                    current_original = Some(format!("{}\\", drive.trim_end_matches('\\')));
                }
            }
            continue;
        }
    }

    if let Some(id) = current_id {
        if matches_volume(&current_original, volume_path) {
            snapshots.push(SnapshotInfo {
                handle: SnapshotHandle {
                    id,
                    source: VolumeRef::new(volume_path),
                },
                backend,
                path_hint: current_device.as_ref().map(PathBuf::from),
                read_only: true,
            });
        }
    }

    snapshots
}

fn extract_guid(line: &str) -> Option<String> {
    if let Some(start) = line.find('{') {
        if let Some(end) = line[start..].find('}') {
            let candidate = &line[start..start + end + 1];
            let inner = &candidate[1..candidate.len() - 1];
            if inner.split('-').count() == 5 {
                return Some(candidate.to_string());
            }
        }
    }
    None
}

fn find_device_path(output: &str) -> Option<String> {
    for line in output.lines() {
        let line = line.trim();
        if line.contains(r"\\?\GLOBALROOT\Device\") {
            if let Some(pos) = line.rfind(r"\\?\GLOBALROOT\Device\") {
                let path = &line[pos..];
                let path = path.trim_end();
                if !path.is_empty() {
                    return Some(path.to_string());
                }
            }
        }
    }
    None
}

fn matches_volume(original: &Option<String>, target: &str) -> bool {
    match original {
        Some(vol) => vol.trim_end_matches('\\').eq_ignore_ascii_case(
            target.trim_end_matches('\\'),
        ),
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_guid_from_indented_line() {
        let line = "   Shadow Copy ID: {12345678-abcd-ef01-1122-334455667788}";
        let guid = extract_guid(line);
        assert_eq!(guid.as_deref(), Some("{12345678-abcd-ef01-1122-334455667788}"));
    }

    #[test]
    fn extract_guid_returns_none_for_no_braces() {
        assert_eq!(extract_guid("no guid here"), None);
    }

    #[test]
    fn extract_guid_returns_none_for_wrong_segment_count() {
        assert_eq!(extract_guid("{1234-5678}"), None);
    }

    #[test]
    fn parse_wmic_field_extracts_value() {
        let output = "ReturnValue = 0;\nShadowID = {ABC-123};\n";
        assert_eq!(parse_wmic_field(output, "ShadowID"), Some("{ABC-123}".to_string()));
    }

    #[test]
    fn parse_wmic_field_returns_none_for_missing_field() {
        let output = "ReturnValue = 0;\n";
        assert_eq!(parse_wmic_field(output, "ShadowID"), None);
    }

    #[test]
    fn matches_volume_case_insensitive() {
        assert!(matches_volume(&Some("C:\\".to_string()), "c:"));
        assert!(matches_volume(&Some("D:".to_string()), "D:\\"));
        assert!(!matches_volume(&Some("C:".to_string()), "D:"));
        assert!(!matches_volume(&None, "C:"));
    }

    #[test]
    fn parse_vssadmin_list_output_parses_snapshots() {
        let output = r#"Shadow Copy ID: {11111111-2222-3333-4444-555555555555}
   Original Volume: (C:)\\?\Volume{66666666-7777-8888-9999-aaaaaaaaaaaa}\
   Shadow Copy Volume: \\?\GLOBALROOT\Device\HarddiskVolumeShadowCopy1
   Originating Machine: DESKTOP
   Service Machine: DESKTOP
   Provider: Microsoft Software Shadow Copy provider 1.0
   Type: ClientAccessible
   Attributes: Auto_release

Shadow Copy ID: {aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee}
   Original Volume: (D:)\\?\Volume{ffffffff-1111-2222-3333-444444444444}\
   Shadow Copy Volume: \\?\GLOBALROOT\Device\HarddiskVolumeShadowCopy2
"#;
        let snapshots = parse_vssadmin_list_output(output, "C:\\", "windows-vss");
        assert_eq!(snapshots.len(), 1);
        assert_eq!(snapshots[0].handle.id, "{11111111-2222-3333-4444-555555555555}");
    }

    #[test]
    fn parse_vssadmin_list_output_returns_empty_for_no_match() {
        let output = r#"Shadow Copy ID: {11111111-2222-3333-4444-555555555555}
   Original Volume: (D:)\\?\Volume{66666666-7777-8888-9999-aaaaaaaaaaaa}\
"#;
        let snapshots = parse_vssadmin_list_output(output, "C:\\", "windows-vss");
        assert_eq!(snapshots.len(), 0);
    }

    #[test]
    fn find_device_path_extracts_globalroot_path() {
        let output = "   Shadow Copy Volume: \\?\GLOBALROOT\Device\HarddiskVolumeShadowCopy1\n";
        assert_eq!(
            find_device_path(output),
            Some(r"\\?\GLOBALROOT\Device\HarddiskVolumeShadowCopy1".to_string())
        );
    }

    #[test]
    fn find_device_path_returns_none_for_empty() {
        assert_eq!(find_device_path("no device path here"), None);
    }
}
