use std::path::PathBuf;
use std::process::ExitCode;
use tracing::{error, info};

use vpt_rs::Backend;
use vpt_rs::backup::BackupExecutor;
use vpt_rs::logging;
use vpt_rs::platform;
use vpt_rs::restore::RestorePlanner;
use vpt_rs::{
    BackupPlan, BackupSource, BackupTarget, RestorePlan, SnapshotKind, SnapshotPolicy,
    SnapshotProvider, SnapshotRef, SnapshotRequest, VolumeRef,
};

fn main() -> ExitCode {
    logging::init_logging();
    info!(tool = "vptcli", "cli started");
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            error!(tool = "vptcli", error = %err, timeout_secs = err.timeout_secs(), "cli failed");
            eprintln!("error: {err}");
            ExitCode::from(1)
        }
    }
}

fn run() -> vpt_rs::Result<()> {
    let mut args = std::env::args().skip(1);
    let Some(command) = args.next() else {
        print_usage();
        return Ok(());
    };

    let remaining: Vec<String> = args.collect();
    match command.as_str() {
        "snapshot" => run_snapshot(remaining),
        "backup" => run_backup(remaining),
        "restore" => run_restore(remaining),
        "help" | "--help" | "-h" => {
            print_usage();
            Ok(())
        }
        _ => Err(vpt_rs::Error::InvalidArgument {
            message: format!("unknown command `{command}`"),
        }),
    }
}

fn print_usage() {
    println!("vptcli <command> [args]");
    println!();
    println!("Commands:");
    println!("  snapshot    Create, list, delete snapshots; query backends and capabilities");
    println!("  backup      Back up a volume to a stream or image file");
    println!("  restore     Restore a volume from a stream or image file");
    println!();
    println!("Run `vptcli <command>` with no args for subcommand usage.");
}

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

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
            // On non-Linux platforms, accept the platform's native backend name
            let backend = platform::current_backend();
            if name == backend.backend_name() {
                return Ok(backend);
            }
            return Err(vpt_rs::Error::InvalidArgument {
                message: format!("provider selection is not supported on this platform: `{name}`"),
            });
        }
        Ok(platform::current_backend())
    }
}

fn missing(flag: &str) -> vpt_rs::Error {
    vpt_rs::Error::InvalidArgument {
        message: format!("missing value after `{flag}`"),
    }
}

fn parse_block_size(value: &str) -> vpt_rs::Result<usize> {
    let value = value.trim();
    if value.is_empty() {
        return Err(vpt_rs::Error::InvalidArgument {
            message: "block size must not be empty".to_string(),
        });
    }

    let last = value.as_bytes()[value.len() - 1];
    let (num_str, multiplier) = match last {
        b'K' | b'k' => (&value[..value.len() - 1], 1024usize),
        b'M' | b'm' => (&value[..value.len() - 1], 1024 * 1024),
        b'G' | b'g' => (&value[..value.len() - 1], 1024 * 1024 * 1024),
        _ => (value, 1),
    };

    let num: usize = num_str
        .parse()
        .map_err(|_| vpt_rs::Error::InvalidArgument {
            message: format!(
                "invalid block size `{value}`; expected a number with optional K/M/G suffix"
            ),
        })?;

    let size = num
        .checked_mul(multiplier)
        .ok_or_else(|| vpt_rs::Error::InvalidArgument {
            message: format!("block size `{value}` overflows"),
        })?;
    if size == 0 {
        return Err(vpt_rs::Error::InvalidArgument {
            message: "block size must be greater than zero".to_string(),
        });
    }

    Ok(size)
}

// ---------------------------------------------------------------------------
// vptcli snapshot
// ---------------------------------------------------------------------------

