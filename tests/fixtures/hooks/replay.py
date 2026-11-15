"""Portable paired replay; every command uses owned state and a ten-second deadline."""
import hashlib
import json
import os
import pathlib
import signal
import shutil
import subprocess
import sys
import tempfile
import time

ROOT = pathlib.Path(__file__).resolve().parent
TIMEOUT = 10


def integrity(root=ROOT):
    manifest = json.loads((root / "sources/manifest.json").read_text())
    assert len(manifest) == 16, "frozen import closure changed"
    for item in manifest:
        path = root / "sources" / item["repository"] / item["path"]
        assert hashlib.sha256(path.read_bytes()).hexdigest() == item["sha256"], f"frozen source mismatch: {item['path']}"
    return manifest


def run(argv, text, env, cwd):
    start = time.monotonic()
    try:
        child = subprocess.Popen(argv, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                 stderr=subprocess.PIPE, text=True, env=env, cwd=cwd,
                                 start_new_session=True)
    except FileNotFoundError as error:
        raise AssertionError(f"missing replay prerequisite {argv[0]}; install Python 3 / Node >=22") from error
    try:
        out, err = child.communicate(text, timeout=TIMEOUT)
    except subprocess.TimeoutExpired:
        os.killpg(child.pid, signal.SIGKILL)
        child.communicate()
        raise AssertionError(f"replay stage exceeded {TIMEOUT}s: {argv[0]}") from None
    return dict(exit=child.returncode, stdout=out, stderr=err,
                seconds=round(time.monotonic() - start, 6))


def environment(home, private_bin):
    return {"PATH": str(private_bin) + os.pathsep + os.environ["PATH"], "HOME": str(home),
            "XDG_CONFIG_HOME": str(home / ".config"), "XDG_STATE_HOME": str(home / ".local/state"),
            "USER": "tester", "LC_ALL": "C", "GIT_CONFIG_GLOBAL": "/dev/null", "GIT_CONFIG_NOSYSTEM": "1",
            "GIT_AUTHOR_NAME": "t", "GIT_AUTHOR_EMAIL": "t@example.invalid",
            "GIT_COMMITTER_NAME": "t", "GIT_COMMITTER_EMAIL": "t@example.invalid",
            "GIT_CEILING_DIRECTORIES": str(home.parent),
            "PYTHONDONTWRITEBYTECODE": "1",
            "TASKS_PALETTE": os.environ["TASKS_PALETTE"]}


def claim(path, session, *, root=None, stale=False, unverifiable=False, ident="sci-000001"):
    path.parent.mkdir(parents=True, exist_ok=True)
    stat = pathlib.Path("/proc/self/stat").read_text().rsplit(") ", 1)[1].split()
    ns = pathlib.Path("/proc/self/ns/pid").stat()
    values = dict(owner="fixture", session=session, host="fixture", worktree="/fixture",
                  pid=0 if stale else os.getpid(), pid_start=1 if stale else int(stat[19]),
                  boot_id="stale-boot" if stale else pathlib.Path("/proc/sys/kernel/random/boot_id").read_text().strip(),
                  pid_view="foreign:fixture" if unverifiable else f"{ns.st_dev}:{ns.st_ino}",
                  started="2026-11-15T00:00:00Z", seen="2026-11-15T00:00:00Z")
    if root:
        values["root_session"] = root
    with path.open("a") as stream:
        stream.write(f'[claims."{ident}"]\n')
        for key, value in values.items():
            stream.write(f"{key} = {json.dumps(value)}\n")


def facts(home, case):
    path = home / ".agents/bin/harness-facts"
    mode = case.get("facts")
    if mode == "missing":
        return
    entries = {}
    for name, kind, value in [("controller-wake", "boolean", case.get("wake")),
                              ("controller-wake-kinds", "set", case.get("kinds")),
                              ("stop-input-in-flight-statuses", "set", case.get("statuses"))]:
        entries[name] = {"type": kind, "harness": {case["harness"]: {
            "type": kind, "status": "unknown" if value is None else "probed", "value": value}}}
    view = {"schema": 1, "harnesses": [case["harness"]], "facts": entries}
    path.parent.mkdir(parents=True, exist_ok=True)
    body = "raise SystemExit(7)" if mode == "nonzero" else "print('invalid')" if mode == "invalid" else "print(" + repr(json.dumps(view)) + ")"
    path.write_text("#!/usr/bin/env python3\n" + body + "\n")
    path.chmod(0o755)


