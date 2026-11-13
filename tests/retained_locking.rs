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
        self.stream.write_all(b"+").unwrap();
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
    let deadline = Instant::now() + Duration::from_secs(30);
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
            .env("TASKS_RETAINED_OBSERVER", socket)
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
                .env("TASKS_RETAINED_OBSERVER", &prefix_socket)
                .args(["note", &f.id, "Same prefix waits"])
                .spawn()
                .unwrap(),
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
        let guard = event(&listener, &mut child, "guard_acquired");
        let writer_socket = f.dir.join("writer.sock");
        let writer_listener = self::listener(&writer_socket);
        let mut writer = Running(Some(
            f.env
                .raw(&f.dir)
                .env("TASKS_SESSION", "retained-fixture")
                .env("TASKS_RETAINED_OBSERVER", &writer_socket)
                .args(["note", &other, "Unrelated waits for publication"])
                .spawn()
                .unwrap(),
        ));
        let attempt = event(&writer_listener, &mut writer, "commit_wait");
        let common = attempt.value["detail"].as_str().unwrap();
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
        if readback.value["detail"] == "sender_released" {
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