fn run_snapshot(args: Vec<String>) -> vpt_rs::Result<()> {
    if args.is_empty() {
        print_snapshot_usage();
        return Ok(());
    }

    let mut args = args.into_iter();
    let command = args.next().unwrap();
    let remaining: Vec<String> = args.collect();

    match command.as_str() {
        "backend" => match remaining.first().map(|s| s.as_str()) {
            Some("list") => {
                for descriptor in platform::available_backend_descriptors() {
                    print_descriptor(&descriptor);
                }
                Ok(())
            }
            None => {
                let descriptor = platform::current_backend_descriptor();
                print_descriptor(&descriptor);
                Ok(())
            }
            Some(other) => Err(vpt_rs::Error::InvalidArgument {
                message: format!("unknown backend subcommand `{other}`"),
            }),
        },
        "capabilities" => {
            let provider = parse_provider_flag(remaining)?;
            let backend = resolve_backend(provider.as_deref())?;
            let descriptor = backend_descriptor(&backend);
            println!("{}", descriptor.backend_name);
            for capability in descriptor.capabilities {
                println!("- {capability}");
            }
            Ok(())
        }
        "create" => {
            let (provider, request) = parse_create_request(remaining)?;
            let backend = resolve_backend(provider.as_deref())?;
            let snapshot = backend.create_snapshot(&request)?;
            println!("snapshot: {}", snapshot.handle.id);
            if let Some(source) = &snapshot.handle.source {
                println!("source: {}", source);
            }
            println!("backend: {}", snapshot.backend);
            if let Some(path_hint) = snapshot.path_hint {
                println!("path: {}", path_hint.display());
            }
            Ok(())
        }
        "list" => snapshot_list(remaining),
        "delete" => snapshot_delete(remaining),
        "help" | "--help" | "-h" => {
            print_snapshot_usage();
            Ok(())
        }
        _ => Err(vpt_rs::Error::InvalidArgument {
            message: format!("unknown snapshot command `{command}`"),
        }),
    }
}

fn snapshot_list(args: Vec<String>) -> vpt_rs::Result<()> {
    let mut provider = None;
    let mut volume = None;

    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--provider" => {
                let value = iter.next().ok_or_else(|| vpt_rs::Error::InvalidArgument {
                    message: "missing value after `--provider`".to_string(),
                })?;
                provider = Some(value);
            }
            value if volume.is_none() => volume = Some(value.to_string()),
            _ => {
                return Err(vpt_rs::Error::InvalidArgument {
                    message: format!("unexpected argument `{arg}`"),
                });
            }
        }
    }

    let volume = volume.ok_or_else(|| vpt_rs::Error::InvalidArgument {
        message: "missing volume id for `list`".to_string(),
    })?;

    let backend = resolve_backend(provider.as_deref())?;
    let snapshots = backend.list_snapshots(&VolumeRef::new(volume))?;
    for snapshot in snapshots {
        let source_display = snapshot
            .handle
            .source
            .as_ref()
            .map(|s| s.id.as_str())
            .unwrap_or("-");
        println!(
            "{} {} {}",
            snapshot.handle.id, source_display, snapshot.backend
        );
    }
    Ok(())
}

fn snapshot_delete(args: Vec<String>) -> vpt_rs::Result<()> {
    let mut provider = None;
    let mut snapshot_id = None;

    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--provider" => {
                let value = iter.next().ok_or_else(|| vpt_rs::Error::InvalidArgument {
                    message: "missing value after `--provider`".to_string(),
                })?;
                provider = Some(value);
            }
            value if snapshot_id.is_none() => snapshot_id = Some(value.to_string()),
            _ => {
                return Err(vpt_rs::Error::InvalidArgument {
                    message: format!("unexpected argument `{arg}`"),
                });
            }
        }
    }

    let snapshot_id = snapshot_id.ok_or_else(|| vpt_rs::Error::InvalidArgument {
        message: "missing snapshot id for `delete`".to_string(),
    })?;

    let backend = resolve_backend(provider.as_deref())?;
    backend.delete_snapshot(&vpt_rs::SnapshotHandle {
        id: snapshot_id,
        source: None,
    })?;
    Ok(())
}

