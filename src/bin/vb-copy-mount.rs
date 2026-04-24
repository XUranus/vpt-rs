use std::path::PathBuf;
use std::process::ExitCode;
use tracing::{error, info};

use vpt_rs::copy_mount;
use vpt_rs::logging;
use vpt_rs::platform;
use vpt_rs::{
    MountHandle, MountMode, SnapshotHandle, SnapshotKind, VolumeRef,
};

fn main() -> ExitCode {
    logging::init_logging();
    info!(tool = "vb-copy-mount", "cli started");
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            error!(tool = "vb-copy-mount", error = %error, timeout_secs = error.timeout_secs(), "cli failed");
            eprintln!("error: {error}");
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

    match command.as_str() {
        "open" => {
            let request = parse_open_request(args.collect())?;
            let backend = resolve_backend(request.provider.as_deref())?;
            let session = copy_mount::open_copy_mount(
                &backend,
                &copy_mount::CopyMountRequest {
                    source: request.source,
                    kind: request.kind,
                    label: request.label,
                    mode: request.mode,
                    target: request.target,
                },
            )?;

            println!("backend: {}", backend.backend_name());
            println!("snapshot: {}", session.snapshot.handle.id);
            println!("mount-point: {}", session.mount.mount_point.display());
            Ok(())
        }
        "close" => {
            let request = parse_close_request(args.collect())?;
            let backend = resolve_backend(request.provider.as_deref())?;
            copy_mount::close_copy_mount(
                &backend,
                &SnapshotHandle {
                    id: request.snapshot_id,
                    source: VolumeRef::new("unknown"),
                },
                &MountHandle {
                    id: request.mount_point.display().to_string(),
                    mount_point: request.mount_point,
                },
            )?;
            Ok(())
        }
        "help" | "--help" | "-h" => {
            print_usage();
            Ok(())
        }
        _ => Err(vpt_rs::Error::InvalidArgument {
            message: format!("unknown command `{command}`"),
        }),
    }
}

struct OpenRequest {
    provider: Option<String>,
    source: VolumeRef,
    kind: SnapshotKind,
    label: Option<String>,
    mode: MountMode,
    target: Option<PathBuf>,
}

struct CloseRequest {
    provider: Option<String>,
    snapshot_id: String,
    mount_point: PathBuf,
}

fn parse_open_request(args: Vec<String>) -> vpt_rs::Result<OpenRequest> {
    let mut provider = None;
    let mut source = None;
    let mut kind = SnapshotKind::CrashConsistent;
    let mut label = None;
    let mut mode = MountMode::ReadOnly;
    let mut target = None;

    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--provider" => {
                let Some(value) = iter.next() else {
                    return missing("--provider");
                };
                provider = Some(value);
            }
            "--kind" => {
                let Some(value) = iter.next() else {
                    return missing("--kind");
                };
                kind = value.parse()?;
            }
            "--label" => {
                let Some(value) = iter.next() else {
                    return missing("--label");
                };
                label = Some(value);
            }
            "--read-write" => {
                mode = MountMode::ReadWrite;
            }
            "--target" => {
                let Some(value) = iter.next() else {
                    return missing("--target");
                };
                target = Some(PathBuf::from(value));
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
            message: "missing source volume for `open`".to_string(),
        });
    };

    Ok(OpenRequest {
        provider,
        source,
        kind,
        label,
        mode,
        target,
    })
}

fn parse_close_request(args: Vec<String>) -> vpt_rs::Result<CloseRequest> {
    let mut provider = None;
    let mut snapshot_id = None;
    let mut mount_point = None;

    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--provider" => {
                let Some(value) = iter.next() else {
                    return missing("--provider");
                };
                provider = Some(value);
            }
            "--help" | "-h" | "help" => {
                print_usage();
                std::process::exit(0);
            }
            value if snapshot_id.is_none() => {
                snapshot_id = Some(value.to_string());
            }
            value if mount_point.is_none() => {
                mount_point = Some(PathBuf::from(value));
            }
            _ => {
                return Err(vpt_rs::Error::InvalidArgument {
                    message: format!("unexpected argument `{arg}`"),
                });
            }
        }
    }

    let Some(snapshot_id) = snapshot_id else {
        return Err(vpt_rs::Error::InvalidArgument {
            message: "missing snapshot id for `close`".to_string(),
        });
    };
    let Some(mount_point) = mount_point else {
        return Err(vpt_rs::Error::InvalidArgument {
            message: "missing mount point for `close`".to_string(),
        });
    };

    Ok(CloseRequest {
        provider,
        snapshot_id,
        mount_point,
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

fn missing<T>(flag: &str) -> vpt_rs::Result<T> {
    Err(vpt_rs::Error::InvalidArgument {
        message: format!("missing value after `{flag}`"),
    })
}

fn print_usage() {
    println!(
        "vb-copy-mount open <source> [--provider <name>] [--kind crash|application] [--label <name>] [--target <dir>] [--read-write]"
    );
    println!("vb-copy-mount close <snapshot-id> <mount-point> [--provider <name>]");
}
