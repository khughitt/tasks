"""Disposable closed-park recovery probe; python3 -I FILE [tasks-binary] > result.json.

Two hosts, A and B, each with its own XDG_CONFIG_HOME and XDG_STATE_HOME. A parks a
task from a git worktree; B closes the integrated record; A then inspects its views,
removes the worktree through git, and retries the existing recovery commands. Every
fixture is fresh, so each variant's effects are attributable.
"""
import difflib
import hashlib
import json
import os
import re
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile

binary = str(Path(sys.argv[1] if len(sys.argv) > 1 else shutil.which("tasks")).resolve())
base_env = {k: v for k, v in os.environ.items() if not k.startswith(("TASKS_", "CODEX_", "CLAUDE_", "XDG_"))}
base_env.update(TASKS_OWNER="probe", GIT_CONFIG_GLOBAL="/dev/null", GIT_CONFIG_NOSYSTEM="1",
                GIT_AUTHOR_NAME="Probe", GIT_AUTHOR_EMAIL="probe@example.invalid",
                GIT_COMMITTER_NAME="Probe", GIT_COMMITTER_EMAIL="probe@example.invalid")


class Fixture:
    def __init__(self, scratch, arrival):
        self.scratch, self.root, self.arrival = scratch, Path(scratch), arrival
        self.log = []
        self.hosts = {h: {"XDG_CONFIG_HOME": str(self.root / h / "config"),
                          "XDG_STATE_HOME": str(self.root / h / "state"),
                          "RELAY_STATE_DIR": str(self.root / h / "relay")} for h in "AB"}

    def clean(self, text):
        return text.replace(self.scratch, "<fixture>").replace(os.uname().nodename, "<host>")

    def run(self, cwd, *args, host="A", session="probe", extra=None, code=0):
        env = {**base_env, **self.hosts[host], "TASKS_SESSION": session, **(extra or {})}
        out = subprocess.run(args, cwd=cwd, env=env, text=True, capture_output=True, timeout=60)
        entry = {"host": host, "session": session, "cwd": self.clean(str(cwd)),
                 "argv": ["tasks" if a == binary else self.clean(a) for a in args],
                 "exit": out.returncode, "stdout": self.clean(out.stdout), "stderr": self.clean(out.stderr)}
        if extra:
            entry["env"] = extra
        self.log.append(entry)
        assert code is None or out.returncode == code, entry
        return out

    def task(self, cwd, *args, **kw):
        out = self.run(cwd, binary, *args, **kw)
        text = out.stdout if out.returncode == 0 else out.stderr
        return {"exit": out.returncode, **(json.loads(text) if text.strip() else {})}

    def git(self, cwd, *args):
        return self.run(cwd, "git", *args)

    def store(self, host):
        path = Path(self.hosts[host]["XDG_STATE_HOME"]) / "tasks/claims/pp.toml"
        return self.clean(path.read_text()) if path.exists() else None


def views(fx, cwd, tid, host="A"):
    prime = fx.task(cwd, "prime", host=host)
    parked = fx.task(cwd, "list", "--parked", host=host)
    parked_done = fx.task(cwd, "list", "--parked", "--status", "done", host=host)
    show = fx.task(cwd, "show", tid, host=host)
    return {
        "prime_parked_ids": [r.get("id") for r in prime.get("parked", [])],
        "list_parked_ids": [r.get("id") for r in parked.get("tasks", [])],
        "list_parked_status_done_ids": [r.get("id") for r in parked_done.get("tasks", [])],
        "show_status": show.get("task", {}).get("status"),
        "show_park": show.get("park"),
        "show_warnings": show.get("warnings", []),
        "prime_warnings": prime.get("warnings", []),
        "store": fx.store(host),
    }


def record_state(fx, path, tid, host="A"):
    show = fx.task(path.parent.parent, "show", tid, host=host)["task"]
    return {"bytes": path.read_text(), "sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
            "fields": {k: show.get(k) for k in ("status", "updated", "completed", "model", "last_done", "every")},
            "notes": [n["text"] for n in show.get("notes", [])]}


