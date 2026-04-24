use std::path::PathBuf;
use std::process::ExitCode;
use tracing::{error, info};

use vpt_rs::logging;
use vpt_rs::platform;
use vpt_rs::restore::RestorePlanner;
use vpt_rs::{BackupTarget, RestorePlan, SnapshotRef, VolumeRef};

fn main() -> ExitCode {
    logging::init_logging();
    info!(tool = "vb-restore", "cli started");
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            error!(tool = "vb-restore", error = %error, timeout_secs = error.timeout_secs(), "cli failed");
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

    let request = parse_restore_request(args)?;
    let backend = resolve_backend(request.provider.as_deref())?;
    backend.restore_volume(&RestorePlan {
        source: BackupTarget::ImageFile(request.input.clone()),
        destination: request.destination,
        force: request.force,
        base_snapshot: request.base_snapshot,
    })?;

    println!("backend: {}", backend.backend_name());
    println!("input: {}", request.input.display());
    Ok(())
}

struct RestoreRequest {
    provider: Option<String>,
    input: PathBuf,
    destination: VolumeRef,
    force: bool,
    base_snapshot: Option<SnapshotRef>,
}

fn parse_restore_request(args: Vec<String>) -> vpt_rs::Result<RestoreRequest> {
    let mut provider = None;
    let mut input = None;
    let mut destination = None;
    let mut force = false;
    let mut base_snapshot = None;

    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--provider" => {
                let Some(value) = iter.next() else {
                    return missing("--provider");
                };
                provider = Some(value);
            }
            "--input" => {
                let Some(value) = iter.next() else {
                    return missing("--input");
                };
                input = Some(PathBuf::from(value));
            }
            "--force" => {
                force = true;
            }
            "--base-snapshot" => {
                let Some(value) = iter.next() else {
                    return missing("--base-snapshot");
                };
                base_snapshot = Some(SnapshotRef::new(value));
            }
            "--help" | "-h" | "help" => {
                print_usage();
                std::process::exit(0);
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

    let Some(input) = input else {
        return Err(vpt_rs::Error::InvalidArgument {
            message: "missing `--input <path>`".to_string(),
        });
    };
    let Some(destination) = destination else {
        return Err(vpt_rs::Error::InvalidArgument {
            message: "missing destination volume/directory".to_string(),
        });
    };

    Ok(RestoreRequest {
        provider,
        input,
        destination,
        force,
        base_snapshot,
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

fn missing(flag: &str) -> vpt_rs::Result<RestoreRequest> {
    Err(vpt_rs::Error::InvalidArgument {
        message: format!("missing value after `{flag}`"),
    })
}

fn print_usage() {
    println!(
        "vb-restore <destination-dir> --input <stream-file> [--provider <name>] [--force] [--base-snapshot <id>]"
    );
}
