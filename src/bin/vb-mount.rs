use std::path::PathBuf;
use std::process::ExitCode;
use tracing::{error, info};

use vpt_rs::logging;
use vpt_rs::platform;
use vpt_rs::{MountHandle, MountManager, MountMode, MountRequest, SnapshotHandle, VolumeRef};

fn main() -> ExitCode {
    logging::init_logging();
    info!(tool = "vb-mount", "cli started");
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            error!(tool = "vb-mount", error = %error, timeout_secs = error.timeout_secs(), "cli failed");
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
        "mount" => {
            let request = parse_mount_request(args.collect())?;
            let backend = resolve_backend(request.provider.as_deref())?;
            let handle = backend.mount_snapshot(&MountRequest {
                snapshot: SnapshotHandle {
                    id: request.snapshot_id,
                    source: VolumeRef::new("unknown"),
                },
                mode: request.mode,
                target: request.target,
            })?;

            println!("backend: {}", backend.backend_name());
            println!("mount-point: {}", handle.mount_point.display());
            Ok(())
        }
        "unmount" => {
            let request = parse_unmount_request(args.collect())?;
            let backend = resolve_backend(request.provider.as_deref())?;
            backend.unmount(&MountHandle {
                id: request.mount_point.display().to_string(),
                mount_point: request.mount_point,
            })?;
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

struct MountCliRequest {
    provider: Option<String>,
    snapshot_id: String,
    mode: MountMode,
    target: Option<PathBuf>,
}

struct UnmountCliRequest {
    provider: Option<String>,
    mount_point: PathBuf,
}

fn parse_mount_request(args: Vec<String>) -> vpt_rs::Result<MountCliRequest> {
    let mut provider = None;
    let mut snapshot_id = None;
    let mut target = None;
    let mut mode = MountMode::ReadOnly;

    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--provider" => {
                let Some(value) = iter.next() else {
                    return missing("--provider");
                };
                provider = Some(value);
            }
            "--target" => {
                let Some(value) = iter.next() else {
                    return missing("--target");
                };
                target = Some(PathBuf::from(value));
            }
            "--read-write" => {
                mode = MountMode::ReadWrite;
            }
            "--help" | "-h" | "help" => {
                print_usage();
                std::process::exit(0);
            }
            value if snapshot_id.is_none() => {
                snapshot_id = Some(value.to_string());
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
            message: "missing snapshot id for `mount`".to_string(),
        });
    };

    Ok(MountCliRequest {
        provider,
        snapshot_id,
        mode,
        target,
    })
}

fn parse_unmount_request(args: Vec<String>) -> vpt_rs::Result<UnmountCliRequest> {
    let mut provider = None;
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

    let Some(mount_point) = mount_point else {
        return Err(vpt_rs::Error::InvalidArgument {
            message: "missing mount point for `unmount`".to_string(),
        });
    };

    Ok(UnmountCliRequest {
        provider,
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
    println!("vb-mount mount <snapshot-id> [--provider <name>] [--target <dir>] [--read-write]");
    println!("vb-mount unmount <mount-point> [--provider <name>]");
}
