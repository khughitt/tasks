use crate::resolve::{Doc, DocRead, UnavailableCause};
use std::io::{Read, Result as IoResult};
use std::os::unix::process::CommandExt;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

pub(crate) const MAX_STDOUT: u64 = 2 * 1024 * 1024;
pub(crate) const STRICT_TIMEOUT: Duration = Duration::from_secs(30);

pub(crate) struct FetchRequest {
    pub reference: String,
    pub command: String,
    pub rest: String,
}

#[cfg(test)]
enum CleanupEvent {
    BeforeGroupKill(libc::pid_t),
    AfterGroupKill(libc::pid_t),
    Reaped,
}

fn peek_exit(pid: libc::pid_t) -> IoResult<bool> {
    // A zeroed C record also initializes the no-exit sentinel.
    let mut info: libc::siginfo_t = unsafe { std::mem::zeroed() };
    let result = unsafe {
        libc::waitid(
            libc::P_PID,
            pid as libc::id_t,
            &mut info,
            libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
        )
    };
    if result == -1 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(unsafe { info.si_pid() } != 0)
}

struct ChildGroup {
    child: Option<Child>,
}

impl ChildGroup {
    fn stop(
        &mut self,
        #[cfg(test)] observer: &mut Option<&mut dyn FnMut(CleanupEvent)>,
    ) -> IoResult<Option<ExitStatus>> {
        let Some(child) = &mut self.child else {
            return Ok(None);
        };
        let pid = child.id() as libc::pid_t;
        loop {
            match peek_exit(pid) {
                Ok(_) => break,
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(error) => {
                    if error.raw_os_error() == Some(libc::ECHILD) {
                        self.child = None;
                    }
                    return Err(error);
                }
            }
        }
        #[cfg(test)]
        if let Some(observe) = observer {
            observe(CleanupEvent::BeforeGroupKill(pid));
        }
        // The unreaped child pins the PGID even when every live member has exited.
        if unsafe { libc::kill(-pid, libc::SIGKILL) } == -1 {
            let error = std::io::Error::last_os_error();
            if error.raw_os_error() != Some(libc::ESRCH) {
                return Err(error);
            }
        }
        #[cfg(test)]
        if let Some(observe) = observer {
            observe(CleanupEvent::AfterGroupKill(pid));
        }
        let status = child.wait();
        self.child = None;
        #[cfg(test)]
        if status.is_ok()
            && let Some(observe) = observer
        {
            observe(CleanupEvent::Reaped);
        }
        status.map(Some)
    }
}

impl Drop for ChildGroup {
    fn drop(&mut self) {
        #[cfg(test)]
        let _ = self.stop(&mut None);
        #[cfg(not(test))]
        let _ = self.stop();
    }
}

enum PipeResult {
    Stdout(IoResult<Vec<u8>>),
    Stderr(IoResult<Vec<u8>>),
}

fn stderr_prefix(mut reader: impl Read) -> IoResult<Vec<u8>> {
    let mut prefix = Vec::new();
    let mut buffer = [0; 4096];
    loop {
        let count = reader.read(&mut buffer)?;
        if count == 0 {
            return Ok(prefix);
        }
        let keep = count.min(200 - prefix.len());
        prefix.extend_from_slice(&buffer[..keep]);
    }
}

pub(crate) fn fetch(request: FetchRequest, deadline: Instant) -> DocRead {
    #[cfg(test)]
    {
        fetch_inner(request, deadline, None)
    }
    #[cfg(not(test))]
    {
        fetch_inner(request, deadline)
    }
}

