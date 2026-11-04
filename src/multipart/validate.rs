use super::{ValidatorSpec, content::sha256_hex};
use serde::Serialize;
use std::fs;
use std::io::{self, Read, Write};
use std::os::unix::fs::DirBuilderExt;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

#[derive(Serialize)]
pub struct Validation {
    pub basis: &'static str,
    pub passed: bool,
    pub config_sha256: String,
    pub validators: Vec<ValidatorRun>,
}
#[derive(Serialize)]
pub struct ValidatorRun {
    #[serde(rename = "match")]
    pub glob: String,
    pub command: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub program_sha256: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub program: Option<&'static str>,
    pub exit: Option<i32>,
    pub outcome: &'static str,
    pub cleaned: Vec<u32>,
    pub survivors: Vec<u32>,
    pub stderr: String,
}
struct Sweep {
    cleaned: Vec<u32>,
    survivors: Vec<u32>,
    incomplete: Option<String>,
}
#[cfg(test)]
static SUBREAPER_CALLS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
static VALIDATION: Mutex<()> = Mutex::new(());
static SUBREAPER: OnceLock<Result<(), i32>> = OnceLock::new();
const POLL: Duration = Duration::from_millis(10);
const OUTPUT_CAP: usize = 64 * 1024;

fn run_with<T>(spawner: impl FnOnce() -> T) -> T {
    let _lock = VALIDATION.lock().expect("validator runner lock poisoned");
    spawner()
}

// Call on the command's main thread after waiting for earlier subprocesses;
// no other tasks code may create or reap children on another thread during this call.
pub fn run_declared(
    validators: &[ValidatorSpec],
    config_bytes: &[u8],
    toplevel: &Path,
    name: &str,
    whole: &[u8],
    timeout: Duration,
) -> Validation {
    run_with(|| {
        let entry = children()
            .and_then(|pids| {
                if pids.is_empty() {
                    Ok(())
                } else {
                    Err(io::Error::other(format!(
                        "foreign children exist; refusing without signalling or reaping: {pids:?}"
                    )))
                }
            })
            .and_then(|()| subreaper());
        let mut runs = Vec::new();
        for validator in validators {
            let matching = matches_name(&validator.glob, name);
            if matches!(matching, Ok(false)) {
                continue;
            }
            let mut run = report(validator, name, toplevel);
            if let Err(error) = &entry {
                run.stderr = error.to_string();
            } else if let Err(error) = matching {
                run.stderr = error.to_string();
            } else if run.stderr.is_empty()
                && let Err(error) = execute(&mut run, toplevel, name, whole, timeout)
            {
                run.outcome = "unavailable";
                append_error(&mut run, &error.to_string());
            }
            let unavailable = run.outcome == "unavailable";
            runs.push(run);
            if unavailable {
                break;
            }
        }
        Validation {
            basis: "current_runtime",
            passed: entry.is_ok() && runs.iter().all(|r| r.outcome == "passed"),
            config_sha256: sha256_hex(config_bytes),
            validators: runs,
        }
    })
}

fn matches_name(glob: &str, name: &str) -> io::Result<bool> {
    let glob = std::ffi::CString::new(glob)?;
    let name = std::ffi::CString::new(name)?;
    Ok(unsafe { libc::fnmatch(glob.as_ptr(), name.as_ptr(), 0) } == 0)
}

fn program_identity(
    program: &str,
    toplevel: &Path,
) -> io::Result<(Option<String>, Option<&'static str>)> {
    let path = if program.contains('/') {
        toplevel.join(program)
    } else {
        use std::os::unix::ffi::OsStrExt;
        let search = std::env::var_os("PATH").ok_or_else(|| io::Error::other("PATH is not set"))?;
        let mut selected = None;
        for entry in std::env::split_paths(&search) {
            // Relative and empty PATH entries resolve in the validator's cwd.
            let candidate = toplevel.join(entry).join(program);
            match fs::metadata(&candidate) {
                Ok(metadata) if !metadata.is_file() => continue,
                Ok(_) => (),
                Err(error)
                    if matches!(
                        error.raw_os_error(),
                        Some(libc::ENOENT | libc::ENOTDIR | libc::EACCES)
                    ) =>
                {
                    continue;
                }
                Err(error) => return Err(error),
            }
            let path = std::ffi::CString::new(candidate.as_os_str().as_bytes())?;
            if unsafe {
                libc::faccessat(libc::AT_FDCWD, path.as_ptr(), libc::X_OK, libc::AT_EACCESS)
            } == 0
            {
                selected = Some(candidate);
                break;
            }
            let error = io::Error::last_os_error();
            if !matches!(
                error.raw_os_error(),
                Some(libc::ENOENT | libc::ENOTDIR | libc::EACCES)
            ) {
                return Err(error);
            }
        }
        selected.ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                format!("validator program {program:?} is not executable on PATH"),
            )
        })?
    }
    .canonicalize()?;
    if path.starts_with(toplevel.canonicalize()?) {
        Ok((Some(sha256_hex(&fs::read(path)?)), None))
    } else {
        Ok((None, Some("external")))
    }
}

