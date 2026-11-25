//! The bounded `relay` subprocess every reader of relay's CLI shares.
use std::io::{ErrorKind, Read};
use std::os::fd::AsRawFd;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Run {
    Exited { success: bool, stdout: Vec<u8> },
    Timeout,
    Missing,
    Failed,
}

/// Run `relay` with `args`, keeping stdout and discarding stderr. The child is killed when
/// it outlives `timeout` or writes more than `cap` bytes.
pub(crate) fn run(args: &[&str], timeout: Duration, cap: usize) -> Run {
    let deadline = Instant::now() + timeout;
    let mut child = match Command::new("relay")
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(child) => child,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Run::Missing,
        Err(_) => return Run::Failed,
    };

    let result = collect_relay(&mut child, deadline, cap);
    if !matches!(result, Run::Exited { .. }) {
        let _ = child.kill();
        let _ = child.wait();
    }
    result
}

fn collect_relay(child: &mut Child, deadline: Instant, cap: usize) -> Run {
    let Some(mut pipe) = child.stdout.take() else {
        return Run::Failed;
    };
    let fd = pipe.as_raw_fd();
    // Keep stdout on the deadline owner's thread so timeout closes it immediately.
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    if flags == -1 || unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } == -1 {
        return Run::Failed;
    }

    let mut stdout = Vec::new();
    let mut buffer = [0; 4096];
    loop {
        if Instant::now() >= deadline {
            return Run::Timeout;
        }
        let limit = (cap - stdout.len()).saturating_add(1).min(buffer.len());
        match pipe.read(&mut buffer[..limit]) {
            Ok(0) => break,
            Ok(count) => {
                stdout.extend_from_slice(&buffer[..count]);
                if stdout.len() > cap {
                    return Run::Failed;
                }
            }
            Err(error) if error.kind() == ErrorKind::Interrupted => continue,
            Err(error) if error.kind() == ErrorKind::WouldBlock => {
                pause_until_relay_check(deadline)
            }
            Err(_) => return Run::Failed,
        }
    }
    drop(pipe);

    while Instant::now() < deadline {
        match child.try_wait() {
            Ok(Some(status)) => {
                return Run::Exited {
                    success: status.success(),
                    stdout,
                };
            }
            Ok(None) => pause_until_relay_check(deadline),
            Err(_) => return Run::Failed,
        }
    }
    Run::Timeout
}

fn pause_until_relay_check(deadline: Instant) {
    std::thread::sleep(
        deadline
            .saturating_duration_since(Instant::now())
            .min(Duration::from_millis(5)),
    );
}