fn fetch_inner(
    request: FetchRequest,
    deadline: Instant,
    #[cfg(test)] mut observer: Option<&mut dyn FnMut(CleanupEvent)>,
) -> DocRead {
    let unavailable = |cause: String| DocRead::Unavailable {
        reference: request.reference.clone(),
        cause: UnavailableCause::FetchFailed(cause),
    };
    if Instant::now() >= deadline {
        return unavailable("fetch deadline exceeded before launch".into());
    }
    let child = match Command::new("sh")
        .args([
            "-c",
            &format!("{} \"$1\"", request.command),
            "--",
            &request.rest,
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0)
        .spawn()
    {
        Ok(child) => child,
        Err(error) => return unavailable(format!("failed to start fetcher: {error}")),
    };
    let mut group = ChildGroup { child: Some(child) };
    let child = group.child.as_mut().expect("child was spawned");
    let pid = child.id() as libc::pid_t;
    let stdout = child.stdout.take().expect("stdout is piped");
    let stderr = child.stderr.take().expect("stderr is piped");
    let (sender, receiver) = mpsc::channel();
    let out_sender = sender.clone();
    let stdout_reader = match std::thread::Builder::new().spawn(move || {
        let mut bytes = Vec::new();
        let result = stdout
            .take(MAX_STDOUT + 1)
            .read_to_end(&mut bytes)
            .map(|_| bytes);
        let _ = out_sender.send(PipeResult::Stdout(result));
    }) {
        Ok(reader) => reader,
        Err(error) => return unavailable(format!("failed to start stdout reader: {error}")),
    };
    let stderr_reader = match std::thread::Builder::new().spawn(move || {
        let _ = sender.send(PipeResult::Stderr(stderr_prefix(stderr)));
    }) {
        Ok(reader) => reader,
        Err(error) => {
            #[cfg(test)]
            let cleanup = group.stop(&mut observer);
            #[cfg(not(test))]
            let cleanup = group.stop();
            let joined = stdout_reader.join();
            return unavailable(format!(
                "failed to start stderr reader: {error}; cleanup: {cleanup:?}; stdout reader: {joined:?}"
            ));
        }
    };
    let mut output = None;
    let mut stderr = None;
    let mut status = None;
    let mut failure = None;
    loop {
        if status.is_some() && output.is_some() && stderr.is_some() {
            break;
        }
        let Some(remaining) = deadline.checked_duration_since(Instant::now()) else {
            failure = Some("fetch deadline exceeded".to_string());
            break;
        };
        if status.is_none() {
            match peek_exit(pid) {
                Ok(true) => {
                    #[cfg(test)]
                    let stopped = group.stop(&mut observer);
                    #[cfg(not(test))]
                    let stopped = group.stop();
                    match stopped {
                        Ok(found) => status = found,
                        Err(error) => {
                            failure = Some(format!("fetch cleanup: {error}"));
                            break;
                        }
                    }
                }
                Ok(false) => {}
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(error) => {
                    if error.raw_os_error() == Some(libc::ECHILD) {
                        group.child = None;
                    }
                    failure = Some(format!("fetch exit observation: {error}"));
                    break;
                }
            }
        }
        let pause = remaining.min(Duration::from_millis(10));
        if output.is_some() && stderr.is_some() {
            std::thread::sleep(pause);
            continue;
        }
        match receiver.recv_timeout(pause) {
            Ok(PipeResult::Stdout(Ok(bytes))) => {
                if bytes.len() as u64 > MAX_STDOUT {
                    failure = Some(format!("stdout exceeds {MAX_STDOUT} bytes"));
                    break;
                }
                output = Some(bytes);
            }
            Ok(PipeResult::Stderr(Ok(bytes))) => stderr = Some(bytes),
            Ok(PipeResult::Stdout(Err(error))) | Ok(PipeResult::Stderr(Err(error))) => {
                failure = Some(format!("fetch pipe: {error}"));
                break;
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                failure = Some("fetch pipe readers disconnected".into());
                break;
            }
        }
    }
    #[cfg(test)]
    let stopped = group.stop(&mut observer);
    #[cfg(not(test))]
    let stopped = group.stop();
    if let Err(error) = stopped {
        failure = Some(format!("fetch cleanup: {error}"));
    }
    let stdout_joined = stdout_reader.join();
    let stderr_joined = stderr_reader.join();
    if stdout_joined.is_err() || stderr_joined.is_err() {
        failure = Some("fetch pipe reader panicked".into());
    }
    for result in receiver.try_iter() {
        if let PipeResult::Stderr(Ok(bytes)) = result {
            stderr = Some(bytes);
        }
    }
    let prefix = String::from_utf8_lossy(stderr.as_deref().unwrap_or_default());
    let failed = |error: String| unavailable(format!("{error}; stderr: {prefix}"));
    if let Some(error) = failure {
        return failed(error);
    }
    let status = status.expect("completed fetch has an exit status");
    if !status.success() {
        return failed(format!("fetcher {status}"));
    }
    #[derive(serde::Deserialize)]
    struct Payload {
        id: String,
        body: String,
    }
    let payload: Payload =
        match serde_json::from_slice(&output.expect("completed fetch has stdout")) {
            Ok(payload) => payload,
            Err(error) => return failed(format!("invalid fetch JSON: {error}")),
        };
    let (scheme, _) = request
        .reference
        .split_once(':')
        .expect("external request has a scheme");
    let reference = format!("{scheme}:{}", payload.id);
    if let Err(error) = crate::format::external_doc(&reference) {
        return failed(format!("invalid fetched id: {error}"));
    }
    DocRead::Available(Doc {
        reference,
        body: payload.body,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resolve::DocRead;

    fn request(command: &str, rest: &str) -> FetchRequest {
        FetchRequest {
            reference: "stub:test".into(),
            command: command.into(),
            rest: rest.into(),
        }
    }

    fn run(command: &str, rest: &str) -> DocRead {
        fetch(
            request(command, rest),
            Instant::now() + Duration::from_secs(2),
        )
    }

    #[test]
    fn payload_requires_one_object_and_valid_id() {
        for text in [
            "not json",
            "{}",
            r#"{"id":1,"body":""}"#,
            r#"{"id":"","body":""}"#,
            r#"{"id":"a\u0000b","body":""}"#,
            r#"{"id":"test","body":""} {}"#,
        ] {
            assert!(
                matches!(run("printf '%s'", text), DocRead::Unavailable { .. }),
                "{text}"
            );
        }
        let doc = run("printf '%s'", r#"{"id":"test","body":"","extra":true}"#)
            .required()
            .unwrap();
        assert_eq!(doc.reference, "stub:test");
        assert_eq!(doc.body, "");
    }

    #[test]
    fn expired_deadline_does_not_spawn() {
        let mut cleanup_events = 0;
        let mut observe = |_| cleanup_events += 1;
        let result = fetch_inner(
            request("printf '%s'", r#"{"id":"test","body":""}"#),
            Instant::now() - Duration::from_secs(1),
            Some(&mut observe),
        );
        assert!(matches!(result, DocRead::Unavailable { .. }));
        assert_eq!(cleanup_events, 0);
    }

    #[test]
    fn invalid_response_retains_fetcher_diagnostic() {
        let result = run("printf 'diagnostic' >&2; printf 'bad json'; :", "input");
        assert!(
            result
                .required()
                .unwrap_err()
                .to_string()
                .contains("diagnostic")
        );
    }

    #[test]
    fn single_argument_is_not_shell_code() {
        let temp = tempfile::tempdir().unwrap();
        let captured = temp.path().join("argument");
        let command = format!(
            "printf '%s' \"$1\" > '{}'; printf '%s' '{{\"id\":\"test\",\"body\":\"ok\"}}'; :",
            captured.display()
        );
        let rest = "space ' quote \" $(false); $HOME /part";
        assert_eq!(run(&command, rest).required().unwrap().body, "ok");
        assert_eq!(std::fs::read_to_string(captured).unwrap(), rest);
    }

    #[test]
    fn stderr_is_drained_and_failure_retains_only_prefix() {
        let command = "head -c 100000 /dev/zero | tr '\\000' x >&2; printf '%s' '{\"id\":\"test\",\"body\":\"ok\"}'; :";
        assert_eq!(run(command, "input").required().unwrap().body, "ok");
        let failed = run(
            "head -c 100000 /dev/zero | tr '\\000' x >&2; exit 7; :",
            "input",
        )
        .required()
        .unwrap_err()
        .to_string();
        assert!(failed.contains(&"x".repeat(200)));
        assert!(!failed.contains(&"x".repeat(201)));
    }

    #[test]
    fn overflow_and_invalid_utf8_are_unavailable() {
        let command = format!("head -c {} /dev/zero; :", MAX_STDOUT + 1);
        assert!(
            run(&command, "input")
                .required()
                .unwrap_err()
                .to_string()
                .contains("stdout")
        );
        assert!(matches!(
            run("printf '\\377'; :", "input"),
            DocRead::Unavailable { .. }
        ));
    }

    #[test]
    fn timeout_cleans_up_child_and_pipe_holder() {
        let temp = tempfile::tempdir().unwrap();
        let pidfile = temp.path().join("pid");
        let command = format!(
            "sleep 60 & printf '%s' $! > '{}'; wait; :",
            pidfile.display()
        );
        let started = Instant::now();
        let result = fetch(
            request(&command, "input"),
            started + Duration::from_millis(150),
        );
        assert!(
            result
                .required()
                .unwrap_err()
                .to_string()
                .contains("deadline")
        );
        assert!(started.elapsed() < Duration::from_secs(2));
        let pid: i32 = std::fs::read_to_string(pidfile).unwrap().parse().unwrap();
        assert_not_running(pid);
    }

    fn assert_not_running(pid: i32) {
        let deadline = Instant::now() + Duration::from_secs(1);
        loop {
            let state = std::fs::read_to_string(format!("/proc/{pid}/stat"));
            if state.is_err() || state.unwrap().rsplit_once(") ").unwrap().1.starts_with('Z') {
                return;
            }
            assert!(Instant::now() < deadline, "child {pid} remains running");
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    #[test]
    fn successful_exit_stops_pipe_holder_before_deadline() {
        let temp = tempfile::tempdir().unwrap();
        let pidfile = temp.path().join("pid");
        let command = format!(
            "sleep 60 & printf '%s' $! > '{}'; printf '%s' '{{\"id\":\"test\",\"body\":\"ok\"}}'; exit 0; :",
            pidfile.display()
        );
        let started = Instant::now();
        assert_eq!(run(&command, "input").required().unwrap().body, "ok");
        assert!(started.elapsed() < Duration::from_secs(1));
        let pid = std::fs::read_to_string(pidfile).unwrap().parse().unwrap();
        assert_not_running(pid);
    }

    #[test]
    fn shell_remains_unreaped_until_group_kill() {
        let mut order = Vec::new();
        let mut pinned = Vec::new();
        let mut observe = |event| match event {
            CleanupEvent::BeforeGroupKill(pid) => {
                order.push("before kill");
                pinned.push(matches!(peek_exit(pid), Ok(true)));
            }
            CleanupEvent::AfterGroupKill(pid) => {
                order.push("after kill");
                pinned.push(matches!(peek_exit(pid), Ok(true)));
            }
            CleanupEvent::Reaped => order.push("reaped"),
        };
        let result = fetch_inner(
            request("printf '%s'", r#"{"id":"test","body":""}"#),
            Instant::now() + Duration::from_secs(1),
            Some(&mut observe),
        );
        assert_eq!(result.required().unwrap().body, "");
        assert_eq!(order, ["before kill", "after kill", "reaped"]);
        assert_eq!(pinned, [true, true]);
    }
}