fn report(spec: &ValidatorSpec, name: &str, toplevel: &Path) -> ValidatorRun {
    let command: Vec<_> = spec
        .command
        .iter()
        .map(|arg| arg.replace("{name}", name))
        .collect();
    let identity = command
        .first()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| io::Error::other("validator command is empty"))
        .and_then(|program| program_identity(program, toplevel));
    let (program_sha256, program, stderr) = match identity {
        Ok((hash, external)) => (hash, external, String::new()),
        Err(error) => (None, None, error.to_string()),
    };
    ValidatorRun {
        glob: spec.glob.clone(),
        command,
        program_sha256,
        program,
        exit: None,
        outcome: "unavailable",
        cleaned: Vec::new(),
        survivors: Vec::new(),
        stderr,
    }
}

fn subreaper() -> io::Result<()> {
    match SUBREAPER.get_or_init(|| {
        #[cfg(test)]
        SUBREAPER_CALLS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        if unsafe { libc::prctl(libc::PR_SET_CHILD_SUBREAPER, 1, 0, 0, 0) } == 0 {
            Ok(())
        } else {
            Err(io::Error::last_os_error()
                .raw_os_error()
                .unwrap_or(libc::EIO))
        }
    }) {
        Ok(()) => Ok(()),
        Err(errno) => Err(io::Error::from_raw_os_error(*errno)),
    }
}

fn existing_stat(result: io::Result<Vec<u8>>) -> io::Result<Option<Vec<u8>>> {
    match result {
        Ok(stat) => Ok(Some(stat)),
        // procfs returns ESRCH when the process exits between open and read.
        Err(error)
            if error.kind() == io::ErrorKind::NotFound
                || error.raw_os_error() == Some(libc::ESRCH) =>
        {
            Ok(None)
        }
        Err(error) => Err(error),
    }
}

fn parent_pid(stat: &[u8]) -> io::Result<u32> {
    let name_end = stat
        .iter()
        .rposition(|byte| *byte == b')')
        .ok_or_else(|| io::Error::other("invalid /proc stat"))?;
    // Linux comm is arbitrary bytes; only the fields after its final ')' are text.
    let fields = std::str::from_utf8(&stat[name_end + 1..])
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    fields
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse::<u32>().ok())
        .ok_or_else(|| io::Error::other("invalid /proc parent pid"))
}

fn children() -> io::Result<Vec<u32>> {
    let parent = std::process::id();
    let mut children = Vec::new();
    for entry in fs::read_dir("/proc")? {
        let entry = entry?;
        let Some(pid) = entry
            .file_name()
            .to_str()
            .and_then(|s| s.parse::<u32>().ok())
        else {
            continue;
        };
        let Some(stat) = existing_stat(fs::read(entry.path().join("stat")))? else {
            continue;
        };
        let ppid = parent_pid(&stat)?;
        if ppid == parent {
            children.push(pid);
        }
    }
    children.sort_unstable();
    Ok(children)
}

fn sweep(
    mut list_children: impl FnMut() -> io::Result<Vec<u32>>,
    mut kill: impl FnMut(u32),
    mut try_reap: impl FnMut(u32) -> bool,
    mut known: Vec<u32>,
    deadline: Duration,
) -> Sweep {
    let start = Instant::now();
    let mut cleaned = std::collections::BTreeSet::new();
    loop {
        let incomplete = match list_children() {
            Ok(pids) => {
                known = pids;
                None
            }
            Err(error) => Some(error.to_string()),
        };
        let current = known.clone();
        if current.is_empty() && incomplete.is_none() {
            return Sweep {
                cleaned: cleaned.into_iter().collect(),
                survivors: Vec::new(),
                incomplete: None,
            };
        }
        if start.elapsed() >= deadline {
            return Sweep {
                cleaned: cleaned
                    .into_iter()
                    .filter(|pid| !current.contains(pid))
                    .collect(),
                survivors: current,
                incomplete,
            };
        }
        for pid in current {
            kill(pid);
            if try_reap(pid) {
                cleaned.insert(pid);
                known.retain(|child| *child != pid);
            }
        }
        thread::sleep(POLL.min(deadline.saturating_sub(start.elapsed())));
    }
}

