//! Advisory wait reporting never owns a lock descriptor or publication warnings.
use std::{
    fs::File,
    io::{self, Read, Write},
    sync::{Arc, Condvar, Mutex},
    thread::JoinHandle,
    time::{Duration, Instant},
};

#[derive(Clone, Copy)]
struct Identity {
    major: u64,
    minor: u64,
    inode: u64,
}
impl Identity {
    #[cfg(target_os = "linux")]
    fn of(file: &File) -> Option<Self> {
        use std::os::unix::fs::MetadataExt;
        let m = file.metadata().ok()?;
        Some(Self {
            major: libc::major(m.dev()) as u64,
            minor: libc::minor(m.dev()) as u64,
            inode: m.ino(),
        })
    }
    #[cfg(not(target_os = "linux"))]
    fn of(_: &File) -> Option<Self> {
        None
    }
}

struct Cadence {
    next: Duration,
    interval: Duration,
}
impl Default for Cadence {
    fn default() -> Self {
        Self::new(Duration::from_secs(2), Duration::from_secs(30))
    }
}
impl Cadence {
    fn new(first: Duration, interval: Duration) -> Self {
        Self {
            next: first,
            interval,
        }
    }
    fn due(&mut self, elapsed: Duration) -> bool {
        if elapsed < self.next {
            return false;
        }
        self.next = elapsed + self.interval;
        true
    }
}
fn bounded(value: &str) -> String {
    let mut out = String::new();
    for c in value.chars() {
        let escaped = if c.is_control() {
            c.escape_default().to_string()
        } else {
            c.to_string()
        };
        if out.chars().count() + escaped.chars().count() > 512 {
            break;
        }
        out.push_str(&escaped);
    }
    out
}
fn parse_holder(bytes: &[u8], identity: Identity) -> Result<u32, &'static str> {
    if bytes.len() > 1024 * 1024 {
        return Err("proc read bound");
    }
    let text = std::str::from_utf8(bytes).map_err(|_| "malformed proc data")?;
    let mut holder = None;
    for line in text.lines() {
        if line.len() > 4096 {
            return Err("proc line bound");
        }
        let fields: Vec<_> = line.split_whitespace().collect();
        if fields.get(1) == Some(&"->") {
            continue;
        }
        if fields.len() != 8 || !fields[0].ends_with(':') {
            return Err("malformed proc data");
        }
        if fields[1] != "FLOCK" || fields[3] != "WRITE" {
            continue;
        }
        if fields[2] != "ADVISORY"
            || fields[6] != "0"
            || fields[7] != "EOF"
            || fields[0].trim_end_matches(':').parse::<u64>().is_err()
        {
            return Err("malformed FLOCK entry");
        }
        let pid: u32 = fields[4].parse().map_err(|_| "unavailable PID")?;
        if pid == 0 || pid > i32::MAX as u32 {
            return Err("unavailable PID");
        }
        let parts: Vec<_> = fields[5].split(':').collect();
        if parts.len() != 3 {
            return Err("malformed proc device");
        }
        let major = u64::from_str_radix(parts[0], 16).map_err(|_| "malformed proc device")?;
        let minor = u64::from_str_radix(parts[1], 16).map_err(|_| "malformed proc device")?;
        let inode = parts[2]
            .parse::<u64>()
            .map_err(|_| "malformed proc inode")?;
        if (major, minor, inode) == (identity.major, identity.minor, identity.inode)
            && holder.replace(pid).is_some()
        {
            return Err("ambiguous proc match");
        }
    }
    holder.ok_or("no matching granted FLOCK")
}
fn holder(identity: Option<Identity>) -> Result<u32, &'static str> {
    #[cfg(target_os = "linux")]
    {
        let identity = identity.ok_or("file identity unavailable")?;
        let file = File::open("/proc/locks").map_err(|_| "proc unavailable")?;
        let mut bytes = Vec::new();
        file.take(1024 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| "proc read failed")?;
        parse_holder(&bytes, identity)
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = identity;
        Err("unsupported platform")
    }
}
type Cancellation = Arc<(Mutex<bool>, Condvar)>;
fn report(
    cancel: Cancellation,
    start: Instant,
    first: Duration,
    cadence: Duration,
    mut emit: impl FnMut(Duration) -> io::Result<()>,
) -> bool {
    let mut cadence = Cadence::new(first, cadence);
    let mut shown = false;
    let mut stopped = cancel.0.lock().unwrap_or_else(|e| e.into_inner());
    loop {
        if *stopped {
            return shown;
        }
        let remaining = cadence.next.saturating_sub(start.elapsed());
        if !remaining.is_zero() {
            stopped = cancel
                .1
                .wait_timeout(stopped, remaining)
                .unwrap_or_else(|e| e.into_inner())
                .0;
            continue;
        }
        if !cadence.due(start.elapsed()) {
            continue;
        }
        drop(stopped);
        shown |= emit(start.elapsed()).is_ok();
        stopped = cancel.0.lock().unwrap_or_else(|e| e.into_inner());
    }
}
struct Reporter {
    cancel: Cancellation,
    thread: JoinHandle<bool>,
    start: Instant,
    label: String,
}
impl Reporter {
    fn start(
        scope: String,
        records: String,
        path: String,
        identity: Option<Identity>,
    ) -> Option<Self> {
        let label = format!(
            "scope={} records={} lock={}",
            bounded(&scope),
            bounded(&records),
            bounded(&path)
        );
        let cancel = Arc::new((Mutex::new(false), Condvar::new()));
        let worker_cancel = cancel.clone();
        let worker_label = label.clone();
        let start = Instant::now();
        match std::thread::Builder::new()
            .name("tasks-lock-wait".into())
            .spawn(move || {
                report(
                    worker_cancel,
                    start,
                    Duration::from_secs(2),
                    Duration::from_secs(30),
                    |elapsed| {
                        let observation = match holder(identity) {
                            Ok(pid) => format!("holder PID {pid} (kernel snapshot)"),
                            Err(reason) => format!("holder unavailable ({reason})"),
                        };
                        stderr(&format!(
                            "tasks: waiting {worker_label} elapsed={}s {observation}\n",
                            elapsed.as_secs()
                        ))
                    },
                )
            }) {
            Ok(thread) => Some(Self {
                cancel,
                thread,
                start,
                label,
            }),
            Err(_) => {
                let _ = stderr("tasks: lock wait reporter unavailable; continuing kernel wait\n");
                None
            }
        }
    }
    fn finish(self, acquired: bool) -> bool {
        *self.cancel.0.lock().unwrap_or_else(|e| e.into_inner()) = true;
        self.cancel.1.notify_all();
        let shown = self.thread.join().unwrap_or(false);
        if shown && acquired {
            let _ = stderr(&format!(
                "tasks: acquired {} elapsed={}s\n",
                self.label,
                self.start.elapsed().as_secs()
            ));
        }
        shown
    }
}
fn stderr(text: &str) -> io::Result<()> {
    use std::os::unix::fs::OpenOptionsExt;
    let mut metadata = std::mem::MaybeUninit::<libc::stat>::uninit();
    if unsafe { libc::fstat(libc::STDERR_FILENO, metadata.as_mut_ptr()) } != 0 {
        return Err(io::Error::last_os_error());
    }
    if unsafe { metadata.assume_init() }.st_mode & libc::S_IFMT == libc::S_IFREG {
        // Preserve the shared file position of a redirected log.
        return io::stderr().write_all(text.as_bytes());
    }
    // Reopen pipes/terminals rather than changing flags on the caller's shared stderr.
    let mut output = std::fs::OpenOptions::new()
        .write(true)
        .custom_flags(libc::O_NONBLOCK | libc::O_CLOEXEC | libc::O_NOCTTY)
        .open("/dev/stderr")?;
    output.write_all(text.as_bytes())
}
pub(super) fn lock(
    file: &File,
    path: &std::path::Path,
    scope: &str,
    records: Option<&std::path::Path>,
) -> io::Result<()> {
    loop {
        match file.try_lock() {
            Ok(()) => return Ok(()),
            Err(std::fs::TryLockError::WouldBlock) => break,
            Err(std::fs::TryLockError::Error(e)) if e.kind() == io::ErrorKind::Interrupted => {
                continue;
            }
            Err(std::fs::TryLockError::Error(e)) => return Err(e),
        }
    }
    if scope.starts_with("prefix ") {
        crate::retained::test_observer::event("prefix_wait", scope)
            .map_err(|e| io::Error::other(e.to_string()))?;
    }
    let reporter = Reporter::start(
        scope.into(),
        records.map_or_else(
            || "unavailable".into(),
            |p| p.to_string_lossy().into_owned(),
        ),
        path.to_string_lossy().into_owned(),
        Identity::of(file),
    );
    blocking(file, reporter)
}
fn blocking(file: &File, reporter: Option<Reporter>) -> io::Result<()> {
    let result = loop {
        match file.lock() {
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            result => break result,
        }
    };
    if let Some(reporter) = reporter {
        reporter.finish(result.is_ok());
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn lock_wait_cadence_and_cancel() {
        let mut cadence = Cadence::default();
        for seconds in [0, 1] {
            assert!(!cadence.due(Duration::from_secs(seconds)));
        }
        for seconds in [2, 32, 62] {
            assert!(cadence.due(Duration::from_secs(seconds)));
            assert!(!cadence.due(Duration::from_secs(seconds + 29)));
        }
        let reporter = Reporter::start("prefix sci".into(), "records".into(), "lock".into(), None);
        let before = Instant::now();
        assert!(!reporter.unwrap().finish(false));
        assert!(
            before.elapsed() < Duration::from_secs(1),
            "cancellation must wake and join"
        );
        assert!(!Cadence::default().due(Duration::from_millis(1999)));
        let label = bounded("hello\n\r\u{1b}\t");
        assert!(!label.chars().any(char::is_control));
        assert!(bounded(&"\n".repeat(1000)).chars().count() <= 512);
    }
    #[test]
    fn lock_wait_proc_holder() {
        let identity = Identity {
            major: 0,
            minor: 42,
            inode: 100,
        };
        let entry = "1: FLOCK ADVISORY WRITE 123 00:2a:100 0 EOF\n";
        assert_eq!(parse_holder(entry.as_bytes(), identity), Ok(123));
        assert_eq!(
            parse_holder(entry.replace("123", "456").as_bytes(), identity),
            Ok(456)
        );
        for bad in [
            "",
            "1: -> FLOCK ADVISORY WRITE 123 00:2a:100 0 EOF\n",
            "1: FLOCK ADVISORY WRITE -1 00:2a:100 0 EOF\n",
            "1: FLOCK ADVISORY WRITE xyz 00:2a:100 0 EOF\n",
            "1: FLOCK ADVISORY WRITE 123 01:2a:100 0 EOF\n",
            "1: FLOCK ADVISORY WRITE 123 00:2b:100 0 EOF\n",
            "malformed\n",
        ] {
            assert!(parse_holder(bad.as_bytes(), identity).is_err(), "{bad}");
        }
        assert!(parse_holder(format!("{entry}{entry}").as_bytes(), identity).is_err());
        assert!(parse_holder(&vec![b' '; 1024 * 1024 + 1], identity).is_err());
        assert!(parse_holder(&vec![b' '; 4097], identity).is_err());
    }
    #[test]
    fn lock_wait_reporter_and_proc_unavailable() {
        assert!(holder(None).is_err());
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("empty.lock");
        let held = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(&path)
            .unwrap();
        held.lock().unwrap();
        let (tx, rx) = std::sync::mpsc::channel();
        let worker = std::thread::spawn(move || {
            let file = std::fs::OpenOptions::new().write(true).open(path).unwrap();
            assert!(matches!(
                file.try_lock(),
                Err(std::fs::TryLockError::WouldBlock)
            ));
            tx.send(()).unwrap();
            // The same path used after reporter creation fails: kernel acquisition continues.
            blocking(&file, None).unwrap();
            assert!(file.try_lock().is_ok());
        });
        rx.recv_timeout(Duration::from_secs(1)).unwrap();
        drop(held);
        worker.join().unwrap();
    }
    #[test]
    fn lock_wait_failure_keeps_publication() {
        let cancel = Arc::new((Mutex::new(false), Condvar::new()));
        let worker_cancel = cancel.clone();
        let (tx, rx) = std::sync::mpsc::channel();
        let thread = std::thread::spawn(move || {
            report(
                worker_cancel,
                Instant::now(),
                Duration::ZERO,
                Duration::from_secs(30),
                |_| {
                    tx.send(()).unwrap();
                    Err(io::Error::other("broken stderr"))
                },
            )
        });
        rx.recv_timeout(Duration::from_secs(1)).unwrap();
        *cancel.0.lock().unwrap() = true;
        cancel.1.notify_all();
        assert!(
            !thread.join().unwrap(),
            "failed output never records a displayed notice"
        );
    }
}
