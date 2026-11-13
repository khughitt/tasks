mod common;
use common::retained::*;
use serde_json::{Value, json};
use std::{
    io::{Read, Write},
    os::unix::net::{UnixListener, UnixStream},
    path::Path,
    process::{Child, Output},
    time::{Duration, Instant},
};

// Every stopped child is reaped, including an assertion or observer failure.
struct Running(Option<Child>);
impl Running {
    fn output(mut self) -> Output {
        self.0.take().unwrap().wait_with_output().unwrap()
    }
    fn finished(&mut self) -> bool {
        self.0.as_mut().unwrap().try_wait().unwrap().is_some()
    }
}
impl Drop for Running {
    fn drop(&mut self) {
        if let Some(child) = &mut self.0 {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}
struct Observation {
    stream: UnixStream,
    value: Value,
}
impl Observation {
    fn resume(mut self) {
        if self.value["barrier"] == true {
            self.stream.write_all(b"+").unwrap();
        }
    }
}
fn read(mut stream: UnixStream) -> Observation {
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    let mut line = Vec::new();
    loop {
        let mut b = [0];
        stream.read_exact(&mut b).unwrap();
        if b[0] == b'\n' {
            break;
        }
        line.push(b[0]);
        assert!(line.len() < 4096);
    }
    Observation {
        value: serde_json::from_slice(&line).unwrap(),
        stream,
    }
}
fn event(listener: &UnixListener, child: &mut Running, wanted: &str) -> Observation {
    event_until(listener, child, wanted, Duration::from_secs(30))
}
fn event_until(
    listener: &UnixListener,
    child: &mut Running,
    wanted: &str,
    timeout: Duration,
) -> Observation {
    let deadline = Instant::now() + timeout;
    loop {
        match listener.accept() {
            Ok((stream, _)) => {
                let observation = read(stream);
                if observation.value["event"] == wanted {
                    return observation;
                }
                observation.resume();
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                assert!(!child.finished(), "retained exited before {wanted}");
                assert!(Instant::now() < deadline, "retained never reached {wanted}");
                std::thread::sleep(Duration::from_millis(5));
            }
            Err(e) => panic!("observer accept: {e}"),
        }
    }
}
fn drain(listener: &UnixListener, mut child: Running) -> Output {
    let deadline = Instant::now() + Duration::from_secs(30);
    while !child.finished() {
        match listener.accept() {
            Ok((stream, _)) => read(stream).resume(),
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                assert!(Instant::now() < deadline, "retained did not finish");
                std::thread::sleep(Duration::from_millis(5));
            }
            Err(e) => panic!("observer accept: {e}"),
        }
    }
    child.output()
}
fn finish(listener: &UnixListener, child: Running) -> Output {
    let out = drain(listener, child);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    out
}
fn listener(path: &Path) -> UnixListener {
    let listener = UnixListener::bind(path).unwrap();
    listener.set_nonblocking(true).unwrap();
    listener
}
fn wait(mut child: Running) -> Output {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !child.finished() {
        assert!(Instant::now() < deadline, "unrelated writer did not finish");
        std::thread::sleep(Duration::from_millis(5));
    }
    let out = child.output();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    out
}
fn unrelated(f: &RetainedEnv) -> String {
    let root = f.dir.parent().unwrap().join("unrelated-code");
    std::fs::create_dir(&root).unwrap();
    common::git(&root, &["init", "-qb", "main"]);
    common::git_fixture::assert_repo(f.env.home.path(), &root);
    std::fs::write(root.join("README.md"), "fixture").unwrap();
    common::git(&root, &["add", "."]);
    common::git(&root, &["commit", "-qm", "seed"]);
    f.json(&[
        "-C",
        root.to_str().unwrap(),
        "init",
        "--prefix",
        "aux",
        "--records",
        f.records.to_str().unwrap(),
    ]);
    let config = f.records.join("aux/tasks/.config.toml");
    let mut value: toml::Value =
        toml::from_str(&std::fs::read_to_string(&config).unwrap()).unwrap();
    value.as_table_mut().unwrap().insert(
        "git".into(),
        toml::Value::try_from(json!({"commit":true})).unwrap(),
    );
    std::fs::write(config, toml::to_string(&value).unwrap()).unwrap();
    common::git(&f.records, &["add", "aux"]);
    common::git(&f.records, &["commit", "-qm", "unrelated configuration"]);
    f.json(&["add", "Unrelated writer", "--project", "aux"])["id"]
        .as_str()
        .unwrap()
        .into()
}
fn transfer_child(f: &RetainedEnv, to: &str, socket: &Path) -> Running {
    let request = transfer_request(f, to);
    let request = f.request("transfer.json", &request);
    Running(Some(
        f.env
            .raw(&f.dir)
            .env("TASKS_SESSION", "retained-fixture")
            .env("TASKS_RETAINED_TEST_SOCKET", socket)
            .args([
                "retained",
                "transfer",
                &f.id,
                to,
                "--request",
                request.to_str().unwrap(),
            ])
            .spawn()
            .unwrap(),
    ))
}
#[test]
fn retained_locking_unrelated_writer_during_inspection() {
    for shared in [true, false] {
        let (f, _, _) = transfer_fixture();
        let (_, to) = cross_receiver(&f, shared);
        let other = unrelated(&f);
        let socket = f.dir.join("observer.sock");
        let listener = listener(&socket);
        let mut child = transfer_child(&f, &to, &socket);
        let inspection = event(&listener, &mut child, "inspection");
        let writer = Running(Some(
            f.env
                .raw(&f.dir)
                .env("TASKS_SESSION", "retained-fixture")
                .args(["note", &other, "Unrelated during inspection"])
                .spawn()
                .unwrap(),
        ));
        let unrelated_commit_completed_before_inspection_resume = wait(writer).status.success();
        assert!(unrelated_commit_completed_before_inspection_resume);
        // Same-prefix writers remain serialized even during unlocked inspection.
        let prefix_socket = f.dir.join("prefix.sock");
        let prefix_listener = self::listener(&prefix_socket);
        let mut prefix_writer = Running(Some(
            f.env
                .raw(&f.dir)
                .env("TASKS_SESSION", "retained-fixture")
                .env("TASKS_RETAINED_TEST_SOCKET", &prefix_socket)
                .args(["note", &f.id, "Same prefix waits"])
                .spawn()
                .unwrap(),
        ));
        event(&prefix_listener, &mut prefix_writer, "prefix_wait").resume();
        let lock =
            std::fs::File::open(f.env.home.path().join(".local/state/tasks/claims/sci.lock"))
                .unwrap();
        assert!(matches!(
            lock.try_lock(),
            Err(std::fs::TryLockError::WouldBlock)
        ));
        assert!(!prefix_writer.finished());
        inspection.resume();
        let out = finish(&listener, child);
        assert_eq!(
            serde_json::from_slice::<Value>(&out.stdout).unwrap()["outcome"],
            "transferred"
        );
        finish(&prefix_listener, prefix_writer);
    }
}
#[test]
fn retained_locking_phase_excludes_writer() {
    use sha2::{Digest, Sha256};
    for shared in [true, false] {
        let (f, _, _) = transfer_fixture();
        let (_, to) = cross_receiver(&f, shared);
        let other = unrelated(&f);
        let socket = f.dir.join("observer.sock");
        let listener = listener(&socket);
        let mut child = transfer_child(&f, &to, &socket);
        // The initial guard has ended, and this publication's outside-lock precheck passed.
        event(&listener, &mut child, "prechecked").resume();
        let guard = event(&listener, &mut child, "before_write");
        let writer_socket = f.dir.join("writer.sock");
        let writer_listener = self::listener(&writer_socket);
        let mut writer = Running(Some(
            f.env
                .raw(&f.dir)
                .env("TASKS_SESSION", "retained-fixture")
                .env("TASKS_RETAINED_TEST_SOCKET", &writer_socket)
                .args(["note", &other, "Unrelated waits for publication"])
                .spawn()
                .unwrap(),
        ));
        let attempt = event(&writer_listener, &mut writer, "commit_wait");
        let common = attempt.value["phase"].as_str().unwrap();
        let digest = format!("{:x}", Sha256::digest(common.as_bytes()));
        let lock_path = f.env.home.path().join(format!(
            ".local/state/tasks/claims/.commit-{}.lock",
            &digest[..16]
        ));
        let lock = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(lock_path)
            .unwrap();
        assert!(
            matches!(lock.try_lock(), Err(std::fs::TryLockError::WouldBlock)),
            "publication owns the exact writer lock"
        );
        attempt.resume();
        let unrelated_commit_completed_before_guard_release = writer.finished();
        assert!(!unrelated_commit_completed_before_guard_release);
        guard.resume();
        finish(&listener, child);
        wait(writer);
    }
}
#[test]
fn retained_phase_scope_completion_observes_later_receiver() {
    let (f, _, file) = transfer_fixture();
    let (receiver_records, to) = cross_receiver(&f, false);
    let socket = f.dir.join("observer.sock");
    let listener = listener(&socket);
    let mut child = transfer_child(&f, &to, &socket);
    loop {
        let readback = event(&listener, &mut child, "readback");
        if readback.value["phase"] == "sender_released" {
            let path = receiver_records.join(format!("tasks/{to}.md"));
            let raw = std::fs::read_to_string(&path).unwrap();
            std::fs::write(
                &path,
                raw.replace("Foreign receiver", "Receiver worked after release"),
            )
            .unwrap();
            common::git(&receiver_records, &["add", path.to_str().unwrap()]);
            common::git(
                &receiver_records,
                &["commit", "-qm", "receiver after committed release"],
            );
            std::fs::remove_file(file).unwrap();
            readback.resume();
            break;
        }
        readback.resume();
    }
    let out = finish(&listener, child);
    assert_eq!(
        serde_json::from_slice::<Value>(&out.stdout).unwrap()["outcome"],
        "transferred"
    );
}

#[test]
fn retained_phase_freshness_guarded_delta() {
    use std::os::unix::fs::PermissionsExt;
    for case in ["unrelated", "ABA", "mode", "merge"] {
        let (f, to, _) = transfer_fixture();
        let socket = f.dir.join("observer.sock");
        let listener = listener(&socket);
        let mut child = transfer_child(&f, &to, &socket);
        let precheck = event(&listener, &mut child, "prechecked");
        let path = f.records.join(format!("tasks/{}.md", f.id));
        let old = std::fs::read(&path).unwrap();
        let commit = |message| {
            common::git(&f.records, &["add", "."]);
            common::git(&f.records, &["commit", "-qm", message]);
        };
        match case {
            "unrelated" => {
                std::fs::write(f.records.join("unrelated.txt"), "new unrelated edge").unwrap();
                commit("unrelated after precheck");
            }
            "mode" => {
                std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
                commit("participant mode after precheck");
            }
            _ => {
                if case == "merge" {
                    common::git(&f.records, &["checkout", "-qb", "side"]);
                }
                std::fs::write(&path, [old.as_slice(), b"\n"].concat()).unwrap();
                commit("participant intervening edit");
                std::fs::write(&path, &old).unwrap();
                commit("participant reverted bytes");
                if case == "merge" {
                    common::git(&f.records, &["checkout", "main"]);
                    std::fs::write(f.records.join("unrelated.txt"), "main edge").unwrap();
                    commit("main edge");
                    common::git(
                        &f.records,
                        &[
                            "merge",
                            "--no-ff",
                            "-s",
                            "ours",
                            "side",
                            "-m",
                            "new side history after precheck",
                        ],
                    );
                }
            }
        }
        precheck.resume();
        let out = drain(&listener, child);
        if case == "unrelated" {
            assert!(
                out.status.success(),
                "{}",
                String::from_utf8_lossy(&out.stderr)
            );
        } else {
            assert!(
                !out.status.success(),
                "{case} must refuse even when endpoint bytes match"
            );
            assert!(
                String::from_utf8_lossy(&out.stderr).contains("retained_stale_basis"),
                "{out:?}"
            );
        }
    }
}

#[test]
fn retained_phase_freshness_dirty_working_preimage() {
    use std::os::unix::fs::PermissionsExt;
    for case in ["peer_mode", "sender_physical_bytes", "attachment_mode"] {
        let (f, to, _) = transfer_fixture();
        let socket = f.dir.join("observer.sock");
        let listener = listener(&socket);
        let mut child = transfer_child(&f, &to, &socket);
        let precheck = event(&listener, &mut child, "prechecked");
        let path = match case {
            "peer_mode" => f.records.join(format!("tasks/{to}.md")),
            "attachment_mode" => std::fs::read_dir(f.records.join(format!("tasks/files/{}", f.id)))
                .unwrap()
                .next()
                .unwrap()
                .unwrap()
                .path(),
            _ => f.records.join(format!("tasks/{}.md", f.id)),
        };
        if case == "sender_physical_bytes" {
            let mut bytes = std::fs::read(&path).unwrap();
            bytes.push(b'\n');
            std::fs::write(&path, bytes).unwrap();
        } else {
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        // Deliberately leave the edit unstaged and uncommitted: H is still H0.
        precheck.resume();
        let out = drain(&listener, child);
        assert!(
            !out.status.success(),
            "dirty {case} must refuse before publication"
        );
        assert!(
            String::from_utf8_lossy(&out.stderr).contains("retained_stale_basis"),
            "{out:?}"
        );
    }
}
#[test]
fn retained_observer_timing_frames_need_no_ack() {
    let (f, to, _) = transfer_fixture();
    let socket = f.dir.join("observer.sock");
    let listener = listener(&socket);
    let mut child = transfer_child(&f, &to, &socket);
    let observation = event(&listener, &mut child, "lock_wait");
    // Keep this socket open without acknowledgement; timing frames cannot suspend work.
    let barrier = event_until(&listener, &mut child, "prechecked", Duration::from_secs(2));
    drop(observation);
    barrier.resume();
    finish(&listener, child);
}

#[test]
fn retained_observer_disconnect_and_blocked_child_cleanup() {
    for mode in ["disconnect", "unknown", "kill"] {
        let (f, to, _) = transfer_fixture();
        let socket = f.dir.join("observer.sock");
        let listener = listener(&socket);
        let mut child = transfer_child(&f, &to, &socket);
        let mut barrier = event(&listener, &mut child, "before_write");
        let pid = child.0.as_ref().unwrap().id() as libc::pid_t;
        if mode != "kill" {
            if mode == "unknown" {
                barrier.stream.write_all(b"?").unwrap();
            }
            drop(barrier);
            let deadline = Instant::now() + Duration::from_secs(5);
            while !child.finished() {
                assert!(Instant::now() < deadline);
                std::thread::sleep(Duration::from_millis(5));
            }
            assert!(
                !child.output().status.success(),
                "disconnected parent must refuse continuation"
            );
        } else {
            drop(child); // The scoped guard terminates and reaps a child still awaiting acknowledgement.
            drop(barrier);
        }
        let mut status = 0;
        assert_eq!(
            unsafe { libc::waitpid(pid, &mut status, libc::WNOHANG) },
            -1
        );
        assert_eq!(
            std::io::Error::last_os_error().raw_os_error(),
            Some(libc::ECHILD)
        );
        f.json(&["note", &f.id, "Child released its prefix and commit locks"]);
    }
}

#[test]
fn retained_phase_freshness_post_release_dirty_target() {
    use std::os::unix::fs::PermissionsExt;
    let (f, to, _) = transfer_fixture();
    let socket = f.dir.join("observer.sock");
    let listener = listener(&socket);
    let mut child = transfer_child(&f, &to, &socket);
    loop {
        let readback = event(&listener, &mut child, "readback");
        let released = readback.value["phase"] == "sender_released";
        readback.resume();
        if released {
            break;
        }
    }
    let precheck = event(&listener, &mut child, "prechecked");
    assert_eq!(precheck.value["phase"], to);
    std::fs::set_permissions(
        f.records.join(format!("tasks/{to}.md")),
        std::fs::Permissions::from_mode(0o755),
    )
    .unwrap();
    precheck.resume();
    let out = drain(&listener, child);
    assert!(
        !out.status.success(),
        "post-release target mode must remain exact under guard"
    );
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("retained_stale_basis"),
        "{out:?}"
    );
}
#[test]
fn retained_observer_final_release_disconnect_refuses_success() {
    let (f, to, _) = transfer_fixture();
    let socket = f.dir.join("observer.sock");
    let mut listener = Some(listener(&socket));
    let mut child = transfer_child(&f, &to, &socket);
    let mut completed = 0;
    loop {
        let readback = event(listener.as_ref().unwrap(), &mut child, "readback");
        if readback.value["phase"] == "completed" {
            completed += 1;
        }
        if completed == 2 {
            drop(listener.take());
            readback.resume();
            break;
        }
        readback.resume();
    }
    let deadline = Instant::now() + Duration::from_secs(5);
    while !child.finished() {
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(
        !child.output().status.success(),
        "final release observation failure must fail the test invocation"
    );
}

#[test]
fn lock_wait_failure_keeps_target_only_incomplete() {
    use sha2::{Digest, Sha256};
    let (f, _, payload) = transfer_fixture();
    let (_, to) = cross_receiver(&f, false);
    let socket = f.dir.join("broken-notice.sock");
    let listener = listener(&socket);
    let request = f.request("transfer.json", &transfer_request(&f, &to));
    let mut child = Running(Some(
        f.env
            .raw(&f.dir)
            .env("TASKS_SESSION", "retained-fixture")
            .env("TASKS_RETAINED_TEST_SOCKET", &socket)
            .stderr(
                std::fs::OpenOptions::new()
                    .write(true)
                    .open("/dev/full")
                    .unwrap(),
            )
            .args([
                "retained",
                "transfer",
                &f.id,
                &to,
                "--request",
                request.to_str().unwrap(),
            ])
            .spawn()
            .unwrap(),
    ));
    let inspection = event(&listener, &mut child, "inspection");
    // The receiver is stale and cannot become an implicit participant in the sender's receipt.
    let receiver_path = f.dir.parent().unwrap().join(format!("other/tasks/{to}.md"));
    let receiver = std::fs::read_to_string(&receiver_path).unwrap();
    std::fs::write(
        &receiver_path,
        receiver.replace("Foreign receiver", "Changed receiver"),
    )
    .unwrap();
    std::fs::remove_file(payload).unwrap();
    let output = common::git_fixture::command(f.env.home.path(), &f.records)
        .args(["rev-parse", "--path-format=absolute", "--git-common-dir"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let common = String::from_utf8(output.stdout).unwrap();
    let common = common.trim();
    let digest = format!("{:x}", Sha256::digest(common.as_bytes()));
    let path = f.env.home.path().join(format!(
        ".local/state/tasks/claims/.commit-{}.lock",
        &digest[..16]
    ));
    let held = std::fs::OpenOptions::new().write(true).open(&path).unwrap();
    held.lock().unwrap();
    inspection.resume();
    event(&listener, &mut child, "commit_wait").resume();
    std::thread::sleep(Duration::from_millis(2200));
    drop(held);
    let out = drain(&listener, child);
    assert!(
        !out.status.success(),
        "the original inspection failure remains an error"
    );
    let state = f.state();
    let phases = state["operations"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x["note"]["phase"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert!(phases.contains(&"incomplete"), "{state}");
    assert!(!phases.contains(&"sender_released"));
}

#[test]
fn retained_observer_git_frames_bound_mutable_arguments() {
    let (f, to, _) = transfer_fixture();
    let socket = f.dir.join("argument-observer.sock");
    let listener = listener(&socket);
    let mut child = transfer_child(&f, &to, &socket);
    loop {
        let observation = event(&listener, &mut child, "git_start");
        let command: Value =
            serde_json::from_str(observation.value["phase"].as_str().unwrap()).unwrap();
        if command["source"] != "immutable_read" {
            assert_eq!(
                command["argv"].as_array().unwrap().len(),
                1,
                "mutable observations expose only the command verb: {command}"
            );
            observation.resume();
            break;
        }
        observation.resume();
    }
    finish(&listener, child);
}

#[test]
fn migration_recovery_config_changes_at_phase_boundary() {
    let f = common::retained_migration::migration_fixture(HistoricalScenario::CheckpointCompleted);
    let request = json!({"version":1,"operation_id":"d".repeat(32),"host":host(),"inputs":[{"task":f.f.id,"basis":f.f.state()["basis"]}]});
    let request = f.f.request("recovery-race.json", &request);
    let socket = f.f.dir.join("recovery-race.sock");
    let listener = listener(&socket);
    let mut child = Running(Some(
        f.f.env
            .raw(&f.f.dir)
            .env("TASKS_SESSION", "retained-fixture")
            .env("TASKS_RETAINED_TEST_SOCKET", &socket)
            .args([
                "retained",
                "checkpoint",
                &f.f.id,
                "--request",
                request.to_str().unwrap(),
            ])
            .spawn()
            .unwrap(),
    ));
    let inspection = event(&listener, &mut child, "inspection");
    let path = f.f.records.join("tasks/.config.toml");
    let raw = std::fs::read_to_string(&path).unwrap();
    let mut config: toml::Value = toml::from_str(&raw).unwrap();
    config["retention_recovery"]["source_ref"] = toml::Value::String("refs/heads/missing".into());
    std::fs::write(&path, toml::to_string(&config).unwrap()).unwrap();
    inspection.resume();
    let out = drain(&listener, child);
    assert!(!out.status.success(), "{out:?}");
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("stale"),
        "{out:?}"
    );
    std::fs::write(path, raw).unwrap();
    let current = f.f.state();
    assert!(
        !current["operations"]
            .as_array()
            .unwrap()
            .iter()
            .any(|o| o["note"]["key"]["operation_id"] == "d".repeat(32)
                && o["note"]["phase"] == "completed")
    );
}

fn migration_observed(
    f: &common::retained_migration::MigrationFixture,
    args: &[&str],
) -> (Output, Vec<Value>) {
    let socket = f.f.dir.join("check-observer.sock");
    if socket.exists() {
        std::fs::remove_file(&socket).unwrap();
    }
    let listener = listener(&socket);
    let mut child = Running(Some(
        f.f.env
            .raw(&f.f.dir)
            .env("TASKS_SESSION", "retained-fixture")
            .env("TASKS_RETAINED_TEST_SOCKET", &socket)
            .args(args)
            .spawn()
            .unwrap(),
    ));
    let deadline = Instant::now() + Duration::from_secs(30);
    let mut observations = Vec::new();
    loop {
        match listener.accept() {
            Ok((stream, _)) => {
                let event = read(stream);
                observations.push(event.value.clone());
                event.resume();
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                if child.finished() {
                    break;
                }
                assert!(Instant::now() < deadline);
                std::thread::sleep(Duration::from_millis(5));
            }
            Err(e) => panic!("{e}"),
        }
    }
    (child.output(), observations)
}
#[test]
fn migration_check_zero_walks() {
    let f =
        common::retained_migration::migration_fixture(HistoricalScenario::TransferSenderReleased);
    let (out, observed) = migration_observed(&f, &["check"]);
    assert!(out.status.success(), "{out:?}");
    for task in f.declaration["tasks"].as_array().unwrap() {
        assert_eq!(
            observed
                .iter()
                .filter(|e| e["event"] == "recovery_prefix" && e["phase"] == *task)
                .count(),
            1,
            "{task}"
        );
    }
    for name in ["proof_history", "proof_terminal", "publication_discovery"] {
        assert_eq!(
            observed.iter().filter(|e| e["event"] == name).count(),
            0,
            "{name}"
        );
    }
    let (out, observed) = migration_observed(&f, &["retained", "state", &f.f.id]);
    assert!(out.status.success(), "{out:?}");
    for name in ["proof_history", "proof_terminal", "publication_discovery"] {
        assert!(
            observed.iter().any(|e| e["event"] == name),
            "observer did not expose {name} on actual state proof"
        );
    }
    assert!(
        observed
            .iter()
            .filter(|e| e["event"] == "publication_found")
            .any(|e| {
                let found: Value = serde_json::from_str(e["phase"].as_str().unwrap()).unwrap();
                found["basis"]["commit"] == f.cutover && found["basis"]["layout"] == "in-tree"
            }),
        "actual source publication was not observable"
    );
}