fn append_error(run: &mut ValidatorRun, message: &str) {
    if !run.stderr.is_empty() {
        run.stderr.push('\n');
    }
    run.stderr.push_str(message);
}

fn report_survivors(run: &mut ValidatorRun, result: Sweep) {
    run.cleaned = result.cleaned;
    run.survivors = result.survivors;
    if let Some(error) = result.incomplete {
        run.outcome = "unavailable";
        append_error(
            run,
            &format!(
                "cleanup incomplete: child discovery failed: {error}; additional surviving PIDs are unknown"
            ),
        );
    }
    if !run.survivors.is_empty() {
        run.outcome = "unavailable";
        let pids = run
            .survivors
            .iter()
            .map(u32::to_string)
            .collect::<Vec<_>>()
            .join(" ");
        append_error(
            run,
            &format!("validator children survived cleanup: {pids}; stop with: kill -9 {pids}"),
        );
    }
}

struct Home(PathBuf);
impl Home {
    fn create() -> io::Result<Self> {
        let path = std::env::temp_dir().join(format!(
            "tasks-validator-{}-{:016x}",
            std::process::id(),
            fastrand::u64(..)
        ));
        fs::DirBuilder::new().mode(0o700).create(&path)?;
        Ok(Self(path))
    }
}
impl Drop for Home {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn drain(mut pipe: impl Read) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    let mut buffer = [0; 8192];
    loop {
        match pipe.read(&mut buffer) {
            Ok(0) => return Ok(bytes),
            Ok(n) => bytes.extend_from_slice(&buffer[..n.min(OUTPUT_CAP - bytes.len())]),
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error),
        }
    }
}