def git(env, directory, *args):
    result = run(["git", "-C", str(directory), *args], "", env, directory)
    assert result["exit"] == 0, f"fixture git: {result}"
    return result["stdout"].strip()


def repository(env, directory):
    directory.mkdir(exist_ok=True)
    git(env, directory, "init", "-q", "-b", "main")
    assert pathlib.Path(git(env, directory, "rev-parse", "--show-toplevel")) == directory
    assert pathlib.Path(git(env, directory, "rev-parse", "--absolute-git-dir")).is_relative_to(directory)
    (directory / "README").write_text("fixture\n")
    git(env, directory, "add", "-A")
    git(env, directory, "commit", "-qm", "fixture")


def project_fixture(binary, case, env, base, cwd, home):
    notice = case.get("notice")
    tree_root = cwd
    if notice == "nested-without-config":
        repository(env, tree_root)
        cwd = cwd / "nested"
        cwd.mkdir()
    detached = notice in ("detached", "historical", "historical-mismatch", "pairing-mismatch")
    linked = notice == "linked-worktree"
    if detached:
        repository(env, cwd)
        records = base / "records"
        repository(env, records)
        init = run([binary, "init", "--prefix", "sci", "--records", str(records)], "", env, cwd)
        authority = records / "sci"
    else:
        init = run([binary, "init", "--prefix", "sci"], "", env, cwd)
        authority = cwd
    assert init["exit"] == 0, f"fixture init: {init}"
    if notice in ("halt", "bad-scan", "detached", "historical", "historical-mismatch", "pairing-mismatch",
                  "linked-worktree", "in-tree-worktree", "in-tree-without-config", "nested-without-config", "non-directory"):
        added = run([binary, "add", case.get("title", "Stop work"), "--tag", "halt"], "", env, authority)
        assert added["exit"] == 0, f"fixture halt: {added}"
        case["fixture_halt_id"] = json.loads(added["stdout"])["id"]
        halt_path = authority / "tasks" / (case["fixture_halt_id"] + ".md")
        if notice == "bad-scan":
            halt_path.write_text("corrupt record")
    if linked:
        checkout = base / "linked-code"
        repository(env, checkout)
        with (authority / "tasks/.config.toml").open("a") as stream:
            stream.write("\n[checkout]\npath=" + json.dumps(str(checkout)) + "\n")
        cwd = checkout
    if notice in ("in-tree-worktree", "in-tree-without-config", "unregistered-git"):
        repository(env, cwd)
    if notice == "nested-without-config":
        git(env, tree_root, "add", "-A")
        git(env, tree_root, "commit", "-qm", "records")
    if notice in ("historical", "linked-worktree", "in-tree-worktree", "in-tree-without-config", "nested-without-config"):
        worktree = base / "worktree"
        checkout = tree_root if notice == "nested-without-config" else cwd
        git(env, checkout, "worktree", "add", "-q", "-b", "side", str(worktree))
        assert pathlib.Path(git(env, worktree, "rev-parse", "--show-toplevel")) == worktree
        assert pathlib.Path(git(env, worktree, "rev-parse", "--absolute-git-dir")).is_relative_to(checkout)
        if notice == "historical":
            (worktree / "tasks").mkdir()
            (worktree / "tasks/.config.toml").write_text("prefix='sci'\n")
            git(env, worktree, "add", "tasks")
            git(env, worktree, "commit", "-qm", "historical records")
        if notice == "in-tree-worktree":
            (worktree / "tasks" / halt_path.name).unlink()
        cwd = worktree / "nested" if notice == "nested-without-config" else worktree
        if notice in ("in-tree-without-config", "nested-without-config"):
            shutil.rmtree(cwd / "tasks")
    if notice in ("historical-mismatch", "pairing-mismatch"):
        foreign = base / "foreign"
        repository(env, foreign)
        if notice == "historical-mismatch":
            (foreign / "tasks").mkdir()
            (foreign / "tasks/.config.toml").write_text("prefix='sci'\n")
            cwd = foreign
        else:
            (authority / "tasks/.config.toml").write_text("prefix='sci'\n[checkout]\nkind='detached'\npath=" + json.dumps(str(foreign)) + "\n")
    if notice in ("unregistered", "unregistered-git"):
        (home / ".config/tasks/projects.toml").write_text("[projects]\n")
    elif notice in ("corrupt-config", "missing-prefix"):
        (cwd / "tasks/.config.toml").write_text("broken [toml" if notice == "corrupt-config" else "[git]\ncommit=false\n")
    elif notice == "non-directory":
        cwd = cwd / "README"
        cwd.write_text("not a directory")
    elif notice in ("missing-cwd", "no-project"):
        cwd = base / "other"
        if notice == "no-project":
            cwd.mkdir()
    return cwd