def attempt(fx, cwd, record, tid, args, host="A", session="probe-retry", extra=None):
    before = record_state(fx, record, tid, host)
    store_before = fx.store(host)
    result = fx.task(cwd, *args, host=host, session=session, extra=extra, code=None)
    after = record_state(fx, record, tid, host)
    return {
        "argv": ["tasks", *args], "session": session, "env": extra or {},
        "exit": result["exit"], "error": result.get("error"), "warnings": result.get("warnings", []),
        "record_unchanged": before["sha256"] == after["sha256"],
        "record_diff": [l for l in difflib.unified_diff(before["bytes"].splitlines(), after["bytes"].splitlines(), lineterm="", n=0)
                        if not l.startswith(("---", "+++", "@@"))],
        "fields_before": before["fields"], "fields_after": after["fields"],
        "notes_added": after["notes"][len(before["notes"]):] if after["notes"][:len(before["notes"])] == before["notes"] else "rewritten",
        "store_before": store_before, "store_after": fx.store(host),
    }


def build(scratch, arrival, every=False, closer_session="probe-park", close_with="done", a_claim=None):
    """A parks pp-T from worktree wt; B closes it; the closed record reaches A's checkout
    by `arrival`: "sync" (one shared checkout, as file sync shares it) or "git" (B's own
    clone, pushed, then pulled by A). `a_claim` replaces the park with a claim held by
    another session under A ("live" or "stale")."""
    fx = Fixture(scratch, arrival)
    origin = fx.root / "origin.git"
    fx.git(fx.root, "init", "-q", "--bare", "-b", "main", str(origin))
    a_main = fx.root / "a"
    fx.git(fx.root, "clone", "-q", str(origin), str(a_main))
    fx.task(a_main, "init", "--prefix", "pp")
    add = ["add", "T", "-p", "2", "--process", "direct"] + (["--every", "7d"] if every else [])
    tid = fx.task(a_main, *add)["id"]
    if every:
        # A recurring task completed once, so B's close below is a later occurrence.
        fx.task(a_main, "start", tid)
        fx.task(a_main, "done", tid, "first occurrence")
    fx.git(a_main, "add", "-A")
    fx.git(a_main, "commit", "-qm", "seed")
    fx.git(a_main, "push", "-q", "origin", "main")
    wt = a_main / "wt"
    fx.git(a_main, "worktree", "add", "-q", "-b", "work", str(wt))
    holder = None
    if a_claim is None:
        fx.task(wt, "start", tid, session="probe-park")
        fx.task(wt, "park", tid, "resume the probe", session="probe-park")
    else:
        holder = subprocess.Popen(["sleep", "600"])
        fx.task(wt, "start", tid, session="probe-holder", extra={"TASKS_SESSION_PID": str(holder.pid)})
        if a_claim == "stale":
            holder.kill()
            holder.wait()
            holder = None
    fx.git(wt, "commit", "-qam", "park in worktree")
    fx.git(a_main, "merge", "-q", "--ff-only", "work")
    fx.git(a_main, "push", "-q", "origin", "main")
    if arrival == "sync":
        b_main = a_main
    else:
        b_main = fx.root / "b"
        fx.git(fx.root, "clone", "-q", str(origin), str(b_main))
    fx.task(b_main, "init", "--prefix", "pp", "--force", host="B", code=None)
    close = ["done", tid] if close_with == "done" else ["drop", tid, "closed on host B"]
    fx.task(b_main, *close, host="B", session=closer_session, extra={"TASKS_MODEL": "model-b"})
    fx.git(b_main, "commit", "-qam", "close on B")
    if arrival == "git":
        fx.git(b_main, "push", "-q", "origin", "main")
        fx.git(a_main, "pull", "-q", "--ff-only", "origin", "main")
    record = a_main / "tasks" / f"{tid}.md"
    return fx, a_main, wt, tid, record, holder


def remove_worktree(fx, a_main, wt):
    fx.git(a_main, "worktree", "remove", str(wt))
    return fx.git(a_main, "worktree", "list", "--porcelain").stdout


result = {"binary": None, "git": None, "scenarios": []}


def scenario(name, **build_kw):
    def wrap(fn):
        with tempfile.TemporaryDirectory(prefix="tasks-park-probe-") as scratch:
            fx, a_main, wt, tid, record, holder = build(scratch, **build_kw)
            try:
                out = fn(fx, a_main, wt, tid, record)
            finally:
                if holder:
                    holder.kill()
                    holder.wait()
            if result["binary"] is None:
                result["binary"] = fx.run(a_main, binary, "--version").stdout.strip()
                result["git"] = fx.run(a_main, "git", "--version").stdout.strip()
            result["scenarios"].append({"name": name, "build": build_kw, **out, "log_commands": len(fx.log),
                                        "log": fx.log})
        return fn
    return wrap


