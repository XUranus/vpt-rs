use std::path::PathBuf;
use std::process::ExitCode;
use tracing::{error, info};

use vpt_rs::backup::BlockDeviceCopier;
use vpt_rs::logging;
use vpt_rs::platform;
use vpt_rs::{
    BackupPlan, BackupSource, BackupTarget, SnapshotKind, SnapshotPolicy, SnapshotRef, VolumeRef,
};

fn main() -> ExitCode {
    logging::init_logging();
    info!(tool = "vb-backup", "cli started");
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            error!(tool = "vb-backup", error = %error, timeout_secs = error.timeout_secs(), "cli failed");
            eprintln!("error: {error}");
            ExitCode::from(1)
        }
    }
}

fn run() -> vpt_rs::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        print_usage();
        return Ok(());
    }

    let request = parse_backup_request(args)?;
    let backend = resolve_backend(request.provider.as_deref())?;
    backend.backup_volume(&BackupPlan {
        source: request.source,
        target: BackupTarget::ImageFile(request.output.clone()),
        snapshot_policy: request.snapshot_policy,
        parent_snapshot: request.parent_snapshot,
    })?;

    println!("backend: {}", backend.backend_name());
    println!("output: {}", request.output.display());
    Ok(())
}

struct BackupRequest {
    provider: Option<String>,
    source: BackupSource,
    output: PathBuf,
    snapshot_policy: SnapshotPolicy,
    parent_snapshot: Option<SnapshotRef>,
}

fn parse_backup_request(args: Vec<String>) -> vpt_rs::Result<BackupRequest> {
    let mut provider = None;
    let mut source = None;
    let mut output = None;
    let mut snapshot_source = false;
    let mut snapshot_kind = SnapshotKind::CrashConsistent;
    let mut snapshot_label = None;
    let mut snapshot_read_only = true;
    let mut snapshot_enabled = true;
    let mut parent_snapshot = None;

    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--provider" => {
                let Some(value) = iter.next() else {
                    return missing("--provider");
                };
                provider = Some(value);
            }
            "--output" => {
                let Some(value) = iter.next() else {
                    return missing("--output");
                };
                output = Some(PathBuf::from(value));
            }
            "--snapshot-source" => {
                snapshot_source = true;
            }
            "--parent-snapshot" => {
                let Some(value) = iter.next() else {
                    return missing("--parent-snapshot");
                };
                parent_snapshot = Some(SnapshotRef::new(value));
            }
            "--snapshot-label" => {
                let Some(value) = iter.next() else {
                    return missing("--snapshot-label");
                };
                snapshot_label = Some(value);
            }
            "--snapshot-kind" => {
                let Some(value) = iter.next() else {
                    return missing("--snapshot-kind");
                };
                snapshot_kind = value.parse()?;
            }
            "--snapshot-read-write" => {
                snapshot_read_only = false;
            }
            "--no-snapshot" => {
                snapshot_enabled = false;
            }
            "--help" | "-h" | "help" => {
                print_usage();
                std::process::exit(0);
            }
            value if source.is_none() => {
                source = Some(VolumeRef::new(value));
            }
            _ => {
                return Err(vpt_rs::Error::InvalidArgument {
                    message: format!("unexpected argument `{arg}`"),
                });
            }
        }
    }

    let Some(source) = source else {
        return Err(vpt_rs::Error::InvalidArgument {
            message: "missing source volume".to_string(),
        });
    };
    let Some(output) = output else {
        return Err(vpt_rs::Error::InvalidArgument {
            message: "missing `--output <path>`".to_string(),
        });
    };

    Ok(BackupRequest {
        provider,
        source: if snapshot_source {
            let volume = source;
            let snapshot_id = volume.id.clone();
            BackupSource::Snapshot(SnapshotRef::new(snapshot_id).with_origin(volume))
        } else {
            BackupSource::Volume(source)
        },
        output,
        snapshot_policy: if snapshot_enabled {
            SnapshotPolicy::temporary(snapshot_kind, snapshot_label, snapshot_read_only)
        } else {
            SnapshotPolicy::disabled()
        },
        parent_snapshot,
    })
}

fn resolve_backend(provider: Option<&str>) -> vpt_rs::Result<platform::CurrentBackend> {
    #[cfg(target_os = "linux")]
    {
        if let Some(name) = provider {
            return platform::CurrentBackend::named(name);
        }
    }

    #[allow(unreachable_code)]
    {
        if let Some(name) = provider {
            return Err(vpt_rs::Error::InvalidArgument {
                message: format!("provider selection is not supported on this platform: `{name}`"),
            });
        }
        Ok(platform::current_backend())
    }
}

fn missing(flag: &str) -> vpt_rs::Result<BackupRequest> {
    Err(vpt_rs::Error::InvalidArgument {
        message: format!("missing value after `{flag}`"),
    })
}

fn print_usage() {
    println!(
        "vb-backup <source> --output <stream-file> [--provider <name>] [--snapshot-source] [--parent-snapshot <id>] [--snapshot-kind crash|application] [--snapshot-label <name>] [--snapshot-read-write] [--no-snapshot]"
    );
}