fn parse_create_request(args: Vec<String>) -> vpt_rs::Result<(Option<String>, SnapshotRequest)> {
    let mut provider = None;
    let mut volume = None;
    let mut kind = SnapshotKind::CrashConsistent;
    let mut label = None;
    let mut read_only = true;

    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--provider" => {
                provider = Some(iter.next().ok_or_else(|| vpt_rs::Error::InvalidArgument {
                    message: "missing value after `--provider`".to_string(),
                })?);
            }
            "--kind" => {
                let value = iter.next().ok_or_else(|| vpt_rs::Error::InvalidArgument {
                    message: "missing value after `--kind`".to_string(),
                })?;
                kind = value.parse()?;
            }
            "--label" => {
                label = Some(iter.next().ok_or_else(|| vpt_rs::Error::InvalidArgument {
                    message: "missing value after `--label`".to_string(),
                })?);
            }
            "--read-write" => {
                read_only = false;
            }
            value if volume.is_none() => {
                volume = Some(VolumeRef::new(value));
            }
            _ => {
                return Err(vpt_rs::Error::InvalidArgument {
                    message: format!("unexpected argument `{arg}`"),
                });
            }
        }
    }

    let source = volume.ok_or_else(|| vpt_rs::Error::InvalidArgument {
        message: "missing source volume for `create`".to_string(),
    })?;

    Ok((
        provider,
        SnapshotRequest {
            source,
            kind,
            label,
            read_only,
        },
    ))
}

fn parse_provider_flag(args: Vec<String>) -> vpt_rs::Result<Option<String>> {
    let mut provider = None;
    let mut iter = args.into_iter();

    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--provider" => {
                provider = Some(iter.next().ok_or_else(|| vpt_rs::Error::InvalidArgument {
                    message: "missing value after `--provider`".to_string(),
                })?);
            }
            _ => {
                return Err(vpt_rs::Error::InvalidArgument {
                    message: format!("unexpected argument `{arg}`"),
                });
            }
        }
    }

    Ok(provider)
}

fn backend_descriptor(backend: &platform::CurrentBackend) -> platform::BackendDescriptor {
    #[cfg(target_os = "linux")]
    {
        return backend.descriptor();
    }

    #[allow(unreachable_code)]
    platform::BackendDescriptor {
        platform: platform::current_platform(),
        provider_name: None,
        backend_name: backend.backend_name(),
        capabilities: backend.capabilities(),
    }
}

fn print_descriptor(descriptor: &platform::BackendDescriptor) {
    println!("platform: {}", descriptor.platform);
    if let Some(provider) = descriptor.provider_name {
        println!("provider: {provider}");
    }
    println!("backend: {}", descriptor.backend_name);
}

fn print_snapshot_usage() {
    println!("vptcli snapshot <command>");
    println!();
    println!("Commands:");
    println!("  backend");
    println!("  backend list");
    println!("  capabilities [--provider <name>]");
    println!("  list [--provider <name>] <volume>");
    println!("  delete [--provider <name>] <snapshot-id>");
    println!(
        "  create [--provider <name>] <volume> [--kind crash|application] [--label <name>] [--read-write]"
    );
}

// ---------------------------------------------------------------------------
// vptcli backup
// ---------------------------------------------------------------------------

struct BackupRequest {
    provider: Option<String>,
    source: BackupSource,
    output: PathBuf,
    snapshot_policy: SnapshotPolicy,
    parent_snapshot: Option<SnapshotRef>,
    block_size: Option<usize>,
}

fn run_backup(args: Vec<String>) -> vpt_rs::Result<()> {
    if args.is_empty()
        || args
            .iter()
            .any(|a| a == "--help" || a == "-h" || a == "help")
    {
        print_backup_usage();
        return Ok(());
    }

    let request = parse_backup_request(args)?;
    let backend = resolve_backend(request.provider.as_deref())?;
    backend.backup_volume(&BackupPlan {
        source: request.source,
        target: BackupTarget::ImageFile(request.output.clone()),
        snapshot_policy: request.snapshot_policy,
        parent_snapshot: request.parent_snapshot,
        block_size: request.block_size,
    })?;

    println!("backend: {}", backend.backend_name());
    println!("output: {}", request.output.display());
    Ok(())
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
    let mut block_size = None;

    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--provider" => {
                provider = Some(iter.next().ok_or_else(|| missing("--provider"))?);
            }
            "--output" => {
                output = Some(PathBuf::from(
                    iter.next().ok_or_else(|| missing("--output"))?,
                ));
            }
            "--snapshot-source" => {
                snapshot_source = true;
            }
            "--parent-snapshot" => {
                parent_snapshot = Some(SnapshotRef::new(
                    iter.next().ok_or_else(|| missing("--parent-snapshot"))?,
                ));
            }
            "--snapshot-label" => {
                snapshot_label = Some(iter.next().ok_or_else(|| missing("--snapshot-label"))?);
            }
            "--snapshot-kind" => {
                let value = iter.next().ok_or_else(|| missing("--snapshot-kind"))?;
                snapshot_kind = value.parse()?;
            }
            "--snapshot-read-write" => {
                snapshot_read_only = false;
            }
            "--block-size" => {
                let value = iter.next().ok_or_else(|| missing("--block-size"))?;
                block_size = Some(parse_block_size(&value)?);
            }
            "--no-snapshot" => {
                snapshot_enabled = false;
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

    let source = source.ok_or_else(|| vpt_rs::Error::InvalidArgument {
        message: "missing source volume".to_string(),
    })?;
    let output = output.ok_or_else(|| vpt_rs::Error::InvalidArgument {
        message: "missing `--output <path>`".to_string(),
    })?;

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
        block_size,
    })
}

