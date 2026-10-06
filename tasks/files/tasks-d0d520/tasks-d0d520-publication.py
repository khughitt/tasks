"""Disposable dependency publication probe; python3 FILE [tasks-binary]."""
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tempfile

binary = str(Path(sys.argv[1] if len(sys.argv) > 1 else shutil.which("tasks")).resolve())
log = []
phases = []
with tempfile.TemporaryDirectory(prefix="tasks-publication-") as scratch:
    root = Path(scratch)
    up, down = root / "up", root / "down"
    env = os.environ.copy()
    for key in list(env):
        if key.startswith(("TASKS_", "CODEX_", "CLAUDE_")):
            env.pop(key)
    env.update(XDG_CONFIG_HOME=str(root / "config"), XDG_STATE_HOME=str(root / "state"),
               RELAY_STATE_DIR=str(root / "relay"), TASKS_OWNER="probe",
               GIT_CONFIG_GLOBAL="/dev/null", GIT_CONFIG_NOSYSTEM="1")

    def run(cwd, *args, code=0, session="dependency-probe"):
        out = subprocess.run(args, cwd=cwd, env={**env, "TASKS_SESSION": session},
                             text=True, capture_output=True, timeout=30)
        stdout = out.stdout.replace(scratch, "<fixture>").replace(os.uname().nodename, "<host>")
        if args[0] == "work-link":
            stdout = re.sub(r"is not under .*; stays in-tree",
                            "is outside the configured checkout root; stays in-tree", stdout)
        log.append({"cwd": str(cwd).replace(scratch, "<fixture>"), "session": session,
                    "argv": ["tasks" if a == binary else a.replace(scratch, "<fixture>") for a in args],
                    "exit": out.returncode, "stdout": stdout,
                    "stderr": out.stderr.replace(scratch, "<fixture>").replace(os.uname().nodename, "<host>")})
        assert out.returncode == code, log[-1]
        return out

    def task(cwd, *args, **kw):
        out = run(cwd, binary, *args, **kw)
        return json.loads(out.stdout if kw.get("code", 0) == 0 else out.stderr)

    def commit(cwd, message):
        run(cwd, "git", "add", "tasks")
        run(cwd, "git", "commit", "-m", message)

    for project, prefix in [(up, "up"), (down, "down")]:
        project.mkdir()
        run(project, "git", "init", "-b", "main")
        run(project, "git", "config", "user.name", "Probe")
        run(project, "git", "config", "user.email", "probe@example.invalid")
        task(project, "init", "--prefix", prefix)
        if project == up:
            (up / "runtime.txt").write_text("live\n")
            run(up, "git", "add", "runtime.txt")
        commit(project, "chore: initialize fixture")
    version = run(up, binary, "--version").stdout.strip()
    git_version = run(up, "git", "--version").stdout.strip()
    run(up, "work-link", "--ensure", ".worktrees")
    wt = up / ".worktrees" / "publication"
    run(up, "git", "worktree", "add", "-b", "work", str(wt))
    run(up, "git", "worktree", "lock", "--reason", "disposable publication probe", str(wt))
    upstream = task(wt, "add", "Preparation", "--process", "direct")["id"]
    task(wt, "start", upstream, session="up-worker")
    (wt / "runtime.txt").write_text("prepared\n")
    run(wt, "git", "add", "runtime.txt")
    commit(wt, "chore: prepare worktree record and runtime change")
    downstream = task(down, "add", "Dependent work", "--process", "direct")["id"]
    up_file, down_file = f"tasks/{upstream}.md", f"tasks/{downstream}.md"

    def rejected(cwd, kind, *args, session="dependency-probe"):
        before = (wt / up_file).read_bytes(), (down / down_file).read_bytes()
        error = task(cwd, *args, code=1, session=session)
        assert error["error"]["kind"] == kind, error
        assert before == ((wt / up_file).read_bytes(), (down / down_file).read_bytes())
        return error

    def ready(wide=False):
        args = ["ready"] + (["--all-projects"] if wide else [])
        return downstream in [t["id"] for t in task(down, *args)["tasks"]]

    shown = task(down, "show", upstream)
    assert shown["task"]["status"] == "doing"
    assert any("exists only in" in w for w in shown["warnings"])
    assert shown["claim"]["session"] == "up-worker" and shown["claim"]["live"]
    rejected(down, "unresolvable_id", "dep", downstream, "--on", upstream)
    assert not (up / up_file).exists()
    assert ready() and ready(True)  # No edge was written by the failed command.
    phases.append({"phase": "worktree-only claimed upstream", "direct_show": "doing (claim fallback)",
                   "dependency_write": "unresolvable_id; no edge written",
                   "downstream_ready": True, "wide_ready": True})

    def publish(message):
        run(up, "git", "restore", "--source=work", "--", up_file)
        assert (up / up_file).read_bytes() == (wt / up_file).read_bytes()
        assert (up / "runtime.txt").read_text() == "live\n"
        assert (wt / "runtime.txt").read_text() == "prepared\n"
        commit(up, message)

    publish("chore: publish only the preparation record")
    task(down, "dep", downstream, "--on", upstream)
    shown = task(down, "show", upstream)
    assert not any("exists only in" in w for w in shown["warnings"])
    assert shown["claim"]["session"] == "up-worker" and shown["claim"]["live"]
    assert task(down, "show", downstream)["depends_on"][0]["status"] == "doing"
    assert not ready() and not ready(True)
    phases.append({"phase": "open upstream record published", "direct_show": "doing (registered)",
                   "dependency_write": "succeeds despite foreign upstream claim",
                   "dependency_show": "doing", "downstream_ready": False, "wide_ready": False})

    # Published dependency edges enforce cycles; foreign ownership still guards closure.
    rejected(wt, "cycle", "dep", upstream, "--on", downstream, session="up-worker")
    main_before = (up / up_file).read_bytes()
    rejected(down, "claimed", "done", upstream, "foreign attempt")
    assert (up / up_file).read_bytes() == main_before
    assert task(down, "show", upstream)["claim"]["session"] == "up-worker"
    rejected(down, "unresolvable_id", "dep", downstream, "--on", "absent-000001")

    # Missing/config-invalid authority never becomes a satisfied dependency.
    config = up / "tasks/.config.toml"
    config.rename(up / "tasks/.config.saved")
    rejected(down, "unresolvable_id", "dep", downstream, "--on", upstream)
    unavailable = task(down, "ready")
    assert downstream not in [t["id"] for t in unavailable["tasks"]]
    assert any("unreachable" in w and upstream in w for w in unavailable["warnings"])
    (up / "tasks/.config.saved").rename(config)
    original_config = config.read_text()
    config.write_text(original_config + "\n[broken\n")
    rejected(down, "config", "dep", downstream, "--on", upstream)
    assert task(down, "ready", code=1)["error"]["kind"] == "config"
    config.write_text(original_config)

    task(wt, "done", upstream, "preparation complete", session="up-worker")
    commit(wt, "chore: complete preparation in worktree")
    assert task(wt, "show", upstream)["task"]["status"] == "done"
    shown = task(down, "show", upstream)
    assert shown["task"]["status"] == "doing" and "claim" not in shown
    assert task(down, "show", downstream)["depends_on"][0]["status"] == "doing"
    assert not ready() and not ready(True)
    phases.append({"phase": "completion remains worktree-only", "direct_show": "doing (registered)",
                   "dependency_show": "doing", "downstream_ready": False, "wide_ready": False})

    publish("chore: publish only the completed record")
    assert task(down, "show", upstream)["task"]["status"] == "done"
    assert task(down, "show", downstream)["depends_on"][0]["status"] == "done"
    assert ready() and ready(True)
    phases.append({"phase": "completed record published", "direct_show": "done (registered)",
                   "dependency_show": "done", "downstream_ready": True, "wide_ready": True})
    for project in (wt, up, down):
        run(project, binary, "check")
    run(up, "git", "worktree", "unlock", str(wt))
    run(up, "git", "worktree", "remove", str(wt))

print(json.dumps({"tasks_version": version, "git_version": git_version,
                  "binary_sha256": hashlib.sha256(Path(binary).read_bytes()).hexdigest(),
                  "verdict": "passed", "recommendation": "explicit record publication to registered authority",
                  "phases": phases, "checks": {"cycle_refused": True, "foreign_close_refused": True,
                  "missing_authority_blocks": True, "malformed_config_errors": True,
                  "failed_writes_preserve_records": True, "runtime_unchanged": True},
                  "fixture_removed": True, "commands": log}, indent=2))