def replay(case, binary, identities):
    with tempfile.TemporaryDirectory(prefix="tasks-hook-replay-") as tmp:
        base = pathlib.Path(tmp).resolve()
        home, cwd, private_bin = base / "home", base / "code", base / "bin"
        for path in (home, cwd, private_bin):
            path.mkdir()
        (private_bin / "tasks").symlink_to(binary)
        env = environment(home, private_bin)
        notice = case.get("notice")
        cwd = project_fixture(binary, case, env, base, cwd, home)
        store = home / ".local/state/tasks/claims/sci.toml"
        mode = case.get("claims", "none")
        if mode not in ("none", "related", "worker"):
            cross = "claude-code:actor" if case["harness"] == "codex" else "codex:actor"
            session = {"foreign": "foreign", "raw": "actor", "cross": cross}.get(mode, case["harness"] + ":actor")
            claim(store, session, root="codex:actor" if mode == "own-root" else None,
                  stale=mode == "stale", unverifiable=mode == "unverifiable")
            if mode == "multiple":
                claim(store, session, ident="sci-000002")
        if mode in ("worker", "related", "own-related"):
            claim(store, "codex:worker", root="codex:actor", ident="sci-000002")
        if case.get("broken_store"):
            store.write_text("broken [toml")
        if case.get("broken_registry"):
            (home / ".config/tasks/projects.toml").write_text("broken [toml")
        facts(home, case)
        native = case["native"].replace("$CWD", json.dumps(str(cwd))[1:-1])
        role = "notice" if notice else "guard"
        decoded = run(["node", str(ROOT / "decode.mjs"), case["harness"], case["event"], str(cwd), role], native, env, base)
        if case.get("decoder_reject"):
            assert decoded["exit"] != 0 and not decoded["stdout"], f"decoder accepted {case['id']}"
            return dict(id=case["id"], classification="decoder-rejection", stage="decoder-rejected",
                        route="frozen-relay-projection", native_text=native, decoder=decoded,
                        subscriber_invoked=False, source_identities=identities, deadline_seconds=TIMEOUT)
        assert decoded["exit"] == 0, f"decoder stage {case['id']}: {decoded}"
        envelope = json.loads(decoded["stdout"])
        oracle_args = [sys.executable, str(ROOT / "oracle.py"), role, str(cwd) if notice else case["harness"]]
        original = run(oracle_args, native, env, base)
        assert original["exit"] == 0, f"native oracle stage {case['id']}: {original}"
        current = run([binary, "hook", "halt-notice" if notice else "claim-guard"], decoded["stdout"], env, base)
        old = json.loads(original["stdout"]) if original["stdout"].strip() else {}
        new = json.loads(current["stdout"]) if current["stdout"].strip() else {}
        old_decision = ("context" if old else "empty") if notice else ("deny" if old else "allow")
        new_decision = ("error" if current["exit"] else "context" if new else "empty") if notice else new.get("decision")
        row = dict(id=case["id"], route="frozen-relay-projection", classification=case["classification"],
                   native_text=native, envelope=envelope, native_decision=old_decision,
                   subscriber_decision=new_decision, native=original, subscriber=current,
                   decoder=decoded, source_identities=identities, deadline_seconds=TIMEOUT)
        if case.get("registered_halt"):
            assert case["fixture_halt_id"] in new.get("context", ""), json.dumps(row)
            row["fixture_halt_id"] = case["fixture_halt_id"]
        row["evidence_kind"] = "failure-path" if case.get("facts") or case.get("broken_store") else "policy-replay"
        if case["harness"] == "agy" and not notice:
            displaced = run([sys.executable, str(ROOT / "oracle.py"), "hq-evidence"], native, env, base)
            assert displaced["exit"] == 0, f"displaced policy stage: {displaced}"
            row["displaced_hq"] = json.loads(displaced["stdout"])
            if "displaced_expected" in case:
                assert row["displaced_hq"]["decision"] == case["displaced_expected"], json.dumps(row)
        assert [old_decision, new_decision] == case["expected"], json.dumps(row)
        if not notice:
            assert current["exit"] == 0 and not current["stderr"], json.dumps(row)
            assert set(new) == ({"decision", "reason"} if new_decision == "deny" else {"decision"}), json.dumps(row)
            if new_decision == "deny":
                assert 0 < len(new["reason"].encode("utf-16-le")) // 2 <= 10000, json.dumps(row)
        if case["classification"] == "exact":
            expected = {key: value.replace("owner: ops;", "owner: tasks;") if isinstance(value, str) else value for key, value in old.items()}
            if notice:
                assert new == expected, json.dumps(row)
            elif old_decision == "deny":
                assert new["reason"] == expected["reason"], json.dumps(row)
        if "contains" in case:
            assert case["contains"] in new.get("reason", ""), json.dumps(row)
        if "context_contains" in case:
            assert case["context_contains"] in new.get("context", ""), json.dumps(row)
        if case["classification"] == "halt-presentation":
            assert new["context"] == old["context"].replace(case["title"], case["rendered_title"], 1), json.dumps(row)
        if case["classification"] == "private-uncertainty" and case.get("claims") != "none":
            assert "child and wake state are unknown" in new["reason"], json.dumps(row)
            for action in ("(1) Continue", "tasks park", "tasks done", "Do not end the turn"):
                assert action in new["reason"], json.dumps(row)
        if case["classification"] == "read-diagnostic":
            # The Rust direct-store boundary cannot emit Python CLI/OS exception text.
            # Compare the complete recovery tail, keeping every action exact.
            anchor = "If the user told you this turn"
            assert new["reason"].split(anchor, 1)[1] == old["reason"].replace("owner: ops;", "owner: tasks;").split(anchor, 1)[1], json.dumps(row)
        return row