fn print_backup_usage() {
    println!(
        "vptcli backup <source> --output <stream-file> [--provider <name>] [--snapshot-source] [--parent-snapshot <id>] [--snapshot-kind crash|application] [--snapshot-label <name>] [--snapshot-read-write] [--no-snapshot] [--block-size <N[K|M|G]>]"
    );
}

// ---------------------------------------------------------------------------
// vptcli restore
// ---------------------------------------------------------------------------

struct RestoreRequest {
    provider: Option<String>,
    input: PathBuf,
    destination: VolumeRef,
    force: bool,
    base_snapshot: Option<SnapshotRef>,
    block_size: Option<usize>,
}

fn run_restore(args: Vec<String>) -> vpt_rs::Result<()> {
    if args.is_empty()
        || args
            .iter()
            .any(|a| a == "--help" || a == "-h" || a == "help")
    {
        print_restore_usage();
        return Ok(());
    }

    let request = parse_restore_request(args)?;
    let backend = resolve_backend(request.provider.as_deref())?;
    backend.restore_volume(&RestorePlan {
        source: BackupTarget::ImageFile(request.input.clone()),
        destination: request.destination,
        force: request.force,
        base_snapshot: request.base_snapshot,
        block_size: request.block_size,
    })?;

    println!("backend: {}", backend.backend_name());
    println!("input: {}", request.input.display());
    Ok(())
}

fn parse_restore_request(args: Vec<String>) -> vpt_rs::Result<RestoreRequest> {
    let mut provider = None;
    let mut input = None;
    let mut destination = None;
    let mut force = false;
    let mut base_snapshot = None;
    let mut block_size = None;

    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--provider" => {
                provider = Some(iter.next().ok_or_else(|| missing("--provider"))?);
            }
            "--input" => {
                input = Some(PathBuf::from(
                    iter.next().ok_or_else(|| missing("--input"))?,
                ));
            }
            "--force" => {
                force = true;
            }
            "--base-snapshot" => {
                base_snapshot = Some(SnapshotRef::new(
                    iter.next().ok_or_else(|| missing("--base-snapshot"))?,
                ));
            }
            "--block-size" => {
                let value = iter.next().ok_or_else(|| missing("--block-size"))?;
                block_size = Some(parse_block_size(&value)?);
            }
            value if destination.is_none() => {
                destination = Some(VolumeRef::new(value));
            }
            _ => {
                return Err(vpt_rs::Error::InvalidArgument {
                    message: format!("unexpected argument `{arg}`"),
                });
            }
        }
    }

    let input = input.ok_or_else(|| vpt_rs::Error::InvalidArgument {
        message: "missing `--input <path>`".to_string(),
    })?;
    let destination = destination.ok_or_else(|| vpt_rs::Error::InvalidArgument {
        message: "missing destination volume/directory".to_string(),
    })?;

    Ok(RestoreRequest {
        provider,
        input,
        destination,
        force,
        base_snapshot,
        block_size,
    })
}

fn print_restore_usage() {
    println!(
        "vptcli restore <destination-dir> --input <stream-file> [--provider <name>] [--force] [--base-snapshot <id>] [--block-size <N[K|M|G]>]"
    );
}
