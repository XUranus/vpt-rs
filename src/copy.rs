use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::Path;
use std::time::Instant;

use tracing::{error, info};

use crate::error::{Error, Result};

pub const DEFAULT_BLOCK_SIZE: usize = 4 * 1024 * 1024; // 4 MiB

const PROGRESS_INTERVAL_SECS: u64 = 5;

/// Copy a source file/device to a destination file/device in fixed-size blocks.
///
/// Reports progress via `tracing::info!` every [`PROGRESS_INTERVAL_SECS`] seconds.
/// Calls `fsync` on the destination before returning.
pub fn copy_blocks(src: &Path, dst: &Path, block_size: usize) -> Result<u64> {
    if block_size == 0 {
        return Err(Error::InvalidArgument {
            message: "block_size must be greater than zero".to_string(),
        });
    }

    let mut src_file = File::open(src).map_err(|e| {
        error!(src = %src.display(), error = %e, "failed to open source");
        Error::from(e)
    })?;

    let mut dst_file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(false)
        .open(dst)
        .map_err(|e| {
            error!(dst = %dst.display(), error = %e, "failed to open destination");
            Error::from(e)
        })?;

    let mut buffer = vec![0u8; block_size];
    let mut total_bytes: u64 = 0;
    let start = Instant::now();
    let mut last_report = start;

    info!(
        src = %src.display(),
        dst = %dst.display(),
        block_size,
        "block copy started"
    );

    loop {
        let bytes_read = src_file.read(&mut buffer).map_err(|e| {
            error!(src = %src.display(), error = %e, "read failed during block copy");
            Error::from(e)
        })?;

        if bytes_read == 0 {
            break;
        }

        dst_file.write_all(&buffer[..bytes_read]).map_err(|e| {
            error!(dst = %dst.display(), error = %e, "write failed during block copy");
            Error::from(e)
        })?;

        total_bytes += bytes_read as u64;

        if last_report.elapsed().as_secs() >= PROGRESS_INTERVAL_SECS {
            let elapsed = start.elapsed().as_secs_f64();
            let rate = if elapsed > 0.0 {
                total_bytes as f64 / elapsed
            } else {
                0.0
            };
            info!(
                bytes_copied = total_bytes,
                elapsed_secs = elapsed as u64,
                rate_mbps = format!("{:.1}", rate / (1024.0 * 1024.0)),
                "block copy in progress"
            );
            last_report = Instant::now();
        }
    }

    dst_file.flush().map_err(Error::from)?;

    dst_file.sync_all().map_err(|e| {
        error!(dst = %dst.display(), error = %e, "fsync failed after block copy");
        Error::from(e)
    })?;

    let elapsed = start.elapsed().as_secs_f64();
    let rate = if elapsed > 0.0 {
        total_bytes as f64 / elapsed
    } else {
        0.0
    };
    info!(
        src = %src.display(),
        dst = %dst.display(),
        bytes_copied = total_bytes,
        elapsed_secs = format!("{:.1}", elapsed),
        rate_mbps = format!("{:.1}", rate / (1024.0 * 1024.0)),
        "block copy completed"
    );

    Ok(total_bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn copies_file_contents() {
        let dir = tempdir().unwrap();
        let src = dir.path().join("source.bin");
        let dst = dir.path().join("dest.bin");

        let data = vec![0xAB_u8; 1024 * 1024 + 7]; // ~1 MiB, non-aligned
        fs::write(&src, &data).unwrap();

        let copied = copy_blocks(&src, &dst, 64 * 1024).unwrap();
        assert_eq!(copied, data.len() as u64);
        assert_eq!(fs::read(&dst).unwrap(), data);
    }

    #[test]
    fn copies_empty_file() {
        let dir = tempdir().unwrap();
        let src = dir.path().join("empty.bin");
        let dst = dir.path().join("dest.bin");

        fs::write(&src, []).unwrap();

        let copied = copy_blocks(&src, &dst, 4096).unwrap();
        assert_eq!(copied, 0);
        assert_eq!(fs::read(&dst).unwrap(), Vec::<u8>::new());
    }

    #[test]
    fn rejects_zero_block_size() {
        let dir = tempdir().unwrap();
        let src = dir.path().join("src.bin");
        let dst = dir.path().join("dst.bin");
        fs::write(&src, b"data").unwrap();

        let err = copy_blocks(&src, &dst, 0).unwrap_err();
        assert!(matches!(err, Error::InvalidArgument { .. }));
    }
}
