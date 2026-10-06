"""Disposable stale-record recovery probe; python3 FILE [tasks-binary]."""
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
with tempfile.TemporaryDirectory(prefix="tasks-recovery-") as scratch:
    root = Path(scratch)
    main = root / "main"
    main.mkdir()
    env = os.environ.copy()
    for key in list(env):
        if key.startswith(("TASKS_", "CODEX_", "CLAUDE_")):
            env.pop(key)
    env.update(XDG_CONFIG_HOME=str(root / "config"), XDG_STATE_HOME=str(root / "state"),
               RELAY_STATE_DIR=str(root / "relay"), TASKS_SESSION="recovery-probe",
               TASKS_OWNER="probe", GIT_CONFIG_GLOBAL="/dev/null", GIT_CONFIG_NOSYSTEM="1")

    def run(cwd, *args, code=0):
        out = subprocess.run(args, cwd=cwd, env=env, text=True, capture_output=True, timeout=30)
        stdout = out.stdout.replace(scratch, "<fixture>")
        if args[0] == "work-link":
            stdout = re.sub(r"is not under .*; stays in-tree",
                            "is outside the configured checkout root; stays in-tree", stdout)
        log.append({"cwd": str(cwd).replace(scratch, "<fixture>"),
                    "argv": ["tasks" if a == binary else a.replace(scratch, "<fixture>") for a in args],
                    "exit": out.returncode,
                    "stdout": stdout,
                    "stderr": out.stderr.replace(scratch, "<fixture>")})
        assert out.returncode == code, log[-1]
        return out

    def task(cwd, *args, code=0):
        out = run(cwd, binary, *args, code=code)
        return json.loads(out.stdout if code == 0 else out.stderr)

    def commit(cwd, message):
        run(cwd, "git", "add", "tasks")
        run(cwd, "git", "commit", "-m", message)

    run(main, "git", "init", "-b", "base")
    run(main, "git", "config", "user.name", "Probe")
    run(main, "git", "config", "user.email", "probe@example.invalid")
    version = run(main, binary, "--version").stdout.strip()
    git_version = run(main, "git", "--version").stdout.strip()
    task(main, "init", "--prefix", "probe")
    task_id = task(main, "add", "Recovery fixture", "--process", "direct")["id"]
    task(main, "note", task_id, "common history")
    commit(main, "chore: create common record")
    run(main, "git", "switch", "-c", "local")
    task(main, "note", task_id, "local-only history")
    commit(main, "chore: retain local history")
    run(main, "git", "switch", "-c", "source", "base")
    task(main, "note", task_id, "source-only history")
    task(main, "note", task_id, "source second note")
    commit(main, "chore: advance authoritative record")
    run(main, "work-link", "--ensure", ".worktrees")
    wt = main / ".worktrees" / "recovery"
    run(main, "git", "worktree", "add", "-b", "clean", str(wt), "base")
    run(main, "git", "worktree", "lock", "--reason", "disposable recovery probe", str(wt))
    file = f"tasks/{task_id}.md"

    # Ordinary authoritative integration: a behind-only checkout fast-forwards.
    before = (wt / file).read_bytes()
    refused = task(wt, "note", task_id, "clean retry", code=1)
    assert refused["error"]["kind"] == "stale_copy", refused
    assert (wt / file).read_bytes() == before
    run(wt, "git", "merge", "--ff-only", "source")
    task(wt, "note", task_id, "clean retry")
    commit(wt, "chore: verify ordinary handoff")

    # Divergent history is authored through the CLI before the sibling exists.
    run(wt, "git", "switch", "local")
    local_bytes = (wt / file).read_bytes()
    refused = task(wt, "note", task_id, "union retry", code=1)
    assert refused["error"]["kind"] == "stale_copy", refused
    assert (wt / file).read_bytes() == local_bytes
    run(wt, "git", "merge", "--no-commit", "--no-ff", "source", code=1)
    stages = [run(wt, "git", "show", f":{n}:{file}").stdout for n in (1, 2, 3)]
    parts = [text.split("\n---\n", 1) for text in stages]
    for n, (_, body) in enumerate(parts):
        (root / f"body-{n}").write_text(body)
    union = run(wt, "git", "merge-file", "--union", "-p",
                str(root / "body-1"), str(root / "body-0"), str(root / "body-2")).stdout
    resolved = parts[2][0] + "\n---\n" + union
    # Conflict resolution retains the source header verbatim, including its stamp.
    (wt / file).write_text(resolved)
    commit(wt, "chore: resolve conflict retaining both histories")
    source_stamp = task(main, "show", task_id)["task"]["updated"]
    union_task = task(wt, "show", task_id)["task"]
    assert union_task["updated"] == source_stamp
    expected = ["common history", "local-only history", "source-only history", "source second note"]
    assert [n["text"] for n in union_task["notes"]] == expected, union_task
    refusal = task(wt, "note", task_id, "union retry", code=1)
    assert refusal["error"]["kind"] == "stale_copy", refusal
    assert "same stamp" in refusal["error"]["detail"], refusal
    assert (wt / file).read_text() == resolved

    # Publish the merged record back to the other sibling, then retry through CLI.
    run(main, "git", "merge", "--ff-only", "local")
    assert (main / file).read_bytes() == (wt / file).read_bytes()
    task(wt, "note", task_id, "union retry")
    recovered = task(wt, "show", task_id)["task"]
    assert recovered["updated"] > source_stamp
    assert [n["text"] for n in recovered["notes"]] == expected + ["union retry"]
    assert recovered["notes"][:-1] == union_task["notes"]
    run(wt, binary, "check")
    commit(wt, "chore: verify lossless recovery")
    run(main, "git", "worktree", "unlock", str(wt))
    run(main, "git", "worktree", "remove", str(wt))

print(json.dumps({"tasks_version": version, "git_version": git_version,
                  "binary_sha256": hashlib.sha256(Path(binary).read_bytes()).hexdigest(),
                  "verdict": "passed", "clean_handoff": "fast-forward then CLI retry succeeds",
                  "union_with_source_stamp": "same-stamp stale_copy; nothing written",
                  "recovery": "integrate resolved union in both siblings, then CLI retry succeeds",
                  "history_preserved": True, "fixture_removed": True, "commands": log}, indent=2))