def main():
    identities = integrity()
    if sys.argv[1] == "integrity":
        coverage = json.loads((ROOT / "coverage.json").read_text())
        names = {case["id"] for case in json.loads((ROOT / "cases.json").read_text())}
        assert len(coverage["mapping"]) == 107
        assert len({row["test"] for row in coverage["mapping"]}) == 107
        for row in coverage["mapping"]:
            assert row["cases"] or row["boundary"], row["test"]
            assert set(row["cases"]) <= names, row["test"]
        with tempfile.TemporaryDirectory() as tmp:
            scratch = pathlib.Path(tmp)
            shutil.copytree(ROOT / "sources", scratch / "sources")
            target = scratch / "sources/relay/src/adapters/hook-event.js"
            target.write_bytes(target.read_bytes() + b"\n// changed\n")
            try:
                integrity(scratch)
            except AssertionError:
                pass
            else:
                raise AssertionError("source integrity accepted changed projection bytes")
        print(json.dumps({"verified_sources": len(identities)}))
        return
    binary, selection = str(pathlib.Path(sys.argv[1]).resolve()), sys.argv[2]
    version = run(["node", "--version"], "", {"PATH": os.environ["PATH"], "HOME": os.environ["HOME"]}, ROOT)
    assert version["exit"] == 0 and int(version["stdout"].strip().lstrip("v").split(".")[0]) >= 22, "hook replay requires Node >=22"
    cases = json.loads((ROOT / "cases.json").read_text())
    assert len({case["id"] for case in cases}) == len(cases), "duplicate replay case id"
    if selection == "smallest":
        names = {"own-live", "retry", "shell-running", "halt-codex", "agy-count-decimal-one"}
        cases = [c for c in cases if c["id"] in names]
    elif selection != "corpus":
        cases = [c for c in cases if c["id"] == selection]
    assert cases, f"unknown case {selection}"
    digest = hashlib.sha256(pathlib.Path(binary).read_bytes()).hexdigest()
    rows = []
    evidence = os.environ.get("TASKS_HOOK_REPLAY_EVIDENCE")
    journal = pathlib.Path(evidence) / (selection + ".jsonl") if evidence else None
    if journal:
        journal.write_text("")
    for case in cases:
        row = replay(case, binary, identities)
        row["implementation_binary_sha256"] = digest
        row["verdict"] = "pass"
        rows.append(row)
        if journal:
            with journal.open("a") as stream:
                stream.write(json.dumps(row) + "\n")
    print(json.dumps({"rows": rows, "count": len(rows), "python": sys.version.split()[0], "node": version["stdout"].strip()}))


if __name__ == "__main__":
    main()
