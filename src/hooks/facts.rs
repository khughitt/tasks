use super::input::{Harness, strict_json};
use serde_json::Value;
use std::collections::BTreeSet;
use std::io::Read;
use std::os::fd::AsRawFd;
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

const TIMEOUT: Duration = Duration::from_secs(10);
const MAX_OUTPUT: usize = 4 * 1024 * 1024;
#[derive(Default, Debug)]
pub(super) struct WakeFacts {
    pub wake: Option<bool>,
    pub kinds: Option<BTreeSet<String>>,
    pub statuses: Option<BTreeSet<String>>,
}

pub(super) fn parse(text: &str, harness: Harness) -> Result<WakeFacts, String> {
    let doc = strict_json(text.as_bytes()).map_err(|_| "harness-facts printed invalid JSON")?;
    if doc["schema"].as_u64() != Some(1) {
        return Err("harness-facts printed no schema-1 view".into());
    }
    if !doc["harnesses"]
        .as_array()
        .is_some_and(|xs| xs.iter().any(|x| x == harness.name()))
    {
        return Err(format!("harness-facts does not declare {}", harness.name()));
    }
    let entry = |name: &str, kind: &str| -> Result<Option<&Value>, String> {
        let fact = &doc["facts"][name];
        let entry = &fact["harness"][harness.name()];
        if fact["type"] != kind || entry["type"] != kind {
            return Err(format!("harness-facts printed an invalid {name} entry"));
        }
        match entry["status"].as_str() {
            Some("unknown") => Ok(None),
            Some("probed") => entry
                .get("value")
                .map(Some)
                .ok_or_else(|| format!("harness-facts printed an invalid {name} value")),
            _ => Err(format!("harness-facts printed an invalid {name} entry")),
        }
    };
    let wake = entry("controller-wake", "boolean")?
        .map(|v| {
            v.as_bool()
                .ok_or("harness-facts printed an invalid controller-wake value")
        })
        .transpose()?;
    let set = |name| -> Result<Option<BTreeSet<String>>, String> {
        entry(name, "set")?
            .map(|v| {
                v.as_array()
                    .ok_or_else(|| format!("harness-facts printed an invalid {name} value"))?
                    .iter()
                    .map(|v| {
                        v.as_str()
                            .filter(|s| !s.is_empty())
                            .map(str::to_owned)
                            .ok_or_else(|| format!("harness-facts printed an invalid {name} value"))
                    })
                    .collect()
            })
            .transpose()
    };
    Ok(WakeFacts {
        wake,
        kinds: set("controller-wake-kinds")?,
        statuses: set("stop-input-in-flight-statuses")?,
    })
}

