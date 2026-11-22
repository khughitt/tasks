//! Advisory wait reporting never owns a lock descriptor or publication warnings.
use std::{
    fs::File,
    io::{self, Read, Write},
    path::Path,
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
/// The granted holder of one lock and how many processes are blocked behind it.
#[derive(Debug, PartialEq)]
struct Snapshot {
    pid: u32,
    waiters: usize,
}
fn device(field: &str) -> Result<(u64, u64, u64), &'static str> {
    let parts: Vec<_> = field.split(':').collect();
    if parts.len() != 3 {
        return Err("malformed proc device");
    }
    let major = u64::from_str_radix(parts[0], 16).map_err(|_| "malformed proc device")?;
    let minor = u64::from_str_radix(parts[1], 16).map_err(|_| "malformed proc device")?;
    let inode = parts[2]
        .parse::<u64>()
        .map_err(|_| "malformed proc inode")?;
    Ok((major, minor, inode))
}
fn pid(field: &str) -> Result<u32, &'static str> {
    let pid: u32 = field.parse().map_err(|_| "unavailable PID")?;
    if pid == 0 || pid > i32::MAX as u32 {
        return Err("unavailable PID");
    }
    Ok(pid)
}
/// Reads `/proc/locks`: the one granted FLOCK on the file, plus every `->` waiter line
/// (blocked requests, this process included) on the same device and inode.
fn parse_locks(bytes: &[u8], identity: Identity) -> Result<Snapshot, &'static str> {
    if bytes.len() > 1024 * 1024 {
        return Err("proc read bound");
    }
    let text = std::str::from_utf8(bytes).map_err(|_| "malformed proc data")?;
    let wanted = (identity.major, identity.minor, identity.inode);
    let mut holder = None;
    let mut waiters = 0;
    for line in text.lines() {
        if line.len() > 4096 {
            return Err("proc line bound");
        }
        let fields: Vec<_> = line.split_whitespace().collect();
        if fields.get(1) == Some(&"->") {
            if fields.len() != 9 || !fields[0].ends_with(':') {
                return Err("malformed proc data");
            }
            if fields[2] != "FLOCK" {
                continue;
            }
            if device(fields[6])? != wanted {
                continue;
            }
            if fields[3] != "ADVISORY"
                || fields[4] != "WRITE"
                || fields[7] != "0"
                || fields[8] != "EOF"
            {
                return Err("malformed FLOCK waiter");
            }
            pid(fields[5])?;
            waiters += 1;
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
        let found = pid(fields[4])?;
        if device(fields[5])? == wanted && holder.replace(found).is_some() {
            return Err("ambiguous proc match");
        }
    }
    Ok(Snapshot {
        pid: holder.ok_or("no matching granted FLOCK")?,
        waiters,
    })
}
/// A process's argv, NUL separators as spaces, at most `COMMAND_CHARS` characters
/// and debug-quoted so control characters and quotes stay escaped.
const COMMAND_CHARS: usize = 160;
fn command(proc: &Path, pid: u32) -> Result<String, &'static str> {
    let file =
        File::open(proc.join(pid.to_string()).join("cmdline")).map_err(|_| "cmdline unreadable")?;
    let mut bytes = Vec::new();
    file.take(64 * 1024)
        .read_to_end(&mut bytes)
        .map_err(|_| "cmdline unreadable")?;
    let argv: Vec<_> = bytes
        .split(|b| *b == 0)
        .filter(|arg| !arg.is_empty())
        .map(String::from_utf8_lossy)
        .collect();
    if argv.is_empty() {
        return Err("empty cmdline");
    }
    let joined = argv.join(" ");
    let mut shown: String = joined.chars().take(COMMAND_CHARS).collect();
    if joined.chars().count() > COMMAND_CHARS {
        shown.push_str("...");
    }
    Ok(format!("{shown:?}"))
}
/// The children of every thread of `pid`; `None` when the process has exited.
fn children(proc: &Path, pid: u32) -> Result<Option<Vec<u32>>, &'static str> {
    let tasks = match std::fs::read_dir(proc.join(pid.to_string()).join("task")) {
        Ok(tasks) => tasks,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err("proc task list unreadable"),
    };
    let mut found = Vec::new();
    for task in tasks {
        let task = task.map_err(|_| "proc task list unreadable")?;
        let text = match std::fs::read_to_string(task.path().join("children")) {
            Ok(text) => text,
            // A thread that exited while listing has no children to report.
            Err(e) if e.kind() == io::ErrorKind::NotFound => continue,
            Err(_) => return Err("proc children unreadable"),
        };
        for child in text.split_whitespace() {
            found.push(child.parse().map_err(|_| "malformed proc children")?);
        }
    }
    Ok(Some(found))
}
/// Follows the holder's descendants while each has exactly one child, so the deepest
/// process names the phase (a pre-commit hook, the check it started). `None` when the
/// holder runs no single descendant chain.
fn running(proc: &Path, holder: u32) -> Result<Option<u32>, &'static str> {
    let mut chain = vec![holder];
    for _ in 0..16 {
        let current = chain[chain.len() - 1];
        match children(proc, current)? {
            None if current == holder => return Err("holder exited"),
            // Exited since its parent listed it: the parent is the deepest live process.
            None => {
                chain.pop();
                break;
            }
            Some(next) => {
                let [only] = next[..] else { break };
                chain.push(only);
            }
        }
    }
    let deepest = chain[chain.len() - 1];
    Ok((deepest != holder).then_some(deepest))
}
fn observation(proc: &Path, identity: Option<Identity>) -> String {
    let snapshot = match locks(proc, identity) {
        Ok(snapshot) => snapshot,
        Err(reason) => return format!("holder unavailable ({reason})"),
    };
    let mut out = format!("holder PID {} (kernel snapshot)", snapshot.pid);
    match command(proc, snapshot.pid) {
        Ok(command) => out.push_str(&format!(" cmd={command}")),
        Err(reason) => out.push_str(&format!(" cmd unavailable ({reason})")),
    }
    match running(proc, snapshot.pid) {
        Ok(None) => {}
        Ok(Some(pid)) => match command(proc, pid) {
            Ok(command) => out.push_str(&format!(" running={command}")),
            Err(reason) => out.push_str(&format!(" running unavailable ({reason})")),
        },
        Err(reason) => out.push_str(&format!(" running unavailable ({reason})")),
    }
    out.push_str(&format!(" waiters={}", snapshot.waiters));
    out
}
fn locks(proc: &Path, identity: Option<Identity>) -> Result<Snapshot, &'static str> {
    #[cfg(target_os = "linux")]
    {
        let identity = identity.ok_or("file identity unavailable")?;
        let file = File::open(proc.join("locks")).map_err(|_| "proc unavailable")?;
        let mut bytes = Vec::new();
        file.take(1024 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| "proc read failed")?;
        parse_locks(&bytes, identity)
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (proc, identity);
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
                        let observation = observation(Path::new("/proc"), identity);
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
    fn identity() -> Identity {
        Identity {
            major: 0,
            minor: 42,
            inode: 100,
        }
    }
    #[test]
    fn lock_wait_proc_holder() {
        let identity = identity();
        let entry = "1: FLOCK ADVISORY WRITE 123 00:2a:100 0 EOF\n";
        let only = |pid| Ok(Snapshot { pid, waiters: 0 });
        assert_eq!(parse_locks(entry.as_bytes(), identity), only(123));
        assert_eq!(
            parse_locks(entry.replace("123", "456").as_bytes(), identity),
            only(456)
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
            assert!(parse_locks(bad.as_bytes(), identity).is_err(), "{bad}");
        }
        assert!(parse_locks(format!("{entry}{entry}").as_bytes(), identity).is_err());
        assert!(parse_locks(&vec![b' '; 1024 * 1024 + 1], identity).is_err());
        assert!(parse_locks(&vec![b' '; 4097], identity).is_err());
    }
    #[test]
    fn lock_wait_proc_waiters() {
        // Shape observed on Linux 7.2: nested waiters indent further, still `->`.
        let text = "\
            1: FLOCK  ADVISORY  WRITE 123 00:2a:100 0 EOF\n\
            1: -> FLOCK  ADVISORY  WRITE 124 00:2a:100 0 EOF\n\
            1:  -> FLOCK  ADVISORY  WRITE 125 00:2a:100 0 EOF\n\
            2: FLOCK  ADVISORY  WRITE 200 00:2a:101 0 EOF\n\
            2: -> FLOCK  ADVISORY  WRITE 201 00:2a:101 0 EOF\n\
            3: POSIX  ADVISORY  WRITE 300 00:2a:102 0 EOF\n\
            3: -> POSIX  ADVISORY  WRITE 301 00:2a:100 0 EOF\n";
        assert_eq!(
            parse_locks(text.as_bytes(), identity()),
            Ok(Snapshot {
                pid: 123,
                waiters: 2
            })
        );
        let holder = "1: FLOCK ADVISORY WRITE 123 00:2a:100 0 EOF\n";
        for bad in [
            "1: -> FLOCK ADVISORY WRITE 124 00:2a:100\n",
            "1: -> FLOCK ADVISORY READ 124 00:2a:100 0 EOF\n",
            "1: -> FLOCK ADVISORY WRITE 0 00:2a:100 0 EOF\n",
            "1: -> FLOCK ADVISORY WRITE 124 zz:2a:100 0 EOF\n",
        ] {
            assert!(
                parse_locks(format!("{holder}{bad}").as_bytes(), identity()).is_err(),
                "{bad}"
            );
        }
    }
    fn process(proc: &Path, pid: u32, argv: &[u8], children: &[(u32, &str)]) {
        let dir = proc.join(pid.to_string());
        std::fs::create_dir_all(dir.join("task")).unwrap();
        std::fs::write(dir.join("cmdline"), argv).unwrap();
        for (tid, list) in children {
            let task = dir.join("task").join(tid.to_string());
            std::fs::create_dir_all(&task).unwrap();
            std::fs::write(task.join("children"), list).unwrap();
        }
    }
    #[test]
    fn lock_wait_proc_command_and_phase() {
        let root = tempfile::tempdir().unwrap();
        let proc = root.path();
        std::fs::write(
            proc.join("locks"),
            "1: FLOCK ADVISORY WRITE 10 00:2a:100 0 EOF\n\
             1: -> FLOCK ADVISORY WRITE 99 00:2a:100 0 EOF\n",
        )
        .unwrap();
        // The holder's second thread started the commit; the hook chain is single.
        process(
            proc,
            10,
            b"tasks\0note\0band-1\0\"x\"\0",
            &[(10, ""), (11, "20 ")],
        );
        process(proc, 20, b"git\0commit\0", &[(20, "30")]);
        process(proc, 30, b"python3\0.githooks/pre-commit\0", &[(30, "40")]);
        process(proc, 40, b"tasks\0check\0-C\0band\n\0", &[(40, "50 51")]);
        assert_eq!(
            observation(proc, Some(identity())),
            "holder PID 10 (kernel snapshot) cmd=\"tasks note band-1 \\\"x\\\"\" \
             running=\"tasks check -C band\\n\" waiters=1"
        );
        assert_eq!(running(proc, 40), Ok(None), "branching stops at the holder");
        let long = vec![b'a'; COMMAND_CHARS + 5];
        process(proc, 60, &long, &[(60, "")]);
        assert_eq!(
            command(proc, 60).unwrap(),
            format!("\"{}...\"", "a".repeat(COMMAND_CHARS))
        );
        process(proc, 61, b"\0", &[]);
        assert_eq!(command(proc, 61), Err("empty cmdline"));
        assert_eq!(command(proc, 62), Err("cmdline unreadable"));
        assert_eq!(running(proc, 62), Err("holder exited"));
        // A descendant that exits mid-walk leaves its parent as the deepest process.
        process(proc, 70, b"tasks\0", &[(70, "71")]);
        process(proc, 71, b"git\0", &[(71, "72")]);
        assert_eq!(running(proc, 70), Ok(Some(71)));
        process(proc, 80, b"tasks\0", &[(80, "x")]);
        assert_eq!(running(proc, 80), Err("malformed proc children"));
        std::fs::write(
            proc.join("locks"),
            "1: FLOCK ADVISORY WRITE 70 00:2a:100 0 EOF\n",
        )
        .unwrap();
        assert_eq!(
            observation(proc, Some(identity())),
            "holder PID 70 (kernel snapshot) cmd=\"tasks\" running=\"git\" waiters=0"
        );
        std::fs::write(
            proc.join("locks"),
            "1: FLOCK ADVISORY WRITE 62 00:2a:100 0 EOF\n",
        )
        .unwrap();
        assert_eq!(
            observation(proc, Some(identity())),
            "holder PID 62 (kernel snapshot) cmd unavailable (cmdline unreadable) \
             running unavailable (holder exited) waiters=0"
        );
        assert_eq!(
            observation(proc, None),
            "holder unavailable (file identity unavailable)"
        );
    }
    #[test]
    fn lock_wait_reporter_and_proc_unavailable() {
        assert!(locks(Path::new("/proc"), None).is_err());
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
