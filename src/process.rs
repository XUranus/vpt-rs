use std::fs::File;
use std::path::PathBuf;
use std::process::{Command, ExitStatus, Output, Stdio};
use std::thread;
use std::time::Duration;
use std::time::Instant;

use tracing::{debug, error, info, warn};

use crate::error::{Error, Result};

const DEFAULT_TIMEOUT_SECS: u64 = 30;

#[derive(Debug, Clone, Default)]
pub struct CommandIo {
    pub stdin_file: Option<PathBuf>,
    pub stdout_file: Option<PathBuf>,
}

pub fn run_command(
    backend: &'static str,
    operation: &'static str,
    program: &'static str,
    args: &[String],
    io: CommandIo,
) -> Result<Output> {
    let command_line = display_command(program, args);
    let timeout = command_timeout();
    info!(
        backend,
        operation,
        command = %command_line,
        timeout_secs = timeout.as_secs(),
        "starting external command"
    );

    let mut command = Command::new(program);
    command.args(args);
    command.stderr(Stdio::piped());

    if let Some(stdin_path) = &io.stdin_file {
        let file = File::open(stdin_path)?;
        command.stdin(Stdio::from(file));
        debug!(backend, operation, path = %stdin_path.display(), "attached stdin file");
    }

    if let Some(stdout_path) = &io.stdout_file {
        if let Some(parent) = stdout_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let file = File::create(stdout_path)?;
        command.stdout(Stdio::from(file));
        debug!(backend, operation, path = %stdout_path.display(), "attached stdout file");
    } else {
        command.stdout(Stdio::piped());
    }

    let mut child = command.spawn()?;
    match wait_with_timeout(&mut child, timeout)? {
        Some(status) => collect_output(
            backend,
            operation,
            command_line,
            child,
            status,
            io.stdout_file.is_some(),
        ),
        None => {
            warn!(backend, operation, command = %command_line, timeout_secs = timeout.as_secs(), "command timed out; killing child");
            let _ = child.kill();
            let _ = child.wait();
            Err(Error::Timeout {
                operation,
                backend,
                timeout_secs: timeout.as_secs(),
            })
        }
    }
}

fn collect_output(
    backend: &'static str,
    operation: &'static str,
    command_line: String,
    mut child: std::process::Child,
    status: ExitStatus,
    stdout_redirected: bool,
) -> Result<Output> {
    let stdout = if stdout_redirected {
        Vec::new()
    } else {
        read_child_pipe(child.stdout.take())?
    };
    let stderr = read_child_pipe(child.stderr.take())?;

    if status.success() {
        info!(backend, operation, command = %command_line, status = ?status.code(), "external command completed");
        Ok(Output {
            status,
            stdout,
            stderr,
        })
    } else {
        let stderr_text = String::from_utf8_lossy(&stderr).trim().to_string();
        error!(backend, operation, command = %command_line, status = ?status.code(), stderr = %stderr_text, "external command failed");
        Err(Error::CommandFailed {
            command: command_line,
            status: status.code().unwrap_or(-1),
            stderr: stderr_text,
        })
    }
}

fn read_child_pipe(pipe: Option<impl std::io::Read>) -> Result<Vec<u8>> {
    let mut buffer = Vec::new();
    if let Some(mut pipe) = pipe {
        std::io::Read::read_to_end(&mut pipe, &mut buffer)?;
    }
    Ok(buffer)
}

fn wait_with_timeout(
    child: &mut std::process::Child,
    timeout: Duration,
) -> Result<Option<ExitStatus>> {
    let started = Instant::now();
    // Start with a short poll for fast-exiting commands, then back off to
    // reduce CPU usage for long-running operations.
    let mut poll_interval = Duration::from_millis(10);
    let max_interval = Duration::from_millis(200);
    loop {
        if let Some(status) = child.try_wait()? {
            return Ok(Some(status));
        }

        if started.elapsed() >= timeout {
            return Ok(None);
        }

        thread::sleep(poll_interval);
        poll_interval = (poll_interval * 2).min(max_interval);
    }
}

pub fn command_timeout() -> Duration {
    std::env::var("VPT_COMMAND_TIMEOUT_SECS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value > 0)
        .map(Duration::from_secs)
        .unwrap_or_else(|| Duration::from_secs(DEFAULT_TIMEOUT_SECS))
}

fn display_command(program: &str, args: &[String]) -> String {
    if args.is_empty() {
        program.to_string()
    } else {
        format!("{program} {}", args.join(" "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(target_family = "unix")]
    #[test]
    fn wait_with_timeout_returns_none_for_long_running_child() {
        let mut child = Command::new("sh")
            .args(["-c", "sleep 1"])
            .spawn()
            .expect("spawn sleep");

        let status = wait_with_timeout(&mut child, Duration::from_millis(50)).unwrap();
        assert!(status.is_none());

        let _ = child.kill();
        let _ = child.wait();
    }
}
