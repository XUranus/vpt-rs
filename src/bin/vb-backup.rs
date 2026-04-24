use std::path::PathBuf;
use std::process::ExitCode;

use vpt_rs::backup::BlockDeviceCopier;
use vpt_rs::platform;
use vpt_rs::{BackupPlan, BackupTarget, VolumeRef};

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
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
        use_snapshot: request.use_snapshot,
    })?;

    println!("backend: {}", backend.backend_name());
    println!("output: {}", request.output.display());
    Ok(())
}

struct BackupRequest {
    provider: Option<String>,
    source: VolumeRef,
    output: PathBuf,
    use_snapshot: bool,
}

fn parse_backup_request(args: Vec<String>) -> vpt_rs::Result<BackupRequest> {
    let mut provider = None;
    let mut source = None;
    let mut output = None;
    let mut use_snapshot = true;

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
            "--no-snapshot" => {
                use_snapshot = false;
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
        source,
        output,
        use_snapshot,
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
        "vb-backup <source-volume> --output <stream-file> [--provider <name>] [--no-snapshot]"
    );
}