type IoJob = Box<dyn FnOnce() -> io::Result<Vec<u8>> + Send>;
type IoThread = (&'static str, thread::JoinHandle<io::Result<Vec<u8>>>);
fn execute(
    run: &mut ValidatorRun,
    toplevel: &Path,
    name: &str,
    whole: &[u8],
    timeout: Duration,
) -> io::Result<()> {
    let home = Home::create()?;
    let mut child = Command::new(&run.command[0])
        .args(&run.command[1..])
        .current_dir(toplevel)
        .env_clear()
        .env(
            "PATH",
            std::env::var_os("PATH").ok_or_else(|| io::Error::other("PATH is not set"))?,
        )
        .env("HOME", &home.0)
        .env("LANG", "C")
        .env("TASKS_ATTACHMENT_NAME", name)
        .process_group(0)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let leader = child.id();
    let mut threads: Vec<IoThread> = Vec::new();
    let input = whole.to_vec();
    let mut stdin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();
    let jobs: [(&str, IoJob); 3] = [
        (
            "stdin",
            Box::new(move || match stdin.write_all(&input) {
                Ok(()) => Ok(Vec::new()),
                Err(error) if error.kind() == io::ErrorKind::BrokenPipe => Ok(Vec::new()),
                Err(error) => Err(error),
            }),
        ),
        ("stdout", Box::new(move || drain(stdout))),
        ("stderr", Box::new(move || drain(stderr))),
    ];
    let mut failure = None;
    for (name, job) in jobs {
        match thread::Builder::new()
            .name(format!("validator-{name}"))
            .spawn(job)
        {
            Ok(handle) => threads.push((name, handle)),
            Err(error) => {
                failure = Some(error);
                break;
            }
        }
    }
    let start = Instant::now();
    run.outcome = "timeout";
    while failure.is_none() && start.elapsed() < timeout {
        let mut info: libc::siginfo_t = unsafe { std::mem::zeroed() };
        // WNOWAIT keeps the leader's pid reserved until killpg and the sweep.
        let result = unsafe {
            libc::waitid(
                libc::P_PID,
                leader,
                &mut info,
                libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
            )
        };
        if result < 0 {
            let error = io::Error::last_os_error();
            if error.kind() != io::ErrorKind::Interrupted {
                failure = Some(error);
            }
        } else if unsafe { info.si_pid() } != 0 {
            run.outcome = if info.si_code == libc::CLD_EXITED && unsafe { info.si_status() } == 0 {
                "passed"
            } else {
                "rejected"
            };
            break;
        }
        thread::sleep(POLL.min(timeout.saturating_sub(start.elapsed())));
    }
    unsafe {
        libc::killpg(leader as i32, libc::SIGKILL);
    }
    let result = sweep(
        children,
        |pid| unsafe {
            libc::kill(pid as i32, libc::SIGKILL);
        },
        |pid| {
            let mut status = 0;
            let result = unsafe { libc::waitpid(pid as i32, &mut status, libc::WNOHANG) };
            if result == pid as i32 {
                if pid == leader && libc::WIFEXITED(status) {
                    run.exit = Some(libc::WEXITSTATUS(status));
                }
                true
            } else {
                false
            }
        },
        vec![leader],
        Duration::from_secs(5),
    );
    let complete = result.incomplete.is_none() && result.survivors.is_empty();
    report_survivors(run, result);
    run.cleaned.retain(|pid| *pid != leader);
    if let Err(error) = finish_io(run, threads, complete, Duration::from_secs(5)) {
        failure = Some(error);
    }
    fs::remove_dir_all(&home.0)?;
    if let Some(error) = failure {
        return Err(error);
    }
    Ok(())
}

fn finish_io(
    run: &mut ValidatorRun,
    threads: Vec<IoThread>,
    complete: bool,
    deadline: Duration,
) -> io::Result<()> {
    let mut failure = None;
    let join_start = Instant::now();
    while threads.iter().any(|(_, handle)| !handle.is_finished()) && join_start.elapsed() < deadline
    {
        thread::sleep(POLL);
    }
    for (name, handle) in threads {
        if !handle.is_finished() && !complete {
            run.outcome = "unavailable";
            append_error(
                run,
                &format!("detached validator-{name} I/O thread: pipe did not close after cleanup"),
            );
            continue;
        }
        match handle.join() {
            Ok(Ok(bytes)) if name == "stderr" => {
                let diagnostics = std::mem::replace(
                    &mut run.stderr,
                    String::from_utf8_lossy(&bytes).into_owned(),
                );
                if !diagnostics.is_empty() {
                    append_error(run, &diagnostics);
                }
            }
            Ok(Ok(_)) => (),
            Ok(Err(error)) => {
                failure = Some(error);
            }
            Err(_) => {
                failure = Some(io::Error::other(format!(
                    "validator-{name} thread panicked"
                )));
            }
        }
    }
    match failure {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::{Child, Output};
    use std::sync::mpsc;
    use tempfile::TempDir;

    #[test]
    fn exited_process_stat_is_absent_even_if_it_disappears_after_open() {
        assert!(
            existing_stat(Err(io::Error::from_raw_os_error(libc::ESRCH)))
                .unwrap()
                .is_none()
        );
        assert!(
            existing_stat(Err(io::Error::from_raw_os_error(libc::ENOENT)))
                .unwrap()
                .is_none()
        );
        assert!(existing_stat(Err(io::Error::from_raw_os_error(libc::EACCES))).is_err());
    }

    #[test]
    fn process_parent_does_not_require_a_utf8_name() {
        assert_eq!(parent_pid(b"123 (validator-\xff) S 42 1 2 3").unwrap(), 42);
        assert_eq!(parent_pid(b"123 (val) \xff) S 42 1 2 3").unwrap(), 42);
        assert!(parent_pid(b"123 (name) S nope").is_err());
    }

    #[test]
    fn failed_scans_after_reaping_the_known_leader_are_not_empty_cleanup() {
        let mut scans = 0;
        let mut signalled = Vec::new();
        let start = Instant::now();
        let result = sweep(
            || {
                scans += 1;
                Err(io::Error::other("injected scan failure"))
            },
            |pid| signalled.push(pid),
            |_| true,
            vec![4242],
            Duration::from_millis(35),
        );
        assert!(start.elapsed() >= Duration::from_millis(35));
        assert!(start.elapsed() < Duration::from_secs(1));
        assert!(scans >= 3);
        assert_eq!(signalled, [4242]);
        assert_eq!(result.cleaned, [4242]);
        assert!(result.survivors.is_empty());
        assert!(result.incomplete.unwrap().contains("injected scan failure"));
    }

    #[test]
    fn a_complete_scan_can_establish_cleanup_after_discovery_recovers() {
        let mut scans = 0;
        let cleanup = sweep(
            || {
                scans += 1;
                if scans < 3 {
                    Err(io::Error::other("temporary scan failure"))
                } else {
                    Ok(vec![])
                }
            },
            |_| {},
            |_| true,
            vec![4242],
            Duration::from_secs(1),
        );
        assert_eq!(scans, 3);
        assert!(cleanup.incomplete.is_none());
        assert!(cleanup.survivors.is_empty());
        assert_eq!(cleanup.cleaned, [4242]);
    }

    #[test]
    fn incomplete_cleanup_reports_and_detaches_an_unfinished_pipe_reader() {
        let (reader, writer) = std::os::unix::net::UnixStream::pair().unwrap();
        let (finished_tx, finished_rx) = mpsc::channel();
        let reader = thread::spawn(move || {
            let result = drain(reader);
            finished_tx.send(()).unwrap();
            result
        });
        // The fallback release bounds the RED test even if production joins blindly.
        let (release_tx, release_rx) = mpsc::channel();
        let holder = thread::spawn(move || {
            let _ = release_rx.recv_timeout(Duration::from_millis(300));
            drop(writer);
        });
        let mut run = report(&spec("true"), "a.txt", Path::new("/"));
        run.outcome = "passed";
        let start = Instant::now();
        let mut scans = 0;
        let mut signalled = Vec::new();
        let cleanup = sweep(
            || {
                scans += 1;
                Err(io::Error::other("injected scan failure"))
            },
            |pid| signalled.push(pid),
            |_| true,
            vec![4242],
            Duration::from_millis(35),
        );
        let complete = cleanup.incomplete.is_none() && cleanup.survivors.is_empty();
        report_survivors(&mut run, cleanup);
        let result = finish_io(
            &mut run,
            vec![("stdout", reader)],
            complete,
            Duration::from_millis(20),
        );
        let elapsed = start.elapsed();
        let _ = release_tx.send(());
        holder.join().unwrap();
        finished_rx.recv_timeout(Duration::from_secs(1)).unwrap();
        result.unwrap();
        assert!(scans >= 3);
        assert_eq!(signalled, [4242]);
        assert!(!complete);
        assert!(
            elapsed < Duration::from_millis(200),
            "blocked for {elapsed:?}"
        );
        assert_eq!(run.outcome, "unavailable");
        assert!(run.stderr.contains("cleanup incomplete"), "{}", run.stderr);
        assert!(run.stderr.contains("unknown"), "{}", run.stderr);
        assert!(
            run.stderr.contains("detached validator-stdout"),
            "{}",
            run.stderr
        );
    }

    #[test]
    fn sweep_reports_unkillable_survivors() {
        let start = Instant::now();
        let result = sweep(
            || Ok(vec![4242]),
            |_| {},
            |_| false,
            vec![],
            Duration::from_millis(30),
        );
        assert_eq!(result.survivors, [4242]);
        assert!(result.cleaned.is_empty());
        let mut run = report(&spec("true"), "a.txt", Path::new("/"));
        report_survivors(&mut run, result);
        assert_eq!(run.outcome, "unavailable");
        assert!(run.stderr.contains("kill -9 4242"));
        assert!(start.elapsed() < Duration::from_secs(1));
    }

    #[test]
    fn sweep_polls_without_blocking() {
        let mut polls = 0;
        let start = Instant::now();
        sweep(
            || Ok(vec![4242]),
            |_| {},
            |_| {
                polls += 1;
                false
            },
            vec![],
            Duration::from_millis(35),
        );
        assert!(polls >= 3);
        assert!(start.elapsed() < Duration::from_secs(1));
    }

    #[test]
    fn only_matching_globs_run() {
        assert!(matches_name("*.json", "whole.json").unwrap());
        assert!(!matches_name("*.json", "whole.md").unwrap());
        assert!(matches_name("report-?.[mt][dx]", "report-1.md").unwrap());
        assert!(matches_name("*\0", "a").is_err());
    }

    #[test]
    fn program_hash_is_reported_for_repository_programs_only() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("check"), b"abc").unwrap();
        let (hash, external) = program_identity("./check", dir.path()).unwrap();
        assert_eq!(
            hash.as_deref(),
            Some("ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad")
        );
        assert_eq!(external, None);
        assert_eq!(
            program_identity("sh", dir.path()).unwrap(),
            (None, Some("external"))
        );
        std::os::unix::fs::symlink("/bin/sh", dir.path().join("outside")).unwrap();
        assert_eq!(
            program_identity("./outside", dir.path()).unwrap(),
            (None, Some("external"))
        );
        assert!(program_identity("./missing", dir.path()).is_err());
    }

    #[test]
    fn run_declared_holds_the_validation_lock() {
        let (entered_tx, entered_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let first = thread::spawn(move || {
            run_with(|| {
                entered_tx.send(()).unwrap();
                release_rx.recv().unwrap();
            })
        });
        entered_rx.recv().unwrap();
        let (second_tx, second_rx) = mpsc::channel();
        let second = thread::spawn(move || run_with(|| second_tx.send(()).unwrap()));
        assert!(second_rx.recv_timeout(Duration::from_millis(50)).is_err());
        release_tx.send(()).unwrap();
        first.join().unwrap();
        second_rx.recv_timeout(Duration::from_secs(1)).unwrap();
        second.join().unwrap();
    }

    fn spec(script: &str) -> ValidatorSpec {
        ValidatorSpec {
            glob: "*.txt".into(),
            command: vec![
                "/bin/sh".into(),
                "-c".into(),
                script.into(),
                "validator".into(),
                "{name}".into(),
            ],
        }
    }

    fn run(dir: &TempDir, script: &str, bytes: &[u8], timeout: Duration) -> Validation {
        run_declared(
            &[spec(script)],
            b"config",
            dir.path(),
            "whole.txt",
            bytes,
            timeout,
        )
    }

    fn isolated(name: &str, home: &Path) -> Child {
        Command::new(std::env::current_exe().unwrap())
            .args([
                "multipart::validate::tests::isolated_validator_entry",
                "--exact",
                "--test-threads=1",
                "--nocapture",
            ])
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .env("HOME", home)
            .env("LANG", "C")
            .env("TASKS_VALIDATOR_SCENARIO", name)
            .env("FAKE_CREDENTIAL", "must-not-reach-validator")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap()
    }

    fn proof(output: Output, name: &str) {
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8(output.stdout).unwrap();
        let line = stdout
            .lines()
            .find_map(|line| line.strip_prefix("VALIDATOR_RESULT "))
            .expect("isolated test must emit its JSON proof");
        let value: serde_json::Value = serde_json::from_str(line).unwrap();
        assert_eq!(value["scenario"], name);
        assert_eq!(value["passed"], true);
    }

    macro_rules! scenario_test {
        ($($name:ident),+ $(,)?) => { $(#[test] fn $name() {
            let home = TempDir::new().unwrap();
            proof(isolated(stringify!($name), home.path()).wait_with_output().unwrap(), stringify!($name));
        })+ };
    }
    scenario_test!(
        validator_receives_the_whole_on_stdin_and_the_name,
        validator_environment_is_built_key_by_key,
        validator_rejection_and_timeout_are_reported,
        validator_output_is_capped_and_unread_stdin_does_not_hang,
        validator_descendants_holding_pipes_are_killed,
        validator_setsid_and_double_fork_descendants_are_cleaned,
        subreaper_is_set_once,
        non_utf8_validator_and_setsid_descendant_are_cleaned,
        validation_refuses_when_foreign_children_exist,
    );

    #[test]
    fn concurrent_isolated_runs_do_not_touch_each_other() {
        let home = TempDir::new().unwrap();
        let a = isolated("concurrent_a", home.path());
        let b = isolated("concurrent_b", home.path());
        let a = a.wait_with_output().unwrap();
        let b = b.wait_with_output().unwrap();
        proof(a, "concurrent_a");
        proof(b, "concurrent_b");
    }

    struct EmergencyCleanup(Vec<u32>);
    impl Drop for EmergencyCleanup {
        fn drop(&mut self) {
            for pid in &self.0 {
                unsafe {
                    libc::kill(*pid as i32, libc::SIGKILL);
                }
            }
        }
    }

    fn assert_descendants_gone(dir: &TempDir, validation: &Validation) {
        let pids: Vec<u32> = fs::read_to_string(dir.path().join("pids"))
            .unwrap()
            .split_whitespace()
            .map(|s| s.parse().unwrap())
            .collect();
        let mut guard = EmergencyCleanup(pids.clone());
        assert!(!pids.is_empty());
        for pid in pids {
            assert!(
                validation.validators[0].cleaned.contains(&pid),
                "missing {pid}: {}",
                serde_json::to_string(validation).unwrap()
            );
            assert_eq!(unsafe { libc::kill(pid as i32, 0) }, -1);
            assert_eq!(io::Error::last_os_error().raw_os_error(), Some(libc::ESRCH));
        }
        guard.0.clear();
    }

    #[test]
    fn isolated_validator_entry() {
        let Ok(scenario) = std::env::var("TASKS_VALIDATOR_SCENARIO") else {
            return;
        };
        let dir = TempDir::new().unwrap();
        let short = Duration::from_millis(150);
        let normal = Duration::from_secs(2);
        match scenario.as_str() {
            "validator_receives_the_whole_on_stdin_and_the_name" => {
                let value = run(
                    &dir,
                    "cat > received; printf '%s|%s' \"$1\" \"$TASKS_ATTACHMENT_NAME\" >&2",
                    b"whole\nbody",
                    normal,
                );
                assert!(value.passed, "{}", serde_json::to_string(&value).unwrap());
                assert_eq!(value.validators[0].stderr, "whole.txt|whole.txt");
                assert_eq!(
                    fs::read(dir.path().join("received")).unwrap(),
                    b"whole\nbody"
                );
                assert_eq!(value.basis, "current_runtime");
                assert_eq!(value.config_sha256, sha256_hex(b"config"));
                let skipped =
                    run_declared(&[spec("exit 3")], b"", dir.path(), "file.json", b"", normal);
                assert!(skipped.passed && skipped.validators.is_empty());
            }
            "validator_environment_is_built_key_by_key" => {
                let value = run(&dir, "env >&2; test -d \"$HOME\"", b"", normal);
                assert!(value.passed, "{}", serde_json::to_string(&value).unwrap());
                let vars: std::collections::BTreeMap<_, _> = value.validators[0]
                    .stderr
                    .lines()
                    .map(|s| s.split_once('=').unwrap())
                    .collect();
                for key in vars.keys() {
                    assert!(
                        [
                            "PATH",
                            "HOME",
                            "LANG",
                            "TASKS_ATTACHMENT_NAME",
                            "PWD",
                            "SHLVL",
                            "_"
                        ]
                        .contains(key),
                        "unexpected {key}"
                    );
                }
                for key in ["PATH", "HOME", "LANG", "TASKS_ATTACHMENT_NAME"] {
                    assert!(vars.contains_key(key));
                }
                assert_ne!(vars["HOME"], std::env::var("HOME").unwrap());
                assert!(!Path::new(vars["HOME"]).exists());
            }
            "validator_rejection_and_timeout_are_reported" => {
                let value = run(&dir, "echo bad >&2; exit 3", b"", normal);
                assert!(!value.passed);
                assert_eq!(value.validators[0].outcome, "rejected");
                assert_eq!(value.validators[0].exit, Some(3));
                let start = Instant::now();
                let value = run(&dir, "sleep 30", b"", short);
                assert_eq!(value.validators[0].outcome, "timeout");
                assert!(start.elapsed() < Duration::from_secs(3));
                let value = run_declared(
                    &[
                        ValidatorSpec {
                            glob: "*".into(),
                            command: vec!["/definitely/missing".into()],
                        },
                        spec("touch should-not-run"),
                    ],
                    b"",
                    dir.path(),
                    "a.txt",
                    b"",
                    normal,
                );
                assert_eq!(value.validators[0].outcome, "unavailable");
                assert_eq!(value.validators.len(), 1);
                assert!(!dir.path().join("should-not-run").exists());
            }
            "validator_output_is_capped_and_unread_stdin_does_not_hang" => {
                let start = Instant::now();
                let value = run(
                    &dir,
                    "head -c 200000 /dev/zero; head -c 200000 /dev/zero >&2; sleep 30",
                    &vec![b'a'; 1_000_000],
                    short,
                );
                assert_eq!(value.validators[0].outcome, "timeout");
                assert_eq!(value.validators[0].stderr.len(), 65536);
                assert!(start.elapsed() < Duration::from_secs(3));
            }
            "validator_descendants_holding_pipes_are_killed" => {
                for ending in ["exit 0", "sleep 30"] {
                    let start = Instant::now();
                    let value = run(
                        &dir,
                        &format!("sleep 300 & echo $! > pids; {ending}"),
                        b"",
                        short,
                    );
                    assert_eq!(
                        value.validators[0].outcome,
                        if ending == "exit 0" {
                            "passed"
                        } else {
                            "timeout"
                        }
                    );
                    assert!(start.elapsed() < Duration::from_secs(3));
                    assert_descendants_gone(&dir, &value);
                }
            }
            "validator_setsid_and_double_fork_descendants_are_cleaned" => {
                for code in [
                    "import os,time; p=os.fork(); p and os._exit(0); os.setsid(); open('pids','w').write(str(os.getpid())); time.sleep(300)",
                    "import os,time; p=os.fork(); p and os._exit(0); os.setsid(); p=os.fork(); p and os._exit(0); open('pids','w').write(str(os.getpid())); time.sleep(300)",
                ] {
                    let script =
                        format!("python3 -c \"{code}\"; while test ! -s pids; do sleep 0.01; done");
                    let value = run(&dir, &script, b"", normal);
                    assert!(value.passed, "{}", serde_json::to_string(&value).unwrap());
                    assert_descendants_gone(&dir, &value);
                    fs::remove_file(dir.path().join("pids")).unwrap();
                }
            }
            "non_utf8_validator_and_setsid_descendant_are_cleaned" => {
                let code = r#"import ctypes; assert ctypes.CDLL(None).prctl(15, b'validator-\xff', 0, 0, 0) == 0; assert b'\xff' in open('/proc/self/stat','rb').read()"#;
                let value = run(&dir, &format!("exec python3 -c \"{code}\""), b"", normal);
                assert!(value.passed, "{}", serde_json::to_string(&value).unwrap());
                let code = r#"import ctypes,os,time; p=os.fork(); p and os._exit(0); os.setsid(); assert ctypes.CDLL(None).prctl(15, b'validator-\xff', 0, 0, 0) == 0; assert b'\xff' in open('/proc/self/stat','rb').read(); open('pids','w').write(str(os.getpid())); time.sleep(2)"#;
                let script =
                    format!("python3 -c \"{code}\"; while test ! -s pids; do sleep 0.01; done");
                let value = run(&dir, &script, b"", normal);
                assert_descendants_gone(&dir, &value);
                assert!(value.passed, "{}", serde_json::to_string(&value).unwrap());
            }
            "subreaper_is_set_once" => {
                assert!(run(&dir, "true", b"", normal).passed);
                let mut active = 0;
                assert_eq!(
                    unsafe { libc::prctl(libc::PR_GET_CHILD_SUBREAPER, &mut active) },
                    0
                );
                assert_eq!(active, 1);
                assert!(run(&dir, "true", b"", normal).passed);
                assert_eq!(SUBREAPER_CALLS.load(std::sync::atomic::Ordering::SeqCst), 1);
            }
            "validation_refuses_when_foreign_children_exist" => {
                let mut foreign = Command::new("/bin/sleep")
                    .arg("30")
                    .env_clear()
                    .env("PATH", "/usr/bin:/bin")
                    .env("HOME", dir.path())
                    .spawn()
                    .unwrap();
                let mut guard = EmergencyCleanup(vec![foreign.id()]);
                let value = run(&dir, "touch spawned", b"", normal);
                assert_eq!(value.validators[0].outcome, "unavailable");
                assert!(
                    value.validators[0]
                        .stderr
                        .contains(&foreign.id().to_string())
                );
                assert!(!dir.path().join("spawned").exists());
                assert!(foreign.try_wait().unwrap().is_none());
                foreign.kill().unwrap();
                foreign.wait().unwrap();
                guard.0.clear();
            }
            "concurrent_a" => {
                let value = run(&dir, "sleep 3", b"", Duration::from_secs(5));
                assert!(value.passed, "{}", serde_json::to_string(&value).unwrap());
                assert!(value.validators[0].cleaned.is_empty());
            }
            "concurrent_b" => {
                let start = Instant::now();
                while start.elapsed() < Duration::from_secs(3) {
                    let value = run(&dir, "sleep 300 & echo $! > pids", b"", normal);
                    assert!(value.passed, "{}", serde_json::to_string(&value).unwrap());
                    assert_descendants_gone(&dir, &value);
                    let pid: u32 = fs::read_to_string(dir.path().join("pids"))
                        .unwrap()
                        .trim()
                        .parse()
                        .unwrap();
                    assert_eq!(value.validators[0].cleaned, [pid]);
                }
            }
            other => panic!("unknown scenario {other}"),
        }
        assert!(children().unwrap().is_empty());
        println!(
            "\nVALIDATOR_RESULT {}",
            serde_json::json!({"scenario": scenario, "passed": true})
        );
    }
}
