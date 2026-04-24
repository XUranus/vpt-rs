use std::process::ExitCode;

use vpt_rs::platform;
use vpt_rs::{SnapshotKind, SnapshotProvider, SnapshotRequest, VolumeRef};

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
    let mut args = std::env::args().skip(1);
    let Some(command) = args.next() else {
        print_usage();
        return Ok(());
    };

    match command.as_str() {
        "backend" => match args.next().as_deref() {
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
            let provider = parse_provider_flag(args.collect())?;
            let backend = resolve_backend(provider.as_deref())?;
            let descriptor = backend_descriptor(&backend);
            println!("{}", descriptor.backend_name);
            for capability in descriptor.capabilities {
                println!("- {}", capability);
            }
            Ok(())
        }
        "create" => {
            let (provider, request) = parse_create_request(args.collect())?;
            let backend = resolve_backend(provider.as_deref())?;
            let snapshot = backend.create_snapshot(&request)?;
            println!("snapshot: {}", snapshot.handle.id);
            println!("source: {}", snapshot.handle.source);
            println!("backend: {}", snapshot.backend);
            if let Some(path_hint) = snapshot.path_hint {
                println!("path: {}", path_hint.display());
            }
            Ok(())
        }
        "list" => {
            let mut provider = None;
            let mut volume = None;

            while let Some(arg) = args.next() {
                match arg.as_str() {
                    "--provider" => {
                        let Some(value) = args.next() else {
                            return Err(vpt_rs::Error::InvalidArgument {
                                message: "missing value after `--provider`".to_string(),
                            });
                        };
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

            let Some(volume) = volume else {
                return Err(vpt_rs::Error::InvalidArgument {
                    message: "missing volume id for `list`".to_string(),
                });
            };

            let backend = resolve_backend(provider.as_deref())?;
            let snapshots = backend.list_snapshots(&VolumeRef::new(volume))?;
            for snapshot in snapshots {
                println!(
                    "{} {} {}",
                    snapshot.handle.id, snapshot.handle.source, snapshot.backend
                );
            }
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
                let Some(value) = iter.next() else {
                    return Err(vpt_rs::Error::InvalidArgument {
                        message: "missing value after `--provider`".to_string(),
                    });
                };
                provider = Some(value);
            }
            "--kind" => {
                let Some(value) = iter.next() else {
                    return Err(vpt_rs::Error::InvalidArgument {
                        message: "missing value after `--kind`".to_string(),
                    });
                };
                kind = value.parse()?;
            }
            "--label" => {
                let Some(value) = iter.next() else {
                    return Err(vpt_rs::Error::InvalidArgument {
                        message: "missing value after `--label`".to_string(),
                    });
                };
                label = Some(value);
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

    let Some(source) = volume else {
        return Err(vpt_rs::Error::InvalidArgument {
            message: "missing source volume for `create`".to_string(),
        });
    };

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
                let Some(value) = iter.next() else {
                    return Err(vpt_rs::Error::InvalidArgument {
                        message: "missing value after `--provider`".to_string(),
                    });
                };
                provider = Some(value);
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
        capabilities: vpt_rs::SnapshotProvider::capabilities(backend),
    }
}

fn print_descriptor(descriptor: &platform::BackendDescriptor) {
    println!("platform: {}", descriptor.platform);
    if let Some(provider) = descriptor.provider_name {
        println!("provider: {provider}");
    }
    println!("backend: {}", descriptor.backend_name);
}

fn print_usage() {
    println!("vb-snapshot <command>");
    println!();
    println!("Commands:");
    println!("  backend");
    println!("  backend list");
    println!("  capabilities [--provider <name>]");
    println!("  list [--provider <name>] <volume>");
    println!(
        "  create [--provider <name>] <volume> [--kind crash|application] [--label <name>] [--read-write]"
    );
}
