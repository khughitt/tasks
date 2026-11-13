//! Debug-build-only observations and explicit test barriers; never bypass validation.
use crate::error::Result;

#[cfg(debug_assertions)]
fn connect(
    path: &std::path::Path,
    deadline: std::time::Instant,
    barrier: bool,
) -> std::io::Result<std::os::unix::net::UnixStream> {
    use std::{
        io,
        os::{fd::FromRawFd, unix::ffi::OsStrExt},
        time::{Duration, Instant},
    };
    let bytes = path.as_os_str().as_bytes();
    // sockaddr_un is zeroed to terminate the checked pathname; this socket owns its fd.
    let mut address: libc::sockaddr_un = unsafe { std::mem::zeroed() };
    if bytes.is_empty() || bytes.contains(&0) || bytes.len() >= address.sun_path.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "invalid test socket path",
        ));
    }
    address.sun_family = libc::AF_UNIX as libc::sa_family_t;
    for (slot, byte) in address.sun_path.iter_mut().zip(bytes) {
        *slot = *byte as libc::c_char;
    }
    loop {
        let fd = unsafe {
            libc::socket(
                libc::AF_UNIX,
                libc::SOCK_STREAM | libc::SOCK_NONBLOCK | libc::SOCK_CLOEXEC,
                0,
            )
        };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        let stream = unsafe { std::os::unix::net::UnixStream::from_raw_fd(fd) };
        let result = unsafe {
            libc::connect(
                fd,
                (&address as *const libc::sockaddr_un).cast(),
                std::mem::size_of_val(&address) as libc::socklen_t,
            )
        };
        if result == 0 {
            return Ok(stream);
        }
        let error = io::Error::last_os_error();
        drop(stream);
        if error.kind() != io::ErrorKind::WouldBlock {
            return Err(error);
        }
        // Observation frames fail immediately on backpressure; only explicit barriers may wait.
        if !barrier {
            return Err(error);
        }
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "test barrier connection timed out",
            ));
        }
        std::thread::sleep(left.min(Duration::from_millis(1)));
    }
}

pub(crate) fn event(name: &str, phase: &str) -> Result<()> {
    #[cfg(debug_assertions)]
    if let Some(path) = std::env::var_os("TASKS_RETAINED_TEST_SOCKET") {
        use std::{
            io::{Read, Write},
            sync::OnceLock,
            time::{Duration, Instant},
        };
        static START: OnceLock<Instant> = OnceLock::new();
        let elapsed = START.get_or_init(Instant::now).elapsed().as_nanos();
        let barrier = matches!(
            name,
            "inspection" | "prechecked" | "before_write" | "readback"
        );
        let deadline = Instant::now() + Duration::from_secs(60);
        let mut bytes = serde_json::to_vec(
            &serde_json::json!({"event":name,"phase":phase,"elapsed_ns":elapsed,"barrier":barrier}),
        )
        .map_err(super::model::invalid)?;
        bytes.push(b'\n');
        if bytes.len() > 4096 {
            return Err(super::model::invalid("test observation exceeds bound"));
        }
        let mut stream = connect(std::path::Path::new(&path), deadline, barrier)?;
        if barrier {
            stream.set_nonblocking(false)?;
            stream.set_write_timeout(Some(
                deadline
                    .saturating_duration_since(Instant::now())
                    .max(Duration::from_nanos(1)),
            ))?;
        }
        // Nonblocking observation writes never wait for parent scheduling or an acknowledgement.
        stream.write_all(&bytes)?;
        if barrier {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return Err(super::model::invalid("test barrier timed out"));
            }
            stream.set_read_timeout(Some(left))?;
            let mut ack = [0];
            stream.read_exact(&mut ack)?;
            if ack != *b"+" {
                return Err(super::model::invalid("test observer refused continuation"));
            }
        }
    }
    let _ = (name, phase);
    Ok(())
}

#[cfg(all(test, debug_assertions))]
mod tests {
    #[test]
    fn retained_observer_backpressure_is_bounded() {
        use std::{
            io::ErrorKind,
            os::{fd::AsRawFd, unix::net::UnixListener},
            time::{Duration, Instant},
        };
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("observer.sock");
        let listener = UnixListener::bind(&path).unwrap();
        assert_eq!(unsafe { libc::listen(listener.as_raw_fd(), 1) }, 0);
        let mut connections = Vec::new();
        loop {
            match super::connect(&path, Instant::now(), false) {
                Ok(stream) => {
                    connections.push(stream);
                    assert!(connections.len() < 1024);
                }
                Err(error) => {
                    assert_eq!(error.kind(), ErrorKind::WouldBlock);
                    break;
                }
            }
        }
        // A full accept queue cannot make observation connect block; a barrier has one deadline.
        assert_eq!(
            super::connect(&path, Instant::now() + Duration::from_millis(5), true)
                .unwrap_err()
                .kind(),
            ErrorKind::TimedOut
        );
    }
}

/// Command classification only; no environment, input bytes or general process tracing.
pub(crate) fn git_start(source: &str, repository: &std::path::Path, args: &[&str]) -> Result<()> {
    query_event("git_start", source, repository, args)
}

pub(crate) fn query_admitted(repository: &std::path::Path, args: &[&str]) -> Result<()> {
    query_event("query_admitted", "immutable_read", repository, args)
}

fn query_event(
    name: &str,
    source: &str,
    repository: &std::path::Path,
    args: &[&str],
) -> Result<()> {
    #[cfg(debug_assertions)]
    if std::env::var_os("TASKS_RETAINED_TEST_SOCKET").is_some() {
        // Mutable starts need only a verb; commit messages and other inputs stay out.
        let args = if source == "immutable_read" {
            args
        } else {
            &args[..args.len().min(1)]
        };
        event(
            name,
            &serde_json::to_string(
                &serde_json::json!({"source":source,"repository":repository,"argv":args}),
            )
            .map_err(super::model::invalid)?,
        )?;
    }
    let _ = (name, source, repository, args);
    Ok(())
}