/// Keep the leader unreaped until the group is killed, pinning its process-group id.
struct OwnedReader(Option<Child>);
impl Drop for OwnedReader {
    fn drop(&mut self) {
        if let Some(child) = self.0.as_mut() {
            unsafe {
                libc::kill(-(child.id() as i32), libc::SIGKILL);
            }
            let _ = child.wait();
        }
    }
}
fn exited(pid: u32) -> Result<bool, String> {
    let mut info: libc::siginfo_t = unsafe { std::mem::zeroed() };
    let rc = unsafe {
        libc::waitid(
            libc::P_PID,
            pid,
            &mut info,
            libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
        )
    };
    if rc == -1 {
        return Err("harness-facts process status could not be read".into());
    }
    Ok(unsafe { info.si_pid() } != 0)
}
fn nonblocking(pipe: &impl AsRawFd) -> Result<(), String> {
    let fd = pipe.as_raw_fd();
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    if flags == -1 || unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } == -1 {
        return Err("harness-facts pipe could not be read".into());
    }
    Ok(())
}
fn drain(pipe: &mut impl Read, bytes: &mut Vec<u8>, keep: usize) -> Result<bool, String> {
    let mut buffer = [0; 8192];
    // A bounded batch lets a continuously writing child reach the deadline check.
    for _ in 0..16 {
        match pipe.read(&mut buffer) {
            Ok(0) => return Ok(true),
            Ok(n) => {
                let n = n.min(keep.saturating_sub(bytes.len()));
                bytes.extend_from_slice(&buffer[..n]);
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => return Ok(false),
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => return Err("harness-facts pipe could not be read".into()),
        }
    }
    Ok(false)
}
fn run(mut command: Command, timeout: Duration) -> Result<String, String> {
    let child = command
        .arg("list")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0)
        .spawn()
        .map_err(|_| "harness-facts is missing or cannot start; repair hq's installed reader")?;
    let mut child = OwnedReader(Some(child));
    let process = child.0.as_mut().expect("spawned reader");
    let mut stdout = process.stdout.take().expect("piped stdout");
    let mut stderr = process.stderr.take().expect("piped stderr");
    nonblocking(&stdout)?;
    nonblocking(&stderr)?;
    let deadline = Instant::now() + timeout;
    let mut bytes = Vec::new();
    let mut discard = Vec::new();
    loop {
        let out_done = drain(&mut stdout, &mut bytes, MAX_OUTPUT + 1)?;
        let err_done = drain(&mut stderr, &mut discard, 0)?;
        if bytes.len() > MAX_OUTPUT {
            return Err("harness-facts output exceeds the read limit".into());
        }
        if exited(process.id())? && out_done && err_done {
            break;
        }
        if Instant::now() >= deadline {
            return Err("harness-facts did not answer within its timeout".into());
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    // Kill descendants before reaping the pinned leader, including after a successful read.
    unsafe {
        libc::kill(-(process.id() as i32), libc::SIGKILL);
    }
    let status = process
        .wait()
        .map_err(|_| "harness-facts could not be reaped")?;
    // Drop must not signal a possibly reused PID after wait.
    child.0 = None;
    if !status.success() {
        return Err(format!(
            "harness-facts exited {}; repair hq's installed reader",
            status.code().map_or("by signal".into(), |c| c.to_string())
        ));
    }
    String::from_utf8(bytes).map_err(|_| "harness-facts printed invalid UTF-8".into())
}
pub(super) fn read(harness: Harness) -> Result<WakeFacts, String> {
    let home =
        std::env::var_os("HOME").ok_or("HOME is unavailable for hq's installed facts reader")?;
    parse(
        &run(
            Command::new(Path::new(&home).join(".agents/bin/harness-facts")),
            TIMEOUT,
        )?,
        harness,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::os::unix::fs::PermissionsExt;
    fn view(wake: Value, status: &str) -> Value {
        json!({"schema":1,"harnesses":["codex"],"facts":{
            "controller-wake":{"type":"boolean","harness":{"codex":{"type":"boolean","status":status,"value":wake}}},
            "controller-wake-kinds":{"type":"set","harness":{"codex":{"type":"set","status":"probed","value":["shell"]}}},
            "stop-input-in-flight-statuses":{"type":"set","harness":{"codex":{"type":"set","status":"unknown"}}}}})
    }
    #[test]
    fn facts_validation_preserves_false_and_unknown() {
        let f = parse(&view(json!(false), "probed").to_string(), Harness::Codex).unwrap();
        assert_eq!(f.wake, Some(false));
        assert!(f.statuses.is_none());
        assert!(
            parse(&view(json!(true), "unknown").to_string(), Harness::Codex)
                .unwrap()
                .wake
                .is_none()
        );
        for v in [json!("true"), json!(1), json!(null)] {
            assert!(parse(&view(v, "probed").to_string(), Harness::Codex).is_err());
        }
        assert!(parse("{}", Harness::Codex).is_err());
        assert!(parse(&view(json!(true), "probed").to_string(), Harness::Agy).is_err());
    }
    #[test]
    fn installed_reader_failures_are_bounded_and_cleaned() {
        assert_eq!(TIMEOUT, Duration::from_secs(10));
        let dir = tempfile::tempdir().unwrap();
        let script = dir.path().join("reader");
        let command = || {
            let mut c = Command::new(&script);
            c.env_clear()
                .env("PATH", "/usr/bin:/bin")
                .env("HOME", dir.path());
            c
        };
        assert!(run(command(), Duration::from_millis(100)).is_err());
        std::fs::write(&script, "#!/bin/sh\nprintf secret >&2\nexit 7\n").unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        let error = run(command(), Duration::from_millis(100)).unwrap_err();
        assert!(error.contains("7"));
        assert!(!error.contains("secret"));
        for body in ["sleep 30", "sleep 30 &\nexit 0"] {
            let pidfile = dir.path().join("pid");
            std::fs::write(
                &script,
                format!(
                    "#!/bin/sh\nprintf '%s' $$ > '{}'\n{body}\n",
                    pidfile.display()
                ),
            )
            .unwrap();
            let started = Instant::now();
            assert!(
                run(command(), Duration::from_millis(100))
                    .unwrap_err()
                    .contains("timeout")
            );
            assert!(started.elapsed() < Duration::from_secs(2));
            let pid: i32 = std::fs::read_to_string(&pidfile).unwrap().parse().unwrap();
            assert_eq!(unsafe { libc::kill(pid, 0) }, -1);
        }
    }
}