for arrival in ("sync", "git"):
    for closer in ("probe-park", "probe-closer"):
        @scenario(f"{arrival}: closed by {'same' if closer == 'probe-park' else 'distinct'} session, bare retry after worktree removal",
                  arrival=arrival, closer_session=closer)
        def _(fx, a_main, wt, tid, record):
            before = views(fx, a_main, tid)
            removed = remove_worktree(fx, a_main, wt)
            after_removal = views(fx, a_main, tid)
            retry = attempt(fx, a_main, record, tid, ["done", tid])
            return {"views_before_removal": before, "worktrees_after_removal": fx.clean(removed),
                    "views_after_removal": after_removal, "retries": [retry],
                    "views_after_retry": views(fx, a_main, tid)}

for label, args, extra in [("explicit message", ["done", "{id}", "cleanup retry"], None),
                           ("changed TASKS_MODEL", ["done", "{id}"], {"TASKS_MODEL": "model-a-different"})]:
    @scenario(f"git: retry with {label}", arrival="git")
    def _(fx, a_main, wt, tid, record, args=args, extra=extra):
        remove_worktree(fx, a_main, wt)
        retry = attempt(fx, a_main, record, tid, [a.replace("{id}", tid) for a in args], extra=extra)
        return {"retries": [retry], "views_after_retry": views(fx, a_main, tid)}


@scenario("sync: dropped on B, retry drop on A", arrival="sync", close_with="drop")
def _(fx, a_main, wt, tid, record):
    before = views(fx, a_main, tid)
    remove_worktree(fx, a_main, wt)
    retries = [attempt(fx, a_main, record, tid, ["done", tid]),
               attempt(fx, a_main, record, tid, ["drop", tid, "cleanup retry"])]
    return {"views_before_removal": before, "retries": retries, "views_after_retry": views(fx, a_main, tid)}


@scenario("sync recurring: retry in main with the worktree present, then after removal", arrival="sync", every=True)
def _(fx, a_main, wt, tid, record):
    before = views(fx, a_main, tid)
    retries = [attempt(fx, a_main, record, tid, ["done", tid])]
    remove_worktree(fx, a_main, wt)
    retries.append(attempt(fx, a_main, record, tid, ["done", tid]))
    retries.append(attempt(fx, a_main, record, tid, ["park", tid, "clear"]))
    return {"views_before_removal": before, "retries": retries, "views_after_retry": views(fx, a_main, tid)}


@scenario("sync recurring: retry in the parked worktree after integrating main", arrival="sync", every=True)
def _(fx, a_main, wt, tid, record):
    fx.git(wt, "merge", "-q", "--ff-only", "main")
    wt_record = wt / "tasks" / f"{tid}.md"
    retries = [attempt(fx, wt, wt_record, tid, ["done", tid])]
    return {"retries": retries, "views_after_retry": views(fx, a_main, tid)}


@scenario("sync recurring: start then done from main after worktree removal", arrival="sync", every=True)
def _(fx, a_main, wt, tid, record):
    remove_worktree(fx, a_main, wt)
    retries = [attempt(fx, a_main, record, tid, ["start", tid]),
               attempt(fx, a_main, record, tid, ["done", tid])]
    return {"retries": retries, "views_after_retry": views(fx, a_main, tid)}


for state in ("live", "stale"):
    @scenario(f"sync: closed while another session under A holds a {state} claim", arrival="sync", a_claim=state)
    def _(fx, a_main, wt, tid, record, state=state):
        before = views(fx, a_main, tid)
        retries = [attempt(fx, a_main, record, tid, ["done", tid])]
        return {"views_before_retry": before, "retries": retries, "views_after_retry": views(fx, a_main, tid)}

text = json.dumps(result, indent=1)
text = re.sub(r"/[^\s\"']*/tasks-park-probe-[A-Za-z0-9_]+", "<fixture>", text)
text = re.sub(r'boot_id = \\"[0-9a-f-]+\\"', 'boot_id = \\"<boot>\\"', text)
sys.stdout.write(text.replace(os.uname().nodename, "<host>") + "\n")
