//! Opt-in synthetic measurements with per-run proofs and logical comparisons.
mod common;
use serde_json::{Value, json};
use std::{
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

#[test]
fn retained_latency_harness_smallest() {
    assert!(configuration(None, None, None, None).unwrap().is_none());
    assert_eq!(git_count_verdict(Mode::Matrix, true), None);
    assert_eq!(git_count_verdict(Mode::Repeat, false), Some(false));
    for invalid in ["", "0", "true"] {
        assert!(configuration(Some(invalid), None, None, None).is_err());
    }
    assert!(configuration(Some("1"), None, None, None).is_err());
    assert!(configuration(Some("1"), Some("/missing"), Some("relative"), None).is_err());
    assert!(configuration(Some("1"), Some("/missing"), Some("/tmp/out"), Some("wrong")).is_err());
    let root = tempfile::tempdir().unwrap();
    let binary = assert_cmd::cargo::cargo_bin("tasks");
    let declared_output = std::env::current_dir()
        .unwrap()
        .join("target/retained-harness-test");
    let mut settings = configuration(
        Some("1"),
        binary.to_str(),
        declared_output.to_str(),
        Some("pilot"),
    )
    .unwrap()
    .unwrap();
    assert_eq!(settings.mode, Mode::Pilot);
    for (name, mode) in [("repeat", Mode::Repeat), ("matrix", Mode::Matrix)] {
        assert_eq!(
            configuration(
                Some("1"),
                binary.to_str(),
                declared_output.to_str(),
                Some(name)
            )
            .unwrap()
            .unwrap()
            .mode,
            mode
        );
    }
    assert!(
        configuration(
            Some("1"),
            binary.to_str(),
            Some("/tmp/retained-harness-test"),
            None
        )
        .is_err()
    );
    // Test-only copies may be ephemeral; operational measurement output must be persistent.
    settings.out = root.path().to_owned();
    assert_eq!(Mode::Repeat.repetitions(), 3);
    assert_eq!(Mode::Matrix.repetitions(), 3);
    let no_request = settings.out.join("not-requested");
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .env_clear()
        .envs(common::isolated_env(settings.out.as_path()))
        .env("TASKS_MEASURE_OUT", &no_request)
        .env("TT_FORCE", "1")
        .args([
            "--exact",
            "retained_latency_measure",
            "--ignored",
            "--nocapture",
        ]);
    let unrequested = Running::start(
        command,
        settings.out.join("no-request.stdout"),
        settings.out.join("no-request.stderr"),
    )
    .unwrap();
    // This is the test harness itself, whose stdout is intentionally plain text.
    let mut unrequested = unrequested;
    let deadline = Instant::now() + Duration::from_secs(10);
    while unrequested
        .child
        .as_mut()
        .unwrap()
        .try_wait()
        .unwrap()
        .is_none()
    {
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(
        unrequested
            .child
            .as_mut()
            .unwrap()
            .wait()
            .unwrap()
            .success()
    );
    assert!(
        fs::read_to_string(&unrequested.stdout)
            .unwrap()
            .contains("retained measurement: not requested")
    );
    assert!(!no_request.exists());
    drop(unrequested);
    let result = smallest(&settings).unwrap();
    assert!(result["requested"].as_bool().unwrap());
    assert!(result["complete"].as_bool().unwrap());
}

#[test]
#[ignore]
fn retained_latency_measure() {
    let env = |key| {
        std::env::var_os(key).map(|v| v.into_string().expect("measurement inputs must be UTF-8"))
    };
    let requested = env("TASKS_RETAINED_MEASURE");
    let baseline = env("TASKS_BASELINE_BIN");
    let out = env("TASKS_MEASURE_OUT");
    let mode = env("TASKS_MEASURE_MODE");
    let Some(settings) = configuration(
        requested.as_deref(),
        baseline.as_deref(),
        out.as_deref(),
        mode.as_deref(),
    )
    .unwrap() else {
        println!("retained measurement: not requested");
        return;
    };
    measure(&settings).unwrap();
}

use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::{Read, Write},
    os::unix::{fs::PermissionsExt, net::UnixListener, process::CommandExt},
    process::{Child, Command, Stdio},
};
type Result<T> = std::result::Result<T, String>;
fn failure(e: impl std::fmt::Display) -> String {
    e.to_string()
}
fn require(ok: bool, what: &str) -> Result<()> {
    if ok { Ok(()) } else { Err(what.into()) }
}
fn digest(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    format!("{:x}", Sha256::digest(bytes))
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum Mode {
    Pilot,
    Repeat,
    Matrix,
}
impl Mode {
    fn repetitions(self) -> usize {
        if self == Self::Pilot { 1 } else { 3 }
    }
    fn name(self) -> &'static str {
        match self {
            Self::Pilot => "pilot",
            Self::Repeat => "repeat",
            Self::Matrix => "matrix",
        }
    }
}
struct Settings {
    baseline: PathBuf,
    out: PathBuf,
    mode: Mode,
}
fn configuration(
    requested: Option<&str>,
    baseline: Option<&str>,
    out: Option<&str>,
    mode: Option<&str>,
) -> Result<Option<Settings>> {
    let Some(requested) = requested else {
        return Ok(None);
    };
    require(
        requested == "1",
        "TASKS_RETAINED_MEASURE must be exactly 1 or absent",
    )?;
    let mode = match mode.unwrap_or("pilot") {
        "pilot" => Mode::Pilot,
        "repeat" => Mode::Repeat,
        "matrix" => Mode::Matrix,
        _ => return Err("TASKS_MEASURE_MODE must be pilot, repeat or matrix".into()),
    };
    let baseline = PathBuf::from(baseline.ok_or("TASKS_BASELINE_BIN required")?);
    let out = PathBuf::from(out.ok_or("TASKS_MEASURE_OUT required")?);
    require(
        baseline.is_absolute()
            && baseline.is_file()
            && fs::metadata(&baseline)
                .map_err(failure)?
                .permissions()
                .mode()
                & 0o111
                != 0,
        "baseline must be an explicit absolute executable file",
    )?;
    require(
        out.is_absolute(),
        "measurement output must be absolute persistent owned storage",
    )?;
    let resolved = if out.exists() {
        out.canonicalize().map_err(failure)?
    } else {
        out.parent()
            .ok_or("output parent required")?
            .canonicalize()
            .map_err(failure)?
            .join(out.file_name().ok_or("output directory name required")?)
    };
    require(
        !["/tmp", "/dev/shm", "/run"]
            .iter()
            .any(|p| resolved.starts_with(p)),
        "measurement output must use persistent storage, outside temporary run directories",
    )?;
    Ok(Some(Settings {
        baseline,
        out: resolved,
        mode,
    }))
}

// Each CLI owns a new process group and file-backed streams, so a full pipe or descendant
// cannot defeat the timeout. The guard kills/reaps on every error and unwinding path.
struct Running {
    child: Option<Child>,
    stdout: PathBuf,
    stderr: PathBuf,
    start: Instant,
}
impl Running {
    fn start(mut command: Command, stdout: PathBuf, stderr: PathBuf) -> Result<Self> {
        command
            .stdin(Stdio::null())
            .stdout(fs::File::create(&stdout).map_err(failure)?)
            .stderr(fs::File::create(&stderr).map_err(failure)?);
        unsafe {
            command.pre_exec(|| {
                if libc::setpgid(0, 0) != 0 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
        let start = Instant::now();
        Ok(Self {
            child: Some(command.spawn().map_err(failure)?),
            stdout,
            stderr,
            start,
        })
    }
    fn finish(mut self, timeout: Duration) -> Result<(Value, u128)> {
        loop {
            if let Some(status) = self.child.as_mut().unwrap().try_wait().map_err(failure)? {
                let elapsed = self.start.elapsed().as_nanos();
                require(
                    status.success(),
                    &format!(
                        "CLI failed: {}",
                        fs::read_to_string(&self.stderr).unwrap_or_default()
                    ),
                )?;
                // Reap and terminate any descendant left by a hook before releasing ownership.
                self.stop();
                let value = serde_json::from_slice(&fs::read(&self.stdout).map_err(failure)?)
                    .map_err(failure)?;
                return Ok((value, elapsed));
            }
            require(
                self.start.elapsed() < timeout,
                "CLI timed out; sample incomplete",
            )?;
            std::thread::sleep(Duration::from_millis(2));
        }
    }
    fn stop(&mut self) {
        if let Some(mut child) = self.child.take() {
            unsafe {
                libc::kill(-(child.id() as i32), libc::SIGKILL);
            }
            let _ = child.wait();
        }
    }
}
impl Drop for Running {
    fn drop(&mut self) {
        self.stop();
    }
}

struct Fixture {
    root: PathBuf,
    home: PathBuf,
    code: PathBuf,
    records: BTreeMap<String, PathBuf>,
    ids: Vec<String>,
    binary: PathBuf,
    logs: PathBuf,
    sequence: usize,
}
impl Fixture {
    fn command(&self, args: &[&str]) -> Command {
        let mut command = Command::new(&self.binary);
        command
            .env_clear()
            .envs(common::isolated_env(&self.home))
            .env("TASKS_SESSION", "retained-measure-fixture")
            .env("GIT_AUTHOR_DATE", "2001-01-01T00:00:00Z")
            .env("GIT_COMMITTER_DATE", "2001-01-01T00:00:00Z")
            .current_dir(&self.code)
            .args(args);
        command
    }
    fn begin(
        &mut self,
        args: &[&str],
        observed: Option<&Path>,
        trace: bool,
    ) -> Result<(Running, PathBuf)> {
        self.sequence += 1;
        let stem = self.logs.join(format!("command-{:05}", self.sequence));
        let trace_path = stem.with_extension("trace2.jsonl");
        let mut command = self.command(args);
        if trace {
            command.env("GIT_TRACE2_EVENT", &trace_path);
        }
        if let Some(socket) = observed {
            command.env("TASKS_RETAINED_TEST_SOCKET", socket);
        }
        Ok((
            Running::start(
                command,
                stem.with_extension("stdout.json"),
                stem.with_extension("stderr.txt"),
            )?,
            trace_path,
        ))
    }
    fn cli(&mut self, args: &[&str]) -> Result<Value> {
        self.begin(args, None, false)?
            .0
            .finish(Duration::from_secs(60))
            .map(|x| x.0)
    }
    fn git(&self, dir: &Path, args: &[&str]) -> Result<Vec<u8>> {
        let out = common::git_fixture::command(&self.home, dir)
            .env("GIT_AUTHOR_DATE", "2001-01-01T00:00:00Z")
            .env("GIT_COMMITTER_DATE", "2001-01-01T00:00:00Z")
            .args(args)
            .output()
            .map_err(failure)?;
        require(
            out.status.success(),
            &format!("git {args:?}: {}", String::from_utf8_lossy(&out.stderr)),
        )?;
        Ok(out.stdout)
    }
    fn repository(&self, path: &Path) -> Result<()> {
        fs::create_dir_all(path).map_err(failure)?;
        self.git(path, &["init", "-qb", "main"])?;
        common::git_fixture::assert_repo(&self.home, path);
        fs::write(path.join("README"), "measurement fixture\n").map_err(failure)?;
        self.git(path, &["add", "."])?;
        self.git(path, &["commit", "-qm", "seed"])?;
        Ok(())
    }
    fn configure(&self, records: &Path) -> Result<()> {
        let path = records.join("tasks/.config.toml");
        let mut value: toml::Value =
            toml::from_str(&fs::read_to_string(&path).map_err(failure)?).map_err(failure)?;
        value["git"] = toml::Value::try_from(json!({"commit":true})).map_err(failure)?;
        fs::write(path, toml::to_string(&value).map_err(failure)?).map_err(failure)?;
        self.git(records, &["add", "."])?;
        self.git(records, &["commit", "-qm", "configure records"])?;
        Ok(())
    }
    fn seed(root: &Path, logs: &Path, binary: &Path, shared: bool, history: usize) -> Result<Self> {
        fs::create_dir_all(logs).map_err(failure)?;
        let mut f = Self {
            root: root.into(),
            home: root.join("home"),
            code: root.join("code"),
            records: BTreeMap::new(),
            ids: Vec::new(),
            binary: binary.into(),
            logs: logs.into(),
            sequence: 0,
        };
        fs::create_dir_all(&f.home).map_err(failure)?;
        f.repository(&f.code)?;
        let records = root.join("records");
        f.repository(&records)?;
        f.cli(&[
            "init",
            "--prefix",
            "sci",
            "--records",
            records.to_str().unwrap(),
        ])?;
        f.records.insert("sci".into(), records.join("sci"));
        f.configure(&records.join("sci"))?;
        let peer = root.join("peer");
        f.repository(&peer)?;
        if shared {
            f.cli(&[
                "-C",
                peer.to_str().unwrap(),
                "init",
                "--prefix",
                "fam",
                "--records",
                records.to_str().unwrap(),
            ])?;
            f.records.insert("fam".into(), records.join("fam"));
            f.configure(&records.join("fam"))?;
        } else {
            let peer_code = root.join("peer-code");
            f.repository(&peer_code)?;
            f.cli(&[
                "-C",
                peer.to_str().unwrap(),
                "init",
                "--prefix",
                "fam",
                "--checkout",
                peer_code.to_str().unwrap(),
            ])?;
            f.records.insert("fam".into(), peer.clone());
            f.configure(&peer)?;
        }
        let aux = root.join("aux");
        f.repository(&aux)?;
        f.cli(&[
            "-C",
            aux.to_str().unwrap(),
            "init",
            "--prefix",
            "aux",
            "--records",
            records.to_str().unwrap(),
        ])?;
        f.records.insert("aux".into(), records.join("aux"));
        f.configure(&records.join("aux"))?;
        for prefix in ["sci", "fam", "aux"] {
            let id = f.cli(&["add", "Measurement task", "--project", prefix])?["id"]
                .as_str()
                .ok_or("task id")?
                .to_owned();
            f.cli(&["start", &id])?;
            f.ids.push(id);
        }
        // Future task timestamps hold generated phase dates stable; attachments still use wall time.
        for (prefix, id) in ["sci", "fam", "aux"].iter().zip(&f.ids) {
            let path = f.records[*prefix].join(format!("tasks/{id}.md"));
            let raw = fs::read_to_string(&path).map_err(failure)?;
            let raw = raw
                .lines()
                .map(|line| {
                    if line.starts_with("updated: ") {
                        "updated: 2099-01-01T00:00:00Z"
                    } else {
                        line
                    }
                })
                .collect::<Vec<_>>()
                .join("\n")
                + "\n";
            fs::write(path, raw).map_err(failure)?;
            f.git(&f.records[*prefix], &["add", "tasks"])?;
            f.git(
                &f.records[*prefix],
                &["commit", "-qm", "stable future record clock"],
            )?;
        }
        for n in 0..history {
            for index in 0..2 {
                fs::write(
                    root.join(format!("payload-{index}")),
                    vec![b'a' + n as u8; 64],
                )
                .map_err(failure)?;
                f.checkpoint(index, 1 + (n * 2 + index) as u64, false, None)?;
            }
        }
        if history == 0 {
            for index in 0..2 {
                fs::write(root.join(format!("payload-{index}")), vec![b'a'; 64])
                    .map_err(failure)?;
            }
        }
        Ok(f)
    }
    fn state(&mut self, id: &str) -> Result<Value> {
        self.cli(&["retained", "state", id])
    }
    fn request(&self, name: &str, value: &Value) -> Result<PathBuf> {
        let path = self.root.join(name);
        fs::write(&path, serde_json::to_vec(value).map_err(failure)?).map_err(failure)?;
        Ok(path)
    }
    fn checkpoint(
        &mut self,
        index: usize,
        op: u64,
        measured: bool,
        observer: Option<(&Path, usize, usize)>,
    ) -> Result<Sample> {
        let id = self.ids[index].clone();
        let basis = self.state(&id)?["basis"].clone();
        let request = json!({"version":1,"operation_id":format!("{op:032x}"),"host":common::retained::host(),"inputs":[{"task":id,"basis":basis}],"declarations":[common::retained::declaration(&self.root.join(format!("payload-{index}")),"evidence")]});
        let path = self.request(&format!("checkpoint-{op}.json"), &request)?;
        self.run(
            &[
                "retained",
                "checkpoint",
                &id,
                "--request",
                path.to_str().unwrap(),
            ],
            measured,
            observer,
        )
    }
    fn transfer(&mut self, observer: Option<(&Path, usize, usize)>) -> Result<Sample> {
        let from = self.ids[0].clone();
        let to = self.ids[1].clone();
        let sender = self.state(&from)?;
        let receiver = self.state(&to)?;
        let request = json!({"version":1,"operation_id":format!("{:032x}",1000),"host":common::retained::host(),"inputs":[{"task":from,"basis":sender["basis"]},{"task":to,"basis":receiver["basis"]}],"items":[sender["items"][0]["identity"]],"purpose":"Continue review","cleanup_trigger":"Review and disposition complete"});
        let path = self.request("transfer.json", &request)?;
        self.run(
            &[
                "retained",
                "transfer",
                &from,
                &to,
                "--request",
                path.to_str().unwrap(),
            ],
            true,
            observer,
        )
    }
    fn run(
        &mut self,
        args: &[&str],
        trace: bool,
        observer: Option<(&Path, usize, usize)>,
    ) -> Result<Sample> {
        let listener = if let Some((socket, _, _)) = observer {
            let listener = UnixListener::bind(socket).map_err(failure)?;
            listener.set_nonblocking(true).map_err(failure)?;
            Some(listener)
        } else {
            None
        };
        let (mut child, trace_path) = self.begin(args, observer.map(|x| x.0), trace)?;
        let mut frames = Vec::new();
        let mut inspections = false;
        let mut prechecks = false;
        if let Some(listener) = listener {
            loop {
                let exited = child
                    .child
                    .as_mut()
                    .unwrap()
                    .try_wait()
                    .map_err(failure)?
                    .is_some();
                loop {
                    let (mut stream, _) = match listener.accept() {
                        Ok(pair) => pair,
                        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                        Err(e) => return Err(failure(e)),
                    };
                    stream
                        .set_read_timeout(Some(Duration::from_secs(2)))
                        .map_err(failure)?;
                    let mut bytes = Vec::new();
                    loop {
                        let mut b = [0];
                        stream.read_exact(&mut b).map_err(failure)?;
                        if b[0] == b'\n' {
                            break;
                        }
                        bytes.push(b[0]);
                        require(bytes.len() < 4096, "observer frame bound")?;
                    }
                    let value: Value = serde_json::from_slice(&bytes).map_err(failure)?;
                    if value["event"] == "inspection" && !inspections {
                        inspections = true;
                        self.unrelated(observer.unwrap().1)?;
                    }
                    if value["event"] == "prechecked" && !prechecks {
                        prechecks = true;
                        self.unrelated(observer.unwrap().2)?;
                    }
                    if value["barrier"] == true {
                        stream.write_all(b"+").map_err(failure)?;
                    }
                    frames.push(value);
                }
                if exited {
                    break;
                }
                require(
                    child.start.elapsed() < Duration::from_secs(60),
                    "observer CLI timed out; sample incomplete",
                )?;
                std::thread::sleep(Duration::from_millis(1));
            }
            fs::remove_file(observer.unwrap().0).map_err(failure)?;
        }
        let (result, wall_ns) = child.finish(Duration::from_secs(60))?;
        let (git_starts, repeated_immutable) = if trace {
            trace_counts(&trace_path)?
        } else {
            (0, 0)
        };
        let timing = timing(&frames)?;
        Ok(Sample {
            result,
            wall_ns,
            git_starts,
            repeated_immutable,
            frames,
            timing,
        })
    }
    fn unrelated(&mut self, count: usize) -> Result<()> {
        let id = self.ids[2].clone();
        for n in 0..count {
            self.cli(&[
                "note",
                &id,
                &format!("Unrelated observation {}-{n}", self.sequence),
            ])?;
        }
        Ok(())
    }
}

#[derive(Serialize)]
struct Sample {
    result: Value,
    wall_ns: u128,
    git_starts: usize,
    repeated_immutable: usize,
    frames: Vec<Value>,
    timing: Timing,
}
#[derive(Default, Serialize)]
struct Timing {
    holds: Vec<Value>,
    guards_ns: Vec<u64>,
    waits: Vec<Value>,
    guarded_git_starts: usize,
    repeated_old_edge_starts: usize,
    cache_saturated: usize,
    query_admissions: usize,
}
impl Timing {
    fn require_admitted_reuse(&self) -> Result<()> {
        require(
            self.repeated_old_edge_starts == 0,
            "admitted old-edge reads repeated guarded Git starts",
        )
    }
}
fn timing(frames: &[Value]) -> Result<Timing> {
    let mut out = Timing::default();
    let mut waits = BTreeMap::new();
    let mut locks = BTreeMap::new();
    let mut guard = None;
    let mut admitted = BTreeSet::new();
    for frame in frames {
        let name = frame["event"].as_str().ok_or("frame event")?;
        let phase = frame["phase"].as_str().ok_or("frame phase")?;
        let at = frame["elapsed_ns"].as_u64().ok_or("frame timestamp")?;
        require(
            frame["barrier"].as_bool()
                == Some(matches!(
                    name,
                    "inspection" | "prechecked" | "before_write" | "readback"
                )),
            "frame barrier contract",
        )?;
        match name {
            "lock_wait" => {
                waits.insert(phase.to_string(), at);
            }
            "lock_acquired" => {
                let start = waits.remove(phase).ok_or("lock acquired without wait")?;
                out.waits.push(json!({"common":phase,"ns":at-start}));
                require(
                    locks.insert(phase.to_string(), at).is_none(),
                    "overlapping lock intervals",
                )?;
            }
            "lock_released" => {
                let start = locks.remove(phase).ok_or("release without acquisition")?;
                out.holds.push(json!({"common":phase,"ns":at-start}));
            }
            "guard_acquired" => {
                require(guard.replace(at).is_none(), "overlapping guard")?;
            }
            "guard_released" => {
                out.guards_ns
                    .push(at - guard.take().ok_or("guard release without acquisition")?);
            }
            "git_start" => {
                let command: Value = serde_json::from_str(phase).map_err(failure)?;
                if guard.is_some() {
                    out.guarded_git_starts += 1;
                    if command["source"] == "immutable_read" && admitted.contains(phase) {
                        out.repeated_old_edge_starts += 1;
                    }
                }
            }
            "query_admitted" => {
                let command: Value = serde_json::from_str(phase).map_err(failure)?;
                require(
                    command["source"] == "immutable_read",
                    "query admission must name an immutable query",
                )?;
                // The producer's shared cache admits at most 2,048 entries across all kinds.
                require(
                    admitted.len() < 2048 || admitted.contains(phase),
                    "query admission observations exceed cache bound",
                )?;
                admitted.insert(phase.to_string());
                out.query_admissions += 1;
            }
            "cache_saturated" => out.cache_saturated += 1,
            "inspection" | "prechecked" | "before_write" | "readback" | "commit_wait"
            | "prefix_wait" => {}
            _ => return Err(format!("unknown observation {name}")),
        }
    }
    require(
        locks.is_empty() && guard.is_none() && waits.is_empty(),
        "incomplete lock timing intervals",
    )?;
    Ok(out)
}
fn trace_counts(path: &Path) -> Result<(usize, usize)> {
    let events = fs::read_to_string(path)
        .map_err(failure)?
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).map_err(failure))
        .collect::<Result<Vec<_>>>()?;
    let mut repositories = BTreeMap::new();
    for event in &events {
        if event["event"] == "def_repo" {
            repositories.insert(
                event["sid"].as_str().ok_or("trace session")?.to_owned(),
                event["worktree"]
                    .as_str()
                    .ok_or("trace worktree")?
                    .to_owned(),
            );
        }
    }
    let mut starts = 0;
    let mut seen = BTreeSet::new();
    let mut repeats = 0;
    for event in events.iter().filter(|e| e["event"] == "start") {
        starts += 1;
        let args = event["argv"].as_array().ok_or("trace argv")?;
        let immutable = args.iter().any(|x| {
            matches!(
                x.as_str(),
                Some(
                    "show"
                        | "cat-file"
                        | "ls-tree"
                        | "rev-list"
                        | "merge-base"
                        | "diff"
                        | "log"
                        | "rev-parse"
                )
            )
        }) && args.iter().filter_map(Value::as_str).any(|arg| {
            arg.split(|c: char| !c.is_ascii_hexdigit())
                .any(|word| word.len() == 40)
        });
        if immutable {
            let sid = event["sid"].as_str().ok_or("trace session")?;
            let repository = repositories
                .get(sid)
                .ok_or("immutable trace start missing repository identity")?;
            let key = serde_json::to_string(&(repository, args)).map_err(failure)?;
            if !seen.insert(key) {
                repeats += 1;
            }
        }
    }
    require(starts > 0, "empty Git trace is not a completed workload")?;
    Ok((starts, repeats))
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct Counts {
    kept_verified: u64,
    kept_unknown: u64,
    released: u64,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct Identity {
    host: String,
    kind: String,
    path: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct Custody {
    version: u8,
    host: String,
    kind: String,
    path: String,
    state: String,
    verification: String,
    purpose: String,
    next_owner: String,
    cleanup_trigger: String,
    #[serde(default)]
    sha256: Option<String>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
enum ManifestEntry {
    File { path: String, sha256: String },
    Directory { path: String },
    Symlink { path: String, target: String },
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
struct ItemProjection {
    identity: Identity,
    record: Custody,
    ordinal: u64,
    coverage: String,
    payload_sha256: String,
    manifest_entries: Vec<ManifestEntry>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
struct PhaseProjection {
    phase: String,
    ordinal: u64,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
struct TaskProjection {
    task: String,
    phases: Vec<PhaseProjection>,
    items: Vec<ItemProjection>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
struct Projection {
    outcome: String,
    counts: Counts,
    actions: Vec<String>,
    tasks: Vec<TaskProjection>,
}
fn notes(bytes: &[u8]) -> Result<Vec<String>> {
    let raw = std::str::from_utf8(bytes).map_err(failure)?;
    Ok(raw
        .split_once("\n## Notes\n")
        .map_or("", |x| x.1)
        .lines()
        .filter_map(|line| {
            if line.starts_with("- ") {
                line.split_once("): ").map(|x| x.1.to_string())
            } else {
                None
            }
        })
        .collect())
}
impl Fixture {
    fn blob(&self, basis: &Value, path: &str) -> Result<Vec<u8>> {
        let prefix = basis["project"].as_str().ok_or("basis project")?;
        let records = self.records.get(prefix).ok_or("unknown basis project")?;
        let commit = basis["commit"].as_str().ok_or("basis commit")?;
        require(
            commit.len() == 40 && commit.bytes().all(|c| c.is_ascii_hexdigit()),
            "invalid publication commit",
        )?;
        self.git(records, &["show", &format!("{commit}:{path}")])
    }
    fn locator(&self, locator: &Value, source: &Value) -> Result<Vec<u8>> {
        let (basis, path) = match locator["type"].as_str().ok_or("artifact type")? {
            "attachment" => {
                let task = locator["task"].as_str().ok_or("artifact task")?;
                let basis = locator.get("basis").unwrap_or(source);
                require(
                    task.split('-').next() == basis["project"].as_str(),
                    "artifact basis project mismatch",
                )?;
                let subdir = basis["records_subdir"].as_str().ok_or("records subdir")?;
                let relative = format!(
                    "tasks/files/{task}/{}",
                    locator["name"].as_str().ok_or("artifact name")?
                );
                (
                    basis,
                    if subdir == "." {
                        relative
                    } else {
                        format!("{subdir}/{relative}")
                    },
                )
            }
            "git" => (
                locator.get("basis").ok_or("artifact basis")?,
                locator["path"].as_str().ok_or("artifact path")?.into(),
            ),
            _ => return Err("fixture expected a file artifact locator".into()),
        };
        let bytes = self.blob(basis, &path)?;
        require(
            Some(digest(&bytes).as_str()) == locator["sha256"].as_str(),
            "artifact digest mismatch",
        )?;
        Ok(bytes)
    }
    fn publication(&self, publication: &Value, operation: &Value) -> Result<()> {
        let basis = &publication["basis"];
        let id = publication["task"].as_str().ok_or("publication task")?;
        require(
            id.split('-').next() == basis["project"].as_str(),
            "publication task basis mismatch",
        )?;
        let path = basis["task_path"].as_str().ok_or("publication task path")?;
        let expected_phase = publication["phase"].as_str().ok_or("publication phase")?;
        let target = self.blob(basis, path)?;
        let records = &self.records[id.split('-').next().unwrap()];
        let current = self.git(records, &["show", &format!("HEAD:{path}")])?;
        let expected = notes(&current)?
            .into_iter()
            .filter_map(|n| n.strip_prefix("retention operation: ").map(str::to_owned))
            .map(|n| serde_json::from_str::<Value>(&n).map_err(failure))
            .collect::<Result<Vec<_>>>()?
            .into_iter()
            .find(|n| n["key"] == *operation && n["phase"] == expected_phase)
            .ok_or("publication phase missing from final record")?;
        let find = |bytes: &[u8]| -> Result<bool> {
            for note in notes(bytes)? {
                if let Some(text) = note.strip_prefix("retention operation: ") {
                    let value: Value = serde_json::from_str(text).map_err(failure)?;
                    if value == expected {
                        return Ok(true);
                    }
                }
            }
            Ok(false)
        };
        require(find(&target)?, "publication basis lacks named phase")?;
        let records = &self.records[id.split('-').next().unwrap()];
        let commits = self.git(
            records,
            &[
                "rev-list",
                "--reverse",
                "--first-parent",
                basis["commit"].as_str().unwrap(),
                "--",
                &format!(":(top,literal){path}"),
            ],
        )?;
        let mut first = None;
        for commit in std::str::from_utf8(&commits).map_err(failure)?.lines() {
            let raw = self.git(records, &["show", &format!("{commit}:{path}")])?;
            if find(&raw)? {
                first = Some(commit.to_string());
                break;
            }
        }
        require(
            first.as_deref() == basis["commit"].as_str(),
            &format!(
                "basis is not actual first phase publication: {id} {expected_phase} expected {:?}, first {first:?}",
                basis["commit"]
            ),
        )
    }
    // Run the shipped proof reader independently, then verify exact source/artifact bytes
    // and every original publication before building a comparison that excludes generated IDs.
    fn validate(&mut self, result: &Value) -> Result<Projection> {
        require(result["warnings"] == json!([]), "mutation warnings")?;
        require(
            matches!(result["outcome"].as_str(), Some("verified" | "transferred")),
            "incomplete mutation",
        )?;
        for publication in result["publications"]
            .as_array()
            .ok_or("result publications")?
        {
            self.publication(publication, &result["operation"])?;
        }
        let mut tasks = Vec::new();
        for id in self.ids[..2].to_vec() {
            let state = self.state(&id)?;
            require(
                state["complete"] == true
                    && state["warnings"] == json!([])
                    && state["diagnostics"] == json!([]),
                "state proof incomplete",
            )?;
            let source_basis = &state["basis"];
            let task_bytes = self.blob(
                source_basis,
                source_basis["task_path"].as_str().ok_or("task path")?,
            )?;
            let logical_notes = notes(&task_bytes)?;
            let mut phases = Vec::new();
            for entry in state["operations"].as_array().ok_or("operations")? {
                require(
                    entry["source"]["basis"] == state["basis"],
                    "phase source basis mismatch",
                )?;
                let ordinal = entry["source"]["ordinal"]
                    .as_u64()
                    .ok_or("source ordinal")?;
                let text = logical_notes
                    .get(ordinal as usize - 1)
                    .ok_or("source ordinal outside record")?;
                let raw: Value = serde_json::from_str(
                    text.strip_prefix("retention operation: ")
                        .ok_or("source note mismatch")?,
                )
                .map_err(failure)?;
                require(raw == entry["note"], "operation source readback mismatch")?;
                let note = &entry["note"];
                let timestamp = note["at"].as_str().ok_or("phase timestamp")?;
                time::OffsetDateTime::parse(
                    timestamp,
                    &time::format_description::well_known::Rfc3339,
                )
                .map_err(failure)?;
                let request_bytes = self.locator(&note["request"], source_basis)?;
                let request: Value = serde_json::from_slice(&request_bytes).map_err(failure)?;
                require(
                    request["operation_id"] == note["key"]["operation_id"]
                        && request["inputs"] == note["input_bases"]
                        && request["host"] == note["host"],
                    "request phase linkage mismatch",
                )?;
                if let Some(artifacts) = note["outputs"].get("artifacts").and_then(Value::as_array)
                {
                    for artifact in artifacts {
                        self.locator(artifact, source_basis)?;
                    }
                }
                if let Some(publications) = note["outputs"]
                    .get("publications")
                    .and_then(Value::as_array)
                {
                    for publication in publications {
                        self.publication(publication, &note["key"])?;
                    }
                }
                phases.push(PhaseProjection {
                    phase: note["phase"].as_str().ok_or("phase")?.into(),
                    ordinal,
                });
            }
            let mut items = Vec::new();
            for item in state["items"].as_array().ok_or("items")? {
                require(
                    item["source"]["basis"] == state["basis"],
                    "item source basis mismatch",
                )?;
                let ordinal = item["source"]["ordinal"].as_u64().ok_or("item ordinal")?;
                let text = logical_notes
                    .get(ordinal as usize - 1)
                    .ok_or("item source ordinal")?;
                let raw: Value = serde_json::from_str(
                    text.strip_prefix("retained: ")
                        .ok_or("retained source mismatch")?,
                )
                .map_err(failure)?;
                require(
                    raw == item["record"],
                    "retained item source readback mismatch",
                )?;
                let record: Custody = serde_json::from_value(raw.clone()).map_err(failure)?;
                let identity: Identity =
                    serde_json::from_value(item["identity"].clone()).map_err(failure)?;
                let bytes = fs::read(&record.path).map_err(failure)?;
                let payload_sha256 = digest(&bytes);
                if record.state == "kept" {
                    require(
                        record.verification == "verified"
                            && record.sha256.as_deref() == Some(&payload_sha256),
                        "current payload digest/custody mismatch",
                    )?;
                }
                let mut manifest_entries = Vec::new();
                if let Some(manifest) = raw.get("manifest") {
                    let manifest: Value =
                        serde_json::from_slice(&self.locator(manifest, source_basis)?)
                            .map_err(failure)?;
                    manifest_entries =
                        serde_json::from_value(manifest["entries"].clone()).map_err(failure)?;
                }
                items.push(ItemProjection {
                    identity,
                    record,
                    ordinal,
                    coverage: item["coverage"].as_str().ok_or("coverage")?.into(),
                    payload_sha256,
                    manifest_entries,
                });
            }
            tasks.push(TaskProjection {
                task: id,
                phases,
                items,
            });
        }
        if result["outcome"] == "transferred" {
            require(
                tasks[0].items.iter().any(|i| i.record.state == "released"),
                "sender custody not released",
            )?;
            require(
                tasks[1].items.iter().any(|i| {
                    i.record.state == "kept"
                        && i.record.next_owner == format!("task:{}", self.ids[1])
                        && i.record.purpose == "Continue review"
                }),
                "receiver custody missing",
            )?;
        }
        Ok(Projection {
            outcome: result["outcome"].as_str().ok_or("outcome")?.into(),
            counts: serde_json::from_value(result["counts"].clone()).map_err(failure)?,
            actions: serde_json::from_value(result["actions"].clone()).map_err(failure)?,
            tasks,
        })
    }
}
fn copy_tree(from: &Path, to: &Path) -> Result<()> {
    fs::create_dir_all(to).map_err(failure)?;
    for entry in fs::read_dir(from).map_err(failure)? {
        let entry = entry.map_err(failure)?;
        let target = to.join(entry.file_name());
        let kind = entry.file_type().map_err(failure)?;
        if kind.is_dir() {
            copy_tree(&entry.path(), &target)?;
        } else if kind.is_file() {
            fs::copy(entry.path(), target).map_err(failure)?;
        } else {
            return Err("unexpected fixture symlink/special file".into());
        }
    }
    Ok(())
}
fn restore(seed: &Path, f: &Fixture) -> Result<()> {
    // Validate both roots before any destructive fixture reset.
    for records in f.records.values() {
        let top = f.git(records, &["rev-parse", "--show-toplevel"])?;
        let top = PathBuf::from(std::str::from_utf8(&top).map_err(failure)?.trim());
        common::git_fixture::assert_repo(&f.home, &top);
        require(top.starts_with(&f.root), "fixture root escaped")?;
    }
    fs::remove_dir_all(&f.root).map_err(failure)?;
    copy_tree(seed, &f.root)
}
#[derive(Serialize)]
struct Pair {
    layout: String,
    repetition: usize,
    baseline: Vec<Sample>,
    candidate: Vec<Sample>,
    projection_equal: bool,
}
fn pair(
    f: &mut Fixture,
    seed: &Path,
    candidate: &Path,
    baseline: &Path,
    layout: &str,
    repetition: usize,
) -> Result<Pair> {
    let mut runs = Vec::new();
    let mut projections = Vec::new();
    for binary in [baseline, candidate] {
        restore(seed, f)?;
        f.binary = binary.into();
        let mut samples = Vec::new();
        let mut projected = Vec::new();
        for index in 0..2 {
            let sample = f.checkpoint(index, 100 + index as u64, true, None)?;
            projected.push(f.validate(&sample.result)?);
            samples.push(sample);
        }
        let sample = f.transfer(None)?;
        projected.push(f.validate(&sample.result)?);
        samples.push(sample);
        let role = if runs.is_empty() {
            "baseline"
        } else {
            "candidate"
        };
        let saved = f
            .logs
            .join(format!("{layout}-pair-{repetition}-{role}-fixture"));
        copy_tree(&f.root, &saved)?;
        runs.push(samples);
        projections.push(projected);
    }
    require(
        projections[0] == projections[1],
        "logical projection differs",
    )?;
    fs::write(
        f.logs
            .join(format!("{layout}-pair-{repetition}-projection.json")),
        serde_json::to_vec_pretty(&projections).map_err(failure)?,
    )
    .map_err(failure)?;
    Ok(Pair {
        layout: layout.into(),
        repetition,
        baseline: runs.remove(0),
        candidate: runs.remove(0),
        projection_equal: true,
    })
}
fn smallest(settings: &Settings) -> Result<Value> {
    fs::create_dir_all(&settings.out).map_err(failure)?;
    let candidate = assert_cmd::cargo::cargo_bin("tasks");
    let (mut f, seed, metadata) = prepare_seed(settings, true, 0)?;
    let logs = f.logs.clone();
    let pair = pair(&mut f, &seed, &candidate, &candidate, "shared", 0)?;
    let projection = f.validate(&pair.candidate.last().unwrap().result)?;
    require(
        pair.baseline.last().unwrap().result["publications"]
            != pair.candidate.last().unwrap().result["publications"],
        "smallest must exercise distinct generated publication IDs",
    )?;
    let mut bad = projection.clone();
    bad.tasks[1].items[0].payload_sha256 = "bad".into();
    require(bad != projection, "payload projection missed mutation")?;
    let mut bad = projection.clone();
    bad.tasks[1].items[0].record.purpose = "wrong custody".into();
    require(bad != projection, "custody projection missed mutation")?;
    let mut bad = projection.clone();
    bad.tasks[0].phases[0].ordinal += 1;
    require(bad != projection, "ordinal projection missed mutation")?;
    let mut corrupted = pair.candidate.last().unwrap().result.clone();
    corrupted["publications"][0]["basis"]["commit"] = json!("0".repeat(40));
    require(
        f.validate(&corrupted).is_err(),
        "corrupt publication binding passed validation",
    )?;
    let state = f.state(&f.ids[0].clone())?;
    let mut locator = state["operations"][0]["note"]["request"].clone();
    locator["sha256"] = json!("0".repeat(64));
    require(
        f.locator(&locator, &state["basis"]).is_err(),
        "corrupt artifact reference passed validation",
    )?;
    restore(&seed, &f)?;
    let socket_dir = tempfile::tempdir().map_err(failure)?;
    let socket = socket_dir.path().join("matrix.sock");
    let checkpoint = f.checkpoint(0, 100, true, Some((&socket, 1, 1)))?;
    f.validate(&checkpoint.result)?;
    require(
        !checkpoint.timing.holds.is_empty() && !checkpoint.timing.guards_ns.is_empty(),
        "missing instrumented hold intervals",
    )?;
    checkpoint.timing.require_admitted_reuse()?;
    let transfer = f.transfer(Some((&socket, 1, 1)))?;
    f.validate(&transfer.result)?;
    require(
        !transfer.timing.holds.is_empty(),
        "transfer hold protocol failed",
    )?;
    transfer.timing.require_admitted_reuse()?;
    fs::write(
        logs.join("smallest-matrix.json"),
        serde_json::to_vec_pretty(&json!([checkpoint, transfer])).map_err(failure)?,
    )
    .map_err(failure)?;
    // A timed-out descendant is killed with its parent and cannot keep captured streams open.
    let mut command = Command::new("sh");
    command
        .env_clear()
        .envs(common::isolated_env(&f.home))
        .args(["-c", "sleep 60 & wait"]);
    let running = Running::start(
        command,
        logs.join("timeout.stdout"),
        logs.join("timeout.stderr"),
    )?;
    let pid = running.child.as_ref().unwrap().id();
    require(
        running.finish(Duration::from_millis(20)).is_err(),
        "timeout did not refuse",
    )?;
    require(
        unsafe { libc::waitpid(pid as i32, std::ptr::null_mut(), libc::WNOHANG) } == -1,
        "timed-out child not reaped",
    )?;
    let repeat = Settings {
        baseline: settings.baseline.clone(),
        out: settings.out.clone(),
        mode: Mode::Repeat,
    };
    let (reused, reused_seed, reused_metadata) = prepare_seed(&repeat, true, 0)?;
    require(
        reused_seed == seed && reused.ids == f.ids && reused_metadata == metadata,
        "canonical seed changed across modes",
    )?;
    let raw_seed_file = seed.join("code/README");
    fs::write(&raw_seed_file, "corrupted seed").map_err(failure)?;
    let matrix = Settings {
        mode: Mode::Matrix,
        ..repeat
    };
    require(
        prepare_seed(&matrix, true, 0).is_err(),
        "corrupted canonical seed was reused",
    )?;
    let value = json!({"requested":true,"mode":"smallest","complete":true,"pairs":[pair],"verdict":"equal"});
    fs::write(
        settings.out.join("smallest-result.json"),
        serde_json::to_vec_pretty(&value).map_err(failure)?,
    )
    .map_err(failure)?;
    Ok(value)
}
fn stats(mut values: Vec<f64>) -> Value {
    values.sort_by(f64::total_cmp);
    let mean = values.iter().sum::<f64>() / values.len() as f64;
    json!({"samples":values,"median":values[values.len()/2],"variance":values.iter().map(|x|(x-mean).powi(2)).sum::<f64>()/values.len() as f64})
}
fn summary(samples: &[&Sample]) -> Value {
    json!({"wall_ns":stats(samples.iter().map(|x|x.wall_ns as f64).collect()),"git_starts":stats(samples.iter().map(|x|x.git_starts as f64).collect()),"repeated_immutable":stats(samples.iter().map(|x|x.repeated_immutable as f64).collect())})
}
fn matrix_summaries(cells: &[Value]) -> Value {
    let mut groups: BTreeMap<String, Vec<&Value>> = BTreeMap::new();
    for cell in cells.iter().filter(|c| c["pilot"] != true) {
        let key = format!(
            "{} inspection={} after={} {}",
            cell["layout"].as_str().unwrap(),
            cell["during_inspection"],
            cell["after_precheck"],
            cell["operation"].as_str().unwrap()
        );
        groups.entry(key).or_default().push(cell);
    }
    let mut summaries = BTreeMap::new();
    for (key, cells) in groups {
        let sum = |name: &str| {
            stats(
                cells
                    .iter()
                    .map(|c| {
                        c["sample"]["timing"][name]
                            .as_array()
                            .unwrap()
                            .iter()
                            .map(|v| v.as_u64().unwrap_or_else(|| v["ns"].as_u64().unwrap()) as f64)
                            .sum()
                    })
                    .collect(),
            )
        };
        summaries.insert(key,json!({"hold_total_ns":sum("holds"),"guard_total_ns":sum("guards_ns"),"acquisition_wait_total_ns":sum("waits"),"guarded_git_starts":stats(cells.iter().map(|c|c["sample"]["timing"]["guarded_git_starts"].as_u64().unwrap() as f64).collect()),"cache_saturated":stats(cells.iter().map(|c|c["sample"]["timing"]["cache_saturated"].as_u64().unwrap() as f64).collect())}));
    }
    json!(summaries)
}
fn progress(
    artifact: &Path,
    settings: &Settings,
    seeds: &[Value],
    pairs: &[Pair],
    cells: &[Value],
) -> Result<()> {
    fs::write(artifact,serde_json::to_vec_pretty(&json!({"requested":true,"complete":false,"mode":settings.mode,"seeds":seeds,"pairs":pairs,"matrix":cells})).map_err(failure)?).map_err(failure)
}
fn measure(settings: &Settings) -> Result<()> {
    fs::create_dir_all(&settings.out).map_err(failure)?;
    let marker = settings.out.join("measurement-protocol.json");
    if marker.exists() {
        require(
            fs::read(&marker).map_err(failure)? == b"{\"version\":1}\n",
            "unrecognized measurement directory",
        )?;
    } else {
        require(
            fs::read_dir(&settings.out)
                .map_err(failure)?
                .next()
                .is_none(),
            "measurement output must be empty or contain the canonical measurement protocol",
        )?;
        fs::write(&marker, b"{\"version\":1}\n").map_err(failure)?;
    }
    if settings.mode != Mode::Pilot {
        let pilot: Value = serde_json::from_slice(
            &fs::read(settings.out.join("measurement-pilot-result.json")).map_err(failure)?,
        )
        .map_err(failure)?;
        require(
            pilot["requested"] == true
                && pilot["complete"] == true
                && pilot["logical_verdict"] == "equal",
            "complete equal pilot required before repeat/matrix",
        )?;
        require(
            pilot["candidate_sha256"]
                == digest(&fs::read(assert_cmd::cargo::cargo_bin("tasks")).map_err(failure)?)
                && pilot["baseline_sha256"]
                    == digest(&fs::read(&settings.baseline).map_err(failure)?),
            "pilot binary revisions changed",
        )?;
    }
    let artifact = settings
        .out
        .join(format!("measurement-{}-result.json", settings.mode.name()));
    require(
        !artifact.exists(),
        "mode artifact already exists; preserve it before an explicit new attempt",
    )?;
    progress(&artifact, settings, &[], &[], &[])?;
    let result = measure_inner(settings, &artifact);
    if let Err(error) = &result {
        let mut saved: Value =
            serde_json::from_slice(&fs::read(&artifact).map_err(failure)?).map_err(failure)?;
        saved["error"] = json!(error);
        saved["complete"] = json!(false);
        fs::write(
            &artifact,
            serde_json::to_vec_pretty(&saved).map_err(failure)?,
        )
        .map_err(failure)?;
    }
    result
}
fn prepare_seed(
    settings: &Settings,
    shared: bool,
    history: usize,
) -> Result<(Fixture, PathBuf, Value)> {
    let layout = if shared { "shared" } else { "separate" };
    let layout_root = settings.out.join(layout);
    fs::create_dir_all(&layout_root).map_err(failure)?;
    let metadata_path = layout_root.join("seed-inputs.json");
    let seed = layout_root.join("canonical-seed");
    let logs = layout_root.join(settings.mode.name()).join("logs");
    require(
        !logs.exists(),
        "this mode already has evidence; preserve it and choose a fresh output root to retry",
    )?;
    fs::create_dir_all(&logs).map_err(failure)?;
    let mut f;
    let metadata;
    if metadata_path.exists() {
        metadata = serde_json::from_slice::<Value>(&fs::read(&metadata_path).map_err(failure)?)
            .map_err(failure)?;
        require(
            metadata["baseline_sha256"] == digest(&fs::read(&settings.baseline).map_err(failure)?),
            "baseline changed since canonical seed creation",
        )?;
        require(
            metadata["manifest"] == tree_manifest(&seed)?,
            "canonical seed manifest mismatch",
        )?;
        f = Fixture {
            root: layout_root.join("active"),
            home: layout_root.join("active/home"),
            code: layout_root.join("active/code"),
            records: serde_json::from_value(metadata["records"].clone()).map_err(failure)?,
            ids: serde_json::from_value(metadata["ids"].clone()).map_err(failure)?,
            binary: settings.baseline.clone(),
            logs,
            sequence: 0,
        };
    } else {
        f = Fixture::seed(
            &layout_root.join("active"),
            &layout_root.join("seed-logs"),
            &settings.baseline,
            shared,
            history,
        )?;
        copy_tree(&f.root, &seed)?;
        let mut starting = BTreeMap::new();
        let mut commits = BTreeMap::new();
        for (prefix, records) in &f.records {
            starting.insert(
                prefix,
                String::from_utf8(f.git(records, &["rev-parse", "HEAD"])?)
                    .map_err(failure)?
                    .trim()
                    .to_owned(),
            );
            commits.insert(
                prefix,
                String::from_utf8(f.git(records, &["rev-list", "--count", "HEAD"])?)
                    .map_err(failure)?
                    .trim()
                    .to_owned(),
            );
        }
        metadata = json!({"layout":layout,"seed_operations":history*2,"seed_commit_counts":commits,"starting_commits":starting,"baseline_sha256":digest(&fs::read(&settings.baseline).map_err(failure)?),"records":f.records,"ids":f.ids,"manifest":tree_manifest(&seed)?});
        fs::write(
            &metadata_path,
            serde_json::to_vec_pretty(&metadata).map_err(failure)?,
        )
        .map_err(failure)?;
        f.logs = logs;
        f.sequence = 0;
    }

    Ok((f, seed, metadata))
}
fn git_count_verdict(mode: Mode, lower: bool) -> Option<bool> {
    (mode != Mode::Matrix).then_some(lower)
}
fn measure_inner(settings: &Settings, artifact: &Path) -> Result<()> {
    fs::create_dir_all(&settings.out).map_err(failure)?;

    let candidate = assert_cmd::cargo::cargo_bin("tasks");
    let mut pairs = Vec::new();
    let mut cells = Vec::new();
    let mut seeds = Vec::new();
    for shared in [true, false] {
        let layout = if shared { "shared" } else { "separate" };
        let (mut f, seed, metadata) = prepare_seed(settings, shared, 3)?;
        seeds.push(metadata);
        if settings.mode != Mode::Matrix {
            for repetition in 0..settings.mode.repetitions() {
                pairs.push(pair(
                    &mut f,
                    &seed,
                    &candidate,
                    &settings.baseline,
                    layout,
                    repetition,
                )?);
                progress(artifact, settings, &seeds, &pairs, &cells)?;
            }
        } else {
            restore(&seed, &f)?;
            f.binary = candidate.clone();
            let socket_dir = tempfile::tempdir().map_err(failure)?;
            let socket = socket_dir.path().join("pilot.sock");
            let checkpoint = f.checkpoint(0, 100, true, Some((&socket, 0, 0)))?;
            let mut reference = vec![f.validate(&checkpoint.result)?];
            require(
                !checkpoint.timing.holds.is_empty(),
                "matrix checkpoint pilot failed",
            )?;
            checkpoint.timing.require_admitted_reuse()?;
            cells.push(
                json!({"pilot":true,"layout":layout,"operation":"checkpoint","sample":checkpoint}),
            );
            let transfer = f.transfer(Some((&socket, 0, 0)))?;
            reference.push(f.validate(&transfer.result)?);
            require(
                !transfer.timing.holds.is_empty(),
                "matrix transfer pilot failed",
            )?;
            transfer.timing.require_admitted_reuse()?;
            cells.push(
                json!({"pilot":true,"layout":layout,"operation":"transfer","sample":transfer}),
            );
            copy_tree(&f.root, &f.logs.join("matrix-pilot-fixture"))?;
            progress(artifact, settings, &seeds, &pairs, &cells)?;
            for during in [0, 10, 100] {
                for after in [0, 1, 10] {
                    for repetition in 0..3 {
                        restore(&seed, &f)?;
                        f.binary = candidate.clone();
                        let socket_dir = tempfile::tempdir().map_err(failure)?;
                        let socket = socket_dir.path().join("observe.sock");
                        let sample = f.checkpoint(0, 100, true, Some((&socket, during, after)))?;
                        require(
                            f.validate(&sample.result)? == reference[0],
                            "matrix checkpoint projection differs from pilot",
                        )?;
                        sample.timing.require_admitted_reuse()?;
                        cells.push(json!({"layout":layout,"during_inspection":during,"after_precheck":after,"repetition":repetition,"operation":"checkpoint","sample":sample}));
                        let sample = f.transfer(Some((&socket, during, after)))?;
                        require(
                            f.validate(&sample.result)? == reference[1],
                            "matrix transfer projection differs from pilot",
                        )?;
                        sample.timing.require_admitted_reuse()?;
                        cells.push(json!({"layout":layout,"during_inspection":during,"after_precheck":after,"repetition":repetition,"operation":"transfer","sample":sample}));
                        copy_tree(
                            &f.root,
                            &f.logs
                                .join(format!("matrix-{during}-{after}-{repetition}-fixture")),
                        )?;
                        progress(artifact, settings, &seeds, &pairs, &cells)?;
                    }
                }
            }
        }
    }
    let mut summaries = Vec::new();
    let mut performance_equal_or_better = true;
    for layout in ["shared", "separate"] {
        for operation in [0, 1, 2] {
            let selected = pairs
                .iter()
                .filter(|x| x.layout == layout)
                .collect::<Vec<_>>();
            if selected.is_empty() {
                continue;
            }
            let baseline = summary(
                &selected
                    .iter()
                    .map(|p| &p.baseline[operation])
                    .collect::<Vec<_>>(),
            );
            let candidate = summary(
                &selected
                    .iter()
                    .map(|p| &p.candidate[operation])
                    .collect::<Vec<_>>(),
            );
            let lower = candidate["git_starts"]["median"].as_f64()
                < baseline["git_starts"]["median"].as_f64()
                && candidate["repeated_immutable"]["median"].as_f64()
                    < baseline["repeated_immutable"]["median"].as_f64();
            performance_equal_or_better &= lower;
            summaries.push(json!({"layout":layout,"operation":if operation==2 {"transfer"} else {"checkpoint"},"participant":operation,"baseline":baseline,"candidate":candidate,"git_counts_lower":lower}));
        }
    }
    let value = json!({"requested":true,"complete":true,"mode":settings.mode,"build":"debug opt-level=0; no release latency inference","candidate_binary":candidate,"candidate_sha256":digest(&fs::read(&candidate).map_err(failure)?),"baseline_binary":settings.baseline,"baseline_sha256":digest(&fs::read(&settings.baseline).map_err(failure)?),"seeds":seeds,"pairs":pairs,"matrix":cells,"matrix_summaries":matrix_summaries(&cells),"summaries":summaries,"logical_verdict":"equal","git_count_verdict":git_count_verdict(settings.mode,performance_equal_or_better),"instrumented_matrix":settings.mode==Mode::Matrix});
    fs::write(
        artifact,
        serde_json::to_vec_pretty(&value).map_err(failure)?,
    )
    .map_err(failure)?;
    println!(
        "retained measurement: requested=true complete=true mode={:?}",
        settings.mode
    );
    Ok(())
}
fn tree_manifest(root: &Path) -> Result<Value> {
    fn walk(root: &Path, dir: &Path, entries: &mut Vec<Value>) -> Result<()> {
        for entry in fs::read_dir(dir).map_err(failure)? {
            let entry = entry.map_err(failure)?;
            let path = entry.path();
            let relative = path
                .strip_prefix(root)
                .map_err(failure)?
                .to_str()
                .ok_or("manifest path")?
                .to_string();
            if entry.file_type().map_err(failure)?.is_dir() {
                entries.push(json!({"path":relative,"type":"directory"}));
                walk(root, &path, entries)?;
            } else if entry.file_type().map_err(failure)?.is_file() {
                entries.push(json!({"path":relative,"type":"file","sha256":digest(&fs::read(path).map_err(failure)?)}));
            } else {
                return Err("unexpected canonical fixture symlink/special file".into());
            }
        }
        Ok(())
    }
    let mut entries = Vec::new();
    walk(root, root, &mut entries)?;
    entries.sort_by(|a, b| a["path"].as_str().cmp(&b["path"].as_str()));
    Ok(json!({"version":1,"entries":entries}))
}

#[test]
fn retained_latency_admission_gate() {
    let signature=serde_json::to_string(&json!({"source":"immutable_read","repository":"/fixture/records","argv":["cat-file","blob",format!("{}:tasks/sci-000001.md","a".repeat(40))]})).unwrap();
    let frame = |name: &str, phase: &str, n: u64| json!({"event":name,"phase":phase,"elapsed_ns":n,"barrier":false});
    let uncached = vec![
        frame("git_start", &signature, 0),
        frame("cache_saturated", "entries_or_bytes", 1),
        frame("guard_acquired", "", 2),
        frame("git_start", &signature, 3),
        frame("cache_saturated", "entries_or_bytes", 4),
        frame("guard_released", "", 5),
    ];
    let permitted = timing(&uncached).unwrap();
    assert_eq!(permitted.guarded_git_starts, 1);
    assert_eq!(permitted.cache_saturated, 2);
    assert_eq!(permitted.query_admissions, 0);
    assert_eq!(
        permitted.repeated_old_edge_starts, 0,
        "cache eligibility alone cannot imply admission"
    );
    assert!(permitted.require_admitted_reuse().is_ok());
    let admitted = vec![
        frame("git_start", &signature, 0),
        frame("query_admitted", &signature, 1),
        frame("guard_acquired", "", 2),
        frame("git_start", &signature, 3),
        frame("guard_released", "", 4),
    ];
    let forbidden = timing(&admitted).unwrap();
    assert_eq!(forbidden.guarded_git_starts, 1);
    assert_eq!(forbidden.cache_saturated, 0);
    assert_eq!(forbidden.query_admissions, 1);
    assert_eq!(forbidden.repeated_old_edge_starts, 1);
    assert!(forbidden.require_admitted_reuse().is_err());
    let other = signature.replace("/fixture/records", "/fixture/peer");
    let distinct = vec![
        frame("query_admitted", &other, 0),
        frame("guard_acquired", "", 1),
        frame("git_start", &signature, 2),
        frame("guard_released", "", 3),
    ];
    assert!(timing(&distinct).unwrap().require_admitted_reuse().is_ok());
    let too_many = (0..=2048)
        .map(|index| {
            frame(
                "query_admitted",
                &signature.replace("sci-000001", &format!("sci-{index:06x}")),
                index,
            )
        })
        .collect::<Vec<_>>();
    assert!(
        timing(&too_many).is_err(),
        "admission evidence remains bounded by the shared cache cap"
    );
}
