# Claim-guard Policy Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Read authoritative wake facts and remind Codex controllers and workers about the correct live claims, without granting foreign closure authority.

**Architecture:** Keep the executable and its tests in ops. Hq supplies validated facts and both native hook registrations. Tasks records the policy, coordination and acceptance; the guard stays read-only and refuses only the first stop.

**Tech Stack:** Existing Python standard library, unittest in ops, pytest through hq's Just gate, the installed tasks CLI, and normally trusted Codex command hooks. No new dependency or public CLI.

**Spec:** [Approved design](../specs/2026-10-08-claim-guard-policy-design.md), accepted at `3d627b3` by claude-code/opus-5.5. This plan is proposed and requires separate review.

**Execution method:** Inline with a fresh final reviewer, agreed in the plan review. Plan approval is still pending.

## Global Constraints

- "No event releases a claim."
- "Recovery stays explicit; no automatic takeover or worker release is introduced."
- "Root grouping does not identify an immediate parent."
- "Read claims before considering an exception: a known child cannot hide a failed inventory read."
- "Each subprocess retains the guard's ten-second timeout"; two successful slow reads can approach twenty seconds.
- "Edit facts in an isolated hq worktree and integrate the validated change; never edit the live file in place."
- "Do not change capability values during that interval" between mirror removal and the direct-reader cutover.
- "Activate worker wiring only after the new guard supports its event."
- "Any merge changing the live guard or home wiring remains an external-action gate."
- Require a bare native `session`, qualified `root_session`, and independent qualified relay/override controls. Native selection fixtures come from actual CLI output under synthetic exports; genuine delivery is separate evidence.
- Require root thread/session/hook-ID equality and a reset retry flag on the second turn before worker activation. A failed pilot revises the design; it does not add an ancestry or retry ledger.

## Review Focus

1. A tagged/display identity substituted for a stored native session must not make a worker invisible; Task 1 emits native fixture rows through `start`/`claims`, and Task 4 checks genuine delivered IDs.
2. A running Claude child must not hide a failed claims read, and a retry must work when both readers fail; Task 1 tests these orderings separately.
3. `unknown`, string booleans and malformed sets must never grant a wake exception; Task 1 tests the subprocess boundary and the real validated reader.
4. A root claim with root metadata must still receive own-claim guidance; a worker claim must receive completion-proven recovery rather than foreign `done`; Tasks 1, 4 and 5 cover both cases.
5. Duplicate hook registration, the work home, and a retry flag sticky across turns must not create false acceptance; Task 3 checks both files and Tasks 4–5 require actual events and a second worker turn.

## Files and execution setup

All paths below are relative to their named owner checkout. Show the user paths prefixed with the worktree directory under that checkout. Code stays in the existing files except hq's small wiring test. Pilot helpers are private, disposable evidence tooling under the tasks worktree's ignored `.superpowers/sdd/claim-guard-policy/`; they are not new product utilities.

| Owner | Files | Responsibility |
| --- | --- | --- |
| ops | `hooks/claim-guard`, `tests/test_claim_guard.py` | Reader, event selection, decision ordering, safe messages and regressions |
| ops | `README.md`, `docs/specs/2026-10-03-agent-layer-split-design.md` | Current guard ownership and a dated supersession note for the mirror |
| hq | `.githooks/pre-commit`, `.githooks/test_pre_commit_facts.py`, `justfile`, `README.md` | Remove the temporary consumer mirror; retain staged fact validation |
| hq | `codex/hooks.json`, `codex/hooks.work.json`, new `tools/test_claim_guard_wiring.py` | Native worker wiring in both declared homes |
| hq | `docs/specs/2026-10-06-residue-design.md` | Dated supersession note; retain historical findings |
| tasks | `docs/notes/2026-10-08-claim-guard-acceptance.md`, delegated-identity brief, task records | Verdicts, limits, review notes and closeout |

After plan approval, publish the reviewed docs and blocked step records to tasks main by a local merge before creating cross-project dependencies. Preserve the unrelated edits already present in main. Reuse this task's worktree for coordination; do not create another tasks workspace.

Start each plan child before its step. In each code owner, read `ops-profile session`, its guide and `tasks prime`; honor live claims and halts. File a scoped owner task with `--source <plan-child-id>`, and record its id on the plan child. No plan link in a foreign project: its task body points to this plan using the registered tasks root. Close the owner task with its code commit; close the plan child with the corresponding coordination record. The parent stays open through acceptance and rollout. Run `trial-arm` after every real task start.

Commit each owner task record before worktree creation. Use `work-link --ensure .worktrees`, `git worktree add` and lock the new tree. Hq runs `just setup` immediately; ops has no setup recipe and uses only its guide's hook setup. Baselines: ops `just test-fast`, hq `just test`, tasks `just test-fast`, before the first code change. If these fail, investigate before treating a later failure as a regression. Do not repoint any shared link or launcher.

The reviewed parent spec provides the design for every child (`process: direct`). Children remain blocked while this plan is in review. On acceptance, record the plan review, unblock them, and preserve dependencies. Inline execution is agreed because the tasks share the same guard interfaces and a strict cross-project rollout order; a fresh whole-branch reviewer still runs before activation.

Execute Tasks 1–3, then complete tasks-724c9e's direct enumeration probe before starting Task 4. Tasks 4–5 are bounded pilot slices, not a detached sweep: expect roughly two minutes of preparation and one to three minutes per model turn, with a ten-minute refusal/timeout limit on each slice. No idle desktop is required. A timeout stops the next slice, preserves the completed evidence and requires cleanup of the foreground harness before parking; it is never worker-completion proof for recovery.

---

### Task 1: Implement the ops guard reader and claim policy

**Files:** ops `hooks/claim-guard`, `tests/test_claim_guard.py`, `README.md`, `docs/specs/2026-10-03-agent-layer-split-design.md`.

**Interfaces:**

- Keep `ReadFailed`, `Unseen`, `HARNESSES`, `TIMEOUT_S = 10`, `BOUNDED_WAIT`, `OWNER_LINE`, `read_claims(run=subprocess.run)` and CLI/output conventions.
- Add `parse_facts(text, harness) -> dict[str, dict]` and `read_facts(harness, command=None, run=subprocess.run) -> dict[str, dict]`. Entries retain hq's `status`, `type` and native `value` representations; `unknown` has no synthesized value.
- Change `held(claims, harness, event) -> (own: list[dict], related: list[dict])` and `in_flight(event, kinds, statuses) -> list[dict]`.
- Change `decide(harness, text, read, facts) -> str | None`; both readers are zero-argument callables, invoked only when needed. `main` supplies `read_claims` and `lambda: read_facts(args.harness)`.
- Adapt `reason(harness, own, related, wake=None, unseen=None, worker=False) -> str`, `_options(harness, wake=None, unseen=None, worker=False) -> str`, `child_sentence(harness, wake, unseen)` and `wait_branch(harness, wake)` together. `wake` is exactly `True`, `False` or `None` for unprobed/unavailable evidence, never a truthiness conversion.

- [ ] **Step 1: Establish real native rows before testing selection.** Capture the installed tasks path at module import, before `Sandbox.setUp` replaces `PATH`. Native CLI tests use `unittest.TestCase` and an isolated subprocess environment, not `Sandbox`'s stub CLI. Set both `XDG_CONFIG_HOME` and `XDG_STATE_HOME` in a `TemporaryDirectory`; remove explicit/Claude identities, use relay-off empty config, and give every command a timeout. The primary rows must be emitted, not hand-written:

```python
import shutil

REAL_TASKS = shutil.which("tasks")
ROOT = "11111111-1111-4111-8111-111111111111"
WORKER = "22222222-2222-4222-8222-222222222222"

def native_rows(binary, repo, env):
    def run(args, actor=ROOT):
        command_env = dict(env, CODEX_SESSION_ID=ROOT, CODEX_THREAD_ID=actor)
        done = subprocess.run([binary, "--json", *args], cwd=repo,
                              env=command_env, capture_output=True, text=True,
                              timeout=10, check=True)
        return json.loads(done.stdout)
    run(["init", "--prefix", "cgprobe"])
    ids = []
    for title, actor in [("controller", ROOT), ("worker", WORKER)]:
        task_id = run(["add", title, "--process", "direct"])["id"]
        run(["start", task_id], actor)
        ids.append(task_id)
    rows = run(["claims", "--all-projects"])["claims"]
    controller = next(r for r in rows if r["id"] == ids[0])
    worker = next(r for r in rows if r["id"] == ids[1])
    assert controller["session"] == ROOT
    assert worker["session"] == WORKER
    assert controller["root_session"] == worker["root_session"] == f"codex:{ROOT}"
    assert all(r["live"] and "pid" not in r for r in (controller, worker))
    for task_id, actor in zip(ids, (ROOT, WORKER)):
        run(["done", task_id, "fixture capture complete"], actor)
    assert run(["claims", "--all-projects"])["claims"] == []
    return controller, worker
```

The test environment starts from `os.environ.copy()` but removes `TASKS_SESSION`, `TASKS_SESSION_PID`, `CLAUDE_CODE_SESSION_ID`, `CLAUDE_PID`, `CODEX_SESSION_ID` and `CODEX_THREAD_ID` before assigning synthetic native exports. The temporary directory owns cleanup even if an assertion fails. A missing real CLI fails this class rather than substituting the stub.

- [ ] **Step 2: Add RED tests for selection, ordering and safe recovery.** `stop()` now emits `hook_event_name: Stop`; create worker payloads with `SubagentStop`, root `session_id` and bare `agent_id`. Primary selection tests use Step 1's rows:

```python
event = {"hook_event_name": "SubagentStop", "session_id": ROOT,
         "agent_id": WORKER, "agent_type": ""}
self.assertEqual(guard.held([controller, worker], "codex", event), ([worker], []))
root_event = {"hook_event_name": "Stop", "session_id": ROOT}
self.assertEqual(guard.held([controller, worker], "codex", root_event),
                 ([controller], [worker]))
```

Add table controls for stale worker, different root, unrelated controller, missing root, root-associated sibling, and qualified `codex:<id>` identities. A worker selects itself without selecting siblings or descendants; a root selects its whole recorded tree. Add optional-root `None`, empty string, integer and valid-string parse cases: absent is accepted, malformed present values refuse. A Codex event must not match `claude-code:<id>` merely because the ID text agrees.

Replace the two read-skipping tests named in the spec. Pin the new ordering and immediate retry with this test shape:

```python
events = [stop(background_tasks=[child()]),
          json.dumps({"hook_event_name": "SubagentStop", "session_id": ROOT,
                      "agent_id": WORKER, "stop_hook_active": False})]
for payload in events:
    calls = []
    def broken_claims():
        calls.append("claims")
        raise guard.ReadFailed("inventory failed")
    def unused_facts():
        self.fail("facts read after inventory failure")
    result = guard.decide("codex" if "SubagentStop" in payload else "claude-code",
                          payload, broken_claims, unused_facts)
    self.assertIn("inventory failed", result)
    self.assertEqual(calls, ["claims"])
    retry = json.loads(payload)
    retry["stop_hook_active"] = True
    self.assertIsNone(guard.decide("codex", json.dumps(retry),
                                  unused_facts, unused_facts))
```

Test no-claim and worker branches with a facts reader that fails if invoked. Test a worker block and a root block containing only related claims: neither may offer direct `tasks done` on those foreign claims. Related messages must contain `start`, `--force`, `--reason`, worker-specific completion, no pending tool/check/background process, and warnings against timeout/silence/shared-PID evidence. Own root claims still get normal done/park guidance. Do not loosen ownership in tasks itself.

Run from the ops worktree:

```sh
just test-one tests.test_claim_guard.DecideTests
just test-one tests.test_claim_guard.NativeClaimTests
```

Expected RED: the new fourth reader/signature/selector is absent; record that exact failure, rather than an unrelated missing dependency. Implementers name the emitted-row class `NativeClaimTests`.

- [ ] **Step 3: Add the facts subprocess seam and its boundary tests.** Pure fixtures explicitly describe the three entries; they are policy controls, not native claim fixtures. For example:

```python
def facts(wake=True, kinds=None, statuses=None):
    return {
        "controller-wake": {"status": "probed", "type": "boolean", "value": wake},
        "controller-wake-kinds": {"status": "probed", "type": "set",
                                  "value": ["shell", "subagent"] if kinds is None else kinds},
        "stop-input-in-flight-statuses": {"status": "probed", "type": "set",
                                         "value": ["running", "pending"] if statuses is None else statuses},
    }

def fact_view(harness, entries):
    return {"schema": 1, "harnesses": [harness], "facts": {
        name: {"type": entry["type"], "harness": {
            harness: dict(entry, fact=name, harness=harness)
        }} for name, entry in entries.items()
    }}

def test_facts_command_is_injected(self):
    view = fact_view("claude-code", facts())
    selected = {name: value["harness"]["claude-code"]
                for name, value in view["facts"].items()}
    seen = []
    def run(argv, **kw):
        seen.append((argv, kw["timeout"]))
        return subprocess.CompletedProcess(argv, 0, json.dumps(view), "")
    command = ["/fixture/harness-facts", "list", "--file", "/fixture/facts.toml"]
    self.assertEqual(guard.read_facts("claude-code", command, run), selected)
    self.assertEqual(seen, [(command, 10)])
```

`fact_view` supplies the list-shaped JSON contract; production still reads hq's actual validator. Add subtests for invalid JSON, missing selected entry, mismatched fact type, `value: "false"`, integer boolean, malformed/non-string set member, nonzero exit with stderr, missing executable and `TimeoutExpired`. Each must raise `ReadFailed`. An `unknown` selected entry remains unknown, not false.

Implement the reader using the existing subprocess pattern, with a fixed production command and no flag/environment override:

```python
def read_facts(harness, command=None, run=subprocess.run):
    argv = command if command is not None else [str(pathlib.Path.home() / ".agents/bin/harness-facts"), "list"]
    try:
        done = run(argv, capture_output=True, text=True, timeout=TIMEOUT_S)
    except FileNotFoundError:
        raise ReadFailed("harness-facts is missing; repair hq's installed reader") from None
    except subprocess.TimeoutExpired:
        raise ReadFailed(f"harness-facts did not answer within {TIMEOUT_S} s") from None
    if done.returncode:
        raise ReadFailed(f"harness-facts exited {done.returncode}: {done.stderr.strip()}")
    return parse_facts(done.stdout, harness)
```

Implement the parser below. Unknown entries grant nothing; the original entries are returned without a synthesized value. `OSError` and unexpected reader failures are also caught by the total `decide` boundary and reported as failed reads.

```python
def parse_facts(text, harness):
    try:
        doc = json.loads(text)
    except (ValueError, TypeError, RecursionError) as error:
        raise ReadFailed(f"harness-facts printed invalid JSON: {error}") from None
    if not isinstance(doc, dict) or type(doc.get("schema")) is not int or doc["schema"] != 1:
        raise ReadFailed("harness-facts printed no schema-1 view")
    if not isinstance(doc.get("harnesses"), list) or harness not in doc["harnesses"]:
        raise ReadFailed(f"harness-facts does not declare {harness}")
    declared = doc.get("facts")
    if not isinstance(declared, dict):
        raise ReadFailed("harness-facts printed no facts mapping")
    selected = {}
    for name, kind in (("controller-wake", "boolean"),
                       ("controller-wake-kinds", "set"),
                       ("stop-input-in-flight-statuses", "set")):
        fact = declared.get(name)
        if not isinstance(fact, dict) or fact.get("type") != kind:
            raise ReadFailed(f"harness-facts printed an invalid {name} fact")
        entries = fact.get("harness")
        entry = entries.get(harness) if isinstance(entries, dict) else None
        if not isinstance(entry, dict) or entry.get("type") != kind or entry.get("status") not in ("probed", "unknown"):
            raise ReadFailed(f"harness-facts printed an invalid {name} entry for {harness}")
        if entry["status"] == "probed":
            value = entry.get("value")
            valid = type(value) is bool if kind == "boolean" else (
                isinstance(value, list) and all(isinstance(item, str) and item for item in value)
            )
            if not valid:
                raise ReadFailed(f"harness-facts printed an invalid {name} value for {harness}")
        selected[name] = entry
    return selected
```

- [ ] **Step 4: Implement the selectors and decision order.** Update every caller found by `rg 'guard\.(decide|reason|held|in_flight)|CHILD_WAKES|WAKING|IN_FLIGHT'`, including `EntryTests`. The selector's native spelling is explicit:

```python
def held(claims, harness, event):
    worker = event["hook_event_name"] == "SubagentStop"
    actor = event["agent_id"] if worker else event["session_id"]
    wanted = {actor, f"codex:{actor}"} if harness == "codex" else {
        actor, *(f"{name}:{actor}" for name in HARNESSES)
    }
    own = [r for r in claims if r["live"] and r["session"] in wanted]
    related = [] if worker or harness != "codex" else [
        r for r in claims if r["live"] and r["session"] not in wanted
        and r.get("root_session") == f"codex:{event['session_id']}"
    ]
    return own, related
```

In `decide`, retain its parse-failure diagnostic and immediate boolean retry check. Validate event name and session; require worker `agent_id`, accept empty/absent `agent_type`. Read/validate claims, select own/related, return `None` if both are empty, and return worker guidance without reading facts for `SubagentStop`. For a controller, read the three entries, derive wake from a probed boolean only, and call `in_flight(event, kinds, statuses)` only when wake is true and both sets are probed. `in_flight` keeps its current validation/diagnostics, replacing only its constant references with parameters. A qualifying child returns `None`; otherwise return the safe reason.

Unknown wake or sets gets an explicit unprobed diagnostic and no exception. False wake needs no status-set proof. Read failures get repair guidance, never a remembered value. `child_sentence` describes a recorded true fact only when wake is exactly true; `wait_branch` supplies bounded-wait guidance for false or unprobed wake. Worker options say finish or use the ordinary owned-task handoff; related-claim guidance remains separate from `_options`, which names actions only for the caller's own claims. Keep the existing detached-job warning and user-stop priority.

The related paragraph uses this text, with the selected ids/worktrees inserted:

```text
Related worker claims remain live: <ids and authoritative worktrees>. Inspect the worker's result and claim. Only after your own completion/wait result for that specific worker proves its assignment stopped with no pending tool, check or background process, recover there with `tasks start <id> --force --reason "<worker-specific completion evidence>"`, then review or close under your own claim. Timeout, silence, root state and a shared PID are not that evidence. If work is still running or uncertain, reconcile it and keep waiting. Do not resume the worker on that task without a new ownership handoff.
```

Extend `parse_claims` to reject a malformed present root and keep complete-inventory failure behavior. `main` calls the four-argument decision function; its crash retry path and appended ops owner line stay. Delete the three constants, `FactsMirrorTests`, its registry/TOML reader, `test_child_wakes_matches_the_probe`, and `test_waking_kinds_match_the_probe`. Adapt remaining reason/in-flight tests to injected values, retaining meaningful behavior and own-reason length checks.

- [ ] **Step 5: Verify through the real hq reader.** Add `RealFactsTests(unittest.TestCase)`. Locate hq with the real CLI captured before the sandbox; `tasks --json resolve hq` returns a result with `status: unregistered` when absent. Only that status skips. Nonzero exit, malformed output, a registered missing root/tool or reader failure fails. Do not parse the registry by hand.

Copy hq's fact file to a temporary file and invoke the real tool via `command=[str(hq_root / "tools/harness-facts"), "list", "--file", str(candidate)]`. Validate the fixture first. Exercise true, false, unknown, and changed sets by modifying the disposable copy, not hq main. Replace only the targeted harness-entry value; assert the replacement count before writing. Test malformed and truncated copies. An unknown entry is a complete entry with `status = "unknown"` and no probed keys; do not construct invalid unknown data by retaining `value/version/date/evidence`. Use the real reader's stdout as `parse_facts` input. Changed-kind and changed-status tests must show the decision changes without guard-code changes.

Update CLI entry tests to put a small facts executable at the fixture home's `.agents/bin/harness-facts` when a relevant controller claim needs facts. No-claim, retry and worker tests prove the tool is not invoked. Stubbed subprocess tests cover missing tool, invalid JSON, exit and timeout; real-tool tests cover its actual validator.

```sh
just test-one tests.test_claim_guard
just test-one tests.test_claim_guard.RealFactsTests
just test-fast
just check
```

Expected GREEN: all focused tests pass, and `RealFactsTests` executes with zero skips on the acceptance host. Record RED→GREEN for new policy and reader failures. Keep this code on the ops branch until Task 6; the live guard still uses main.

- [ ] **Step 6: Record the ownership contract and commit.** Ops README states that tasks owns the policy, ops continues to host the executable, hq supplies validated facts, and controller/worker native events are distinct. Add a dated note to the layer-split spec pointing to this accepted design; do not rewrite historical findings. Close the owner task in its code commit, and record its id, SHA and verification on this plan child. Commit only the named files and owner record, using `feat(hooks): read wake facts and guard native worker claims`. No push or live merge.

### Task 2: Remove hq's temporary mirror while retaining staged validation

**Files:** hq `.githooks/pre-commit`, `.githooks/test_pre_commit_facts.py`, `justfile`, `README.md`, `docs/specs/2026-10-06-residue-design.md`.

**Interfaces:** Keep `facts_blob()` and `facts_check() -> int`; success stays 0, invalid staged facts stays 1. Remove `ops_root`, `mirror`, mirror constants, environment overrides and the `facts-mirror` command/recipe. Hq's own `tools/harness-facts check --file <staged-blob>` remains the validator.

- [ ] **Step 1: Add a RED staged-validity regression independent of ops.** In `.githooks/test_pre_commit_facts.py`, retain the first four staged-file tests. Add:

```python
def test_a_valid_value_change_needs_no_consumer_mirror(repo, monkeypatch):
    good = (repo / "facts/capabilities.toml").read_text()
    stage(repo, good.replace("value = true", "value = false", 1))
    hook = load_hook()
    def forbidden(candidate):
        raise AssertionError("consumer mirror still ran")
    monkeypatch.setattr(hook, "mirror", forbidden, raising=False)
    assert hook.facts_check() == 0

def test_removed_mirror_command_is_refused(repo, capsys):
    assert load_hook().main(["facts-mirror"]) == 1
    assert "accepts no arguments" in capsys.readouterr().err
```

Run hq `just test`; expected RED is the forbidden mirror invocation. Retain the test where an invalid staged blob is rejected even when the working file is repaired.

- [ ] **Step 2: Delete only the mirror.** In `facts_check`, replace `return mirror(blob)` after successful validation with `return 0`; keep `finally: blob.unlink()`. Remove `MIRROR_TEST`, `MIRROR_TIMEOUT`, `ops_root`, `mirror`, and `main`'s `facts-mirror` branch. Remove now-unused `os`, `shutil`, `signal` and `tomllib` imports if no remaining reference uses them. Remove the mirror paragraph from the module docstring and the recipe from `justfile`.

Reject arguments to the now argument-free hook, so a direct invocation of the
removed mirror command cannot silently run an unrelated normal commit check:

```python
def main(argv):
    if argv:
        return fail("pre-commit accepts no arguments")
    return local_layer() or docs_check() or facts_check()
```

Delete the mirror-only test portion beginning at `STUB_JUSTFILE`, its fake ops fixture and all tests below it. Keep staged validity tests and the new regression. Remove the old `FACTS_MIRROR_OPS` cleanup from `repo` because the interface no longer exists.

- [ ] **Step 3: Update current docs and prove the retained gate.** README explains that consumers read hq's validated view directly, no consumer mirror runs, and facts are edited in a worktree. The residue spec gets a dated supersession note for section 4.7 referencing this design, retaining its historical text.

```sh
just test
rg -n 'FACTS_MIRROR_OPS|MIRROR_TEST|def mirror|def ops_root' .githooks/pre-commit justfile README.md
git diff --check
tasks check
```

Expected GREEN; the search finds no active mirror code/README instructions (historical specs/plans and the removed-command regression are excluded). `just --list` must also omit the `facts-mirror` recipe. No capability value change is part of this work. Close the owner task with the commit `refactor(hooks): remove capability facts consumer mirror`; record SHA and validation on this plan child. Do not activate worker wiring in this commit.

### Task 3: Prepare worker wiring in both Codex homes

**Files:** hq `codex/hooks.json`, `codex/hooks.work.json`, new `tools/test_claim_guard_wiring.py`, `README.md`.

**Interfaces:** Existing command `~/d/ops/hooks/claim-guard --harness codex`, registered once on `Stop` and once on `SubagentStop` in each file. `links.toml` keeps personal and work-home routes unchanged. This is a separate hq commit after Task 2, allowing mirror cleanup and wiring to activate at different points.

- [ ] **Step 1: Add a RED configuration test that covers both files.** Hq's `just test` already discovers `tools/test_*.py`; use standard library JSON/TOML parsing and pytest's existing parametrization:

```python
import json
import tomllib
from pathlib import Path
import pytest

ROOT = Path(__file__).resolve().parents[1]
COMMAND = "~/d/ops/hooks/claim-guard --harness codex"

@pytest.mark.parametrize("file,home", [("hooks.json", "codex"),
                                     ("hooks.work.json", "codex-work")])
def test_both_codex_homes_guard_root_and_worker(file, home):
    hooks = json.loads((ROOT / "codex" / file).read_text())["hooks"]
    for event in ("Stop", "SubagentStop"):
        commands = [h["command"] for group in hooks.get(event, [])
                    for h in group["hooks"] if h["type"] == "command"]
        assert commands.count(COMMAND) == 1
    others = [h["command"] for group in hooks["Stop"]
              for h in group["hooks"] if h["command"] != COMMAND]
    assert others == ["~/d/familiar/bin/familiar hook --agent codex Stop",
                      "~/.local/bin/harness-state-refresh"]
    manifest = tomllib.loads((ROOT / "links.toml").read_text())
    assert manifest["harness"][home]["links"]["hooks.json"] == f"codex/{file}"
```

Run hq `just test`; expected RED: no guard on `SubagentStop`. The added assertion protects familiar/state-refresh root commands. Before editing, retain the parsed baseline in the implementation's review evidence and compare every unrelated event/group in the final diff, so a worker registration cannot remove another hook. Do not stage that private baseline as a new configuration source.

- [ ] **Step 2: Add the same stanza to both files.** Use the existing command without a matcher; empty/default worker types must not be filtered:

```json
"SubagentStop": [
  { "hooks": [{ "type": "command", "command": "~/d/ops/hooks/claim-guard --harness codex" }] }
]
```

Keep each original `Stop` array and every unrelated event intact. README names both personal/work registrations and the normally trusted hook review required after a definition changes.

- [ ] **Step 3: Verify and commit the candidate without activation.** Run hq `just test`, `tasks check` and `git diff --check`; inspect both JSON diffs and declared homes. Close the owner task with `feat(hooks): guard Codex worker stops in both homes`. Record this wiring commit separately from Task 2's mirror-removal commit. Do not merge the wiring before the genuine pilot and Task 6's approval.

### Task 4: Establish pilot isolation, inventory and genuine delivery

**Files:** tasks `docs/notes/2026-10-08-claim-guard-acceptance.md`; private pilot workspace and wrapper under `.superpowers/sdd/claim-guard-policy/` (ignored, never staged).

**Interfaces:** Consume the explicit candidate guard path, installed tasks path, hq reader path and both candidate configs from Tasks 1–3. Produce a stage-A verdict containing versions, trusted hook sources, controller actual IDs and emitted claim, and one delivered worker event. Do not start the lifecycle/recovery stages until these are verified.

- [ ] **Step 1: Check the completed enumeration investigation.** The execution setup completes tasks-724c9e under its recorded direct process and trial arm before this dependent child starts. Its disposable main/worktree probe must show that `tasks claims` includes a claimed worktree-only record and a dead claim, while record listings have their documented limits. Use isolated config/state and fixture identities; no actual host claim takeover. Record the comparison, cleanup and verdict on tasks-724c9e and this parent. If inventory is insufficient, stop and revise the remedy before attempting a live pilot.

- [ ] **Step 2: Pin the private world inside the model's writable project.** The pilot git root is `.superpowers/sdd/claim-guard-policy/pilot/project` in the tasks worktree. All writable pilot data goes under that project's gitignored `.private/`: tasks config/state, Codex home, prompts, captures and `pilot.json`. Use real directories, not symlinks to outside state. The model's `-C` is always this root, including resumed runs, so `workspace-write` covers its own task mutations. No `--add-dir` or broader sandbox is needed. Refuse an existing pilot world until its previous evidence and cleanup status have been inspected; do not overwrite a prior auth copy or trust definitions.

The author's private launcher uses task-specific path variables and sets configuration-home variables only in its child-process environment, never as generic path holders in the parent shell. Keep normal HOME for the installed facts endpoint. Establish these paths before any model run:

```python
import os, shutil, subprocess
from pathlib import Path

CG_CODEX_BIN = shutil.which("codex")
assert CG_CODEX_BIN is not None, "installed Codex CLI is required"
CG_PROJECT = Path(".superpowers/sdd/claim-guard-policy/pilot/project").resolve()
CG_PROJECT.mkdir(parents=True, exist_ok=False)
CG_PRIVATE = CG_PROJECT / ".private"
CG_PRIVATE.mkdir(mode=0o700)
(CG_PROJECT / ".gitignore").write_text(".private/\n")
CG_CONFIG = CG_PRIVATE / "tasks-config"
CG_STATE = CG_PRIVATE / "tasks-state"
CG_PILOT_HOME = CG_PRIVATE / "codex-home"
for path in (CG_CONFIG, CG_STATE, CG_PILOT_HOME, CG_PRIVATE / "evidence"):
    path.mkdir(mode=0o700)
    assert path.resolve().is_relative_to(CG_PROJECT)
subprocess.run(["git", "init", "-q", str(CG_PROJECT)], check=True)
child_env = os.environ.copy()
for key in ("TASKS_SESSION", "TASKS_SESSION_PID", "CLAUDE_CODE_SESSION_ID",
            "CLAUDE_PID", "CODEX_SESSION_ID", "CODEX_THREAD_ID",
            "OPENAI_API_KEY", "CODEX_API_KEY", "CODEX_ACCESS_TOKEN"):
    child_env.pop(key, None)
child_env.update(XDG_CONFIG_HOME=str(CG_CONFIG), XDG_STATE_HOME=str(CG_STATE),
                 CODEX_HOME=str(CG_PILOT_HOME))
```

Authentication is an explicit copy of the **personal** `~/.codex/auth.json` into
the private Codex home, created exclusively with mode 0600. Never copy the work
home or a whole live config/trust store. If the personal file is absent or cannot
authenticate this private home, stop preparation and request the missing route;
do not silently switch accounts or create a login flow. Do not print or stage
credential contents. The child environment removes alternative credential
exports, so the copy is the named route rather than an unused file beside an
inherited account credential. Set `cli_auth_credentials_store = "file"` in the
private config so this route reads the private `auth.json`, rather than a host
keyring; the official configuration reference names that store explicitly. The
launcher prepares the copy and checks that route as follows:

```python
source_auth = Path.home() / ".codex/auth.json"
private_auth = CG_PILOT_HOME / "auth.json"
fd = os.open(private_auth, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
try:
    with os.fdopen(fd, "wb") as target, source_auth.open("rb") as source:
        shutil.copyfileobj(source, target)
    private_auth.chmod(0o600)
except BaseException:
    private_auth.unlink(missing_ok=True)
    raise
subprocess.run([CG_CODEX_BIN, "-c", 'cli_auth_credentials_store="file"',
                "login", "status"], env=child_env, check=True,
               capture_output=True, text=True, timeout=30)
```

`CG_CODEX_BIN` is the resolved installed binary recorded before preparation. The
author owns deleting `private_auth` on every preparation failure, abort and final
cleanup, after the foreground harness is stopped. Raw failure captures may be
retained for review, but the credential copy is never retained with them. The
personal source file is never changed. Before parking, verify that the private
credential has been removed and report any remaining capture directory and owner.

The pilot home's user config enables hooks and multi-agent support, selects the session's existing model/effort without guessing a new one, and has no inherited plugin/project hook source. The normal hook trust flow is required. Both candidate hook files are parsed, their registrations are checked with Task 3's test, and the pilot installs only the candidate guard's root/worker registrations plus a capture-only `SubagentStart` hook. Never execute unrelated work-home integrations in a personal scratch project. Record which candidate file's registration is being tested; repeat the smoke for the second file after the lifecycle gates pass.

The feature block is `[features]` with `hooks = true` and `multi_agent = true`; check `codex features list` in the isolated home before launch. Use only the actual current session's established model/effort values. If that routing or authentication cannot be established without guessing, park the preparation with the missing input; do not silently select another model or copy work-account state.

Initialize the disposable project through the explicit tasks binary under the isolated config/state. Add three unclaimed `direct` tasks: controller, normal worker and recovery control; record their returned ids in `.private/pilot.json`. The actual controller acquires the first, and the same actual worker acquires the other two. The author creates records but never impersonates native owners or starts these live-capture claims under synthetic exports. Prepare no-claim pre-gate prompts and separate stage-A/B/C/D/E prompt files containing their actual scratch ids and explicit paths.

The recorder captures genuine input and the candidate guard's exact result into private files, but only the guard's original stdout reaches Codex. Create this private `capture.py`; its two absolute arguments come from `pilot.json`:

```python
#!/usr/bin/env python3
import json, os, subprocess, sys, time
from pathlib import Path

guard, evidence = Path(sys.argv[1]), Path(sys.argv[2])
evidence.mkdir(parents=True, exist_ok=True)
record = {"event": None, "payload": "", "returncode": None,
          "stdout": "", "stderr": "", "error": None}
def as_text(value):
    return value.decode(errors="replace") if isinstance(value, bytes) else value or ""
try:
    payload = sys.stdin.read()
    record["payload"] = payload
    event = json.loads(payload)
    record["event"] = event
    if event["hook_event_name"] == "SubagentStart":
        record["returncode"] = 0
    else:
        run = subprocess.run([str(guard), "--harness", "codex"], input=payload,
                             text=True, capture_output=True, timeout=25)
        record.update(returncode=run.returncode, stdout=run.stdout, stderr=run.stderr)
        if run.returncode:
            record["error"] = f"candidate guard exited {run.returncode}"
except subprocess.TimeoutExpired as error:
    record.update(error="candidate guard timed out after 25 s",
                  stdout=as_text(error.stdout), stderr=as_text(error.stderr))
except Exception as error:
    record["error"] = f"capture failed: {type(error).__name__}: {error}"
finally:
    target = evidence / f"{time.time_ns()}-{os.getpid()}.json"
    target.write_text(json.dumps(record))
if record["error"]:
    sys.stderr.write(record["error"] + "\n")
    sys.exit(1)
sys.stdout.write(record["stdout"])
```

The wrapper never invents an event or edits `session_id`, `agent_id` or retry flags. Before a real harness, self-check capture-only start, a fake guard exiting nonzero, and an injected immediate `TimeoutExpired`: each must leave a record, and failures must exit nonzero. These are recorder controls, not delivery evidence. An unwritable evidence directory itself fails preparation and cannot be called a missing native hook. Use the same wrapper definition throughout both worker turns so a definition change cannot reset trust or state mid-pilot.

- [ ] **Step 3: Review/trust the exact pilot definitions and check startup.** Verify explicit executable resolution and record `codex --version` and `tasks --version`. Codex 0.161.0 was installed when planning; acceptance records whatever version actually executes. The [official hook trust flow](https://learn.chatgpt.com/docs/hooks#review-and-trust-hooks) uses `/hooks` and trusts exact definition hashes. Prepare the source/command/hash list for the user when trust is needed; pause only at that concrete host-use gate. Do not use `--dangerously-bypass-hook-trust`, fabricate managed trust or copy live trust tables wholesale. If the CLI's headless route cannot establish ordinary trust, park with the exact private-home review command and artifact rather than treating skipped hooks as a passing pilot.

Every noninteractive run uses `-a never -s workspace-write`; do not leave an
unattended exec waiting for escalation approval. The [official configuration
reference](https://learn.chatgpt.com/docs/config-file/config-reference) recommends
`never` for noninteractive runs. An out-of-sandbox command or denied escalation
is a preparation failure, not a reason to bypass the sandbox. Normal hook trust
is established separately before these runs; `-a never` does not grant hook trust.

Use a tracked foreground exec/PTY, `--no-daemon` and a ten-minute timeout; no
detached server, tmux or `nohup`. Do not use `--ephemeral`: the no-claim pre-gate
must test the actual persistence mechanism. All commands run through the launcher
with the prepared `child_env`; shell variables below name its paths and prompts,
not changes to the parent shell's home. Both initial and resumed commands set `-C`.

- [ ] **Step 4: Prove writable state with a no-claim smoke.** Before claiming any
pilot task, ask the model to write a sentinel in `.private/tasks-config` and
`.private/tasks-state`, then run real `tasks add`, `note` and `drop` on a separate
smoke record. Require successful commands and `tasks claims` returning an empty
inventory before and after. Capture the effective cwd, config/state paths and
successful write outcomes, never auth contents. No `tasks start`, escalation or
claim acquisition is part of this check. If it fails under `-a never`, stop here
and fix the layout/configuration; stage A has not started.

```sh
timeout --signal=INT --kill-after=10s 600 "$CG_CODEX_BIN" --no-daemon -C "$CG_PILOT_PROJECT" -a never -s workspace-write exec --json - < "$CG_WRITE_SMOKE_PROMPT"
```

- [ ] **Step 5: Probe worker restoration without claims.** In a fresh no-claim
root run, spawn one throwaway worker, ask it to return a checkpoint with no pending
work, wait for its result, record the actual root/worker ids and finish the run.
Check empty claims and actual `SubagentStart`/`SubagentStop` plus root `Stop`
captures. Resume that exact root in a new exec process and use `send_input` to
the recorded worker id, without spawning a replacement. Require a successful
tracked result and a new actual `SubagentStop` with the **same** `agent_id` and
distinct `turn_id`; model text claiming the worker survived is insufficient.
The no-claim root exports and `Stop.session_id` must also agree across the runs.

```sh
timeout --signal=INT --kill-after=10s 600 "$CG_CODEX_BIN" --no-daemon -C "$CG_PILOT_PROJECT" -a never -s workspace-write exec --json - < "$CG_RESUME_PROBE_1_PROMPT"
timeout --signal=INT --kill-after=10s 600 "$CG_CODEX_BIN" --no-daemon -C "$CG_PILOT_PROJECT" -a never -s workspace-write exec resume --json "$CG_PROBE_ROOT_SESSION" - < "$CG_RESUME_PROBE_2_PROMPT"
```

Write the transport verdict before live claims. A restored same-id worker selects
`execution_mode: resumed_processes` in private `pilot.json`. Missing worker state,
an unavailable id or missing same-id capture is a **transport/preparation** result,
not a retry-flag or guard failure. It selects the single-process fallback below
only if the independent write, authentication, trust, root identity and worker
delivery checks passed. An auth, permission, trust or delivery failure stops
preparation instead. Finish the throwaway worker assignment with no pending work;
it is not the held-claim pilot worker.

**Single-process fallback:** start one fresh held-claim pilot worker and run
stages A, B **and C** in the same foreground exec process. C needs the live worker
handle too. Do not end/restart the root between those stages. The root performs
the checks and waits at an author gate between stages; the author inspects actual
captures and inventory before granting the next gate. After C's specific worker
final completion, no further `send_input` is needed, so D and E may resume the
root independently. The preceding no-claim root `Stop` supplies the actual hook
ID used for the early equality check; require the held-claim root's actual exports
to remain identical, and recheck its actual `Stop` after C before recovery or
activation. If that equality cannot be established, stop rather than infer it.

Prepare a private bounded `await-author.py` for this mode:

```python
import json, sys, time
from pathlib import Path
gate, expected = Path(sys.argv[1]), sys.argv[2]
deadline = time.monotonic() + 180
while not gate.exists():
    if time.monotonic() >= deadline:
        raise SystemExit("author gate timed out; stop the pilot without changing claims")
    time.sleep(0.5)
if json.loads(gate.read_text()).get("verdict_id") != expected:
    raise SystemExit("author gate does not match the inspected stage")
```

Use a distinct gate and verdict id per stage, with no preexisting gate files. The
root launches the helper as a bounded foreground tool and waits through its
tracked tool handle; it never detaches it. The author keeps the outer exec tracked,
reads the stage captures and a read-only isolated `tasks claims` snapshot, then
publishes matching gate JSON by writing a sibling temporary file and atomically
replacing the gate only after the stage passes, so the helper cannot read a partial
acknowledgment. A timeout, denied gate
or failed check ends the slice and preserves evidence. There is no automatic
phase advance and no sleeping job left behind at turn end. This mechanism is a
private pilot harness, not a new guard API or persisted retry ledger.
Only the outer author publishes the acknowledgment; the pilot controller and
worker are explicitly forbidden to create or change their own author-gate file.

```python
temporary_gate = gate.with_suffix(".tmp")
temporary_gate.write_text(json.dumps({"verdict_id": inspected_verdict_id}))
temporary_gate.replace(gate)
```

Before acquiring any claims in fallback mode, smoke this exact in-process path
with a throwaway no-claim worker: obtain two separate same-id worker turns, write
one stage verdict, pause at the bounded author gate, let the author inspect and
acknowledge it, then continue and end cleanly. Require actual captures and empty
inventory. A denied/timed-out author gate or lost in-process worker is a preparation
failure and stops here. Do not discover a broken fallback after stage A acquired
live claims.

- [ ] **Step 6: Run and inspect held-claim stage A.** In resumed-process mode,
stage A may end and B/C may resume only after the restoration pre-gate passed. In
fallback mode, the root stays in the same exec through A/B/C and pauses at the
author gate after A instead of ending its process.

```sh
timeout --signal=INT --kill-after=10s 600 "$CG_CODEX_BIN" --no-daemon -C "$CG_PILOT_PROJECT" -a never -s workspace-write exec --json - < "$CG_STAGE_A_PROMPT"
```

The child environment and prompt paths are prepared in the private launcher; the command is run by explicit path if `PATH` resolves to a different binary than the recorded one. Never set fabricated native IDs for this live capture.

Stage-A controller prompt:

```text
This is an isolated claim-guard acceptance probe, not repository implementation. Use the explicit tasks binary and isolated project/state named in .private/pilot.json. Acquire the controller test task under the actual exported native IDs and retain it until stage E. Record only the two native IDs and your tasks claims row in private evidence. Spawn exactly one fresh pilot worker for the normal-worker and recovery-control tasks; it must use its actual exported IDs, acquire both tasks and return a checkpoint under deliberately retained claims. Record its native IDs and emitted claims rows. Do not take over any claim, change exports/hooks, or start recovery. Wait for the worker's tracked result. Tell the worker the retry rule below. In resumed-process mode report the concrete worker id/checkpoint and end this stage. In single-process mode run the stage-A checks, write its verdict, and wait at the stage-A author gate before sending this same worker its stage-B follow-up; do not end the root process.

CONTROLLED RETRY RULE FOR CONTROLLER AND WORKER: this pilot deliberately retains claims across checkpoints. If a stop hook blocks, reply with exactly one line, "Pilot checkpoint; claims retained; awaiting the next inspected stage.", then end that turn. Run no tool and do not park, done, start, force, refresh or otherwise change any claim in response to that block. The retry should be allowed. Recovery is forbidden before the separately authorized stage D; a guard reminder is not authorization to recover early. This rule is a test-only instruction, not a production handoff rule.
```

The captures must contain actual worker `SubagentStart`/`SubagentStop` records,
not only JSONL notifications. Compare root exports, the preceding no-claim root
`Stop.session_id`, controller bare claim and qualified root; require equality.
Compare worker `agent_id` with its actual thread export and bare claim session;
require a distinct worker with the same qualified root. In resumed-process mode,
also check stage A's actual root `Stop` and own/related messages now. In fallback
mode, check that actual held-claim root stop after C, before D; do not claim it
was delivered at an internal author gate.

Write a stage-A PASS or BLOCKED row before proceeding to B. Both worker claims
and the controller claim must still be live after the observed retry. Missing
hooks, skipped trust, identity mismatch or a model releasing/taking over a claim
stops here with its actual cause. A `SubagentStart` record is association evidence,
not permanent ownership or termination proof. Preserve completed-stage evidence
and the transport verdict; do not let a setup/protocol failure become a guard
failure or run the rest hoping it will repair the first stage.

### Task 5: Verify worker turns, release and completion-proven recovery

**Files:** tasks `docs/notes/2026-10-08-claim-guard-acceptance.md`, private pilot prompts and captures.

**Interfaces:** Consume stage-A's actual root/worker ids, retained normal/control
claims and the transport mode selected by the no-claim pre-gate. Produce separate
PASS/BLOCKED verdicts for first retry, second-turn reset, normal release and
completion-proven recovery. A/B are held-claim turns; C is handoff, D recovery,
E closeout. Keep raw ids/captures private. No stage uses `--last` or replaces the
held-claim worker. In fallback mode A/B/C share one foreground exec and author
gates; D/E need only the root and may resume after C. Keep that foreground run
tracked while moving from Task 4's stage-A verdict to Task 5; do not end the
author's turn with it still pending.

- [ ] **Step 1: Check the first turn's refusal, retry and retained claims.**
Inspect stage A's actual `SubagentStop` sequence: first flag not true with a block
naming only the worker's tasks, then flag true with an allowed result. Match actual
`agent_id`/`turn_id`, not file order. After the retry, read the isolated inventory
and require **both** normal-worker and recovery-control claims still live, owned
by the same recorded worker session and qualified root; also require the root's
controller claim unchanged. A parked, closed, refreshed or taken-over claim is a
prompt/protocol failure, not evidence about retry reset. Do not recreate it and
continue B. Record this separate retention verdict before the stage-B gate. If
retry/delivery/retention failed, stop and preserve evidence. Synthetic retry tests
are not proof of actual delivery.

- [ ] **Step 2: Ask the same worker for a second separate turn and verify reset.**
In resumed-process mode only, use the tested transport to resume the exact root:

```sh
timeout --signal=INT --kill-after=10s 600 "$CG_CODEX_BIN" --no-daemon -C "$CG_PILOT_PROJECT" -a never -s workspace-write exec resume --json "$CG_ROOT_SESSION" - < "$CG_STAGE_B_PROMPT"
```

In fallback mode, the still-running root consumes the prepared stage-B instruction
after the author opens its stage-A gate; it uses `send_input` on the same worker
inside that process. Do not run the resume command or end the root. A failure to
address the worker even in this mode is a transport failure, not a sticky-flag verdict.

Stage-B controller prompt:

```text
Continue only the isolated pilot. Send stage B to the exact held-claim worker id in .private/pilot.json; do not spawn a replacement. Ask for a second checkpoint with both existing worker claims retained and no pending tool/check/background work at its report. Keep the controller claim and wait for that worker's specific result. Give both actors the controlled retry rule: on a stop-hook block, reply only "Pilot checkpoint; claims retained; awaiting the next inspected stage." and end that turn with no tool or claim mutation. Do not park, done, start, force or refresh in response to the block. Do not recover before stage D. Record the actual worker id, result and inventory. In resumed-process mode end this stage after the report. In single-process mode write the inspected stage-B verdict and wait at its author gate before C; do not end the root process.
```

Require the same worker `agent_id`, a distinct `turn_id`, a reset first-stop flag
and new block, then allowed flag-true retry. Recheck both worker claims and the
root claim are still live with unchanged ownership and heartbeat timestamps.
Sticky flag is a guard/lifecycle gate failure; a lost/new worker or changed claim
is a transport/protocol failure. Neither permits activation or a retry ledger.
Record the verdict before opening C's gate.

- [ ] **Step 3: Verify ordinary release independently.** Follow up with the same worker to park its normal task with `--waiting-on agent`, verify its claim disappears from the isolated inventory, and return its worker-specific final result with no pending work. The controller then starts the released task, inspects results and closes its own acquired claim. Never force this normal path. Keep a separate intentionally unreleased control task for Step 4, identified in the worker's final result.

```sh
timeout --signal=INT --kill-after=10s 600 "$CG_CODEX_BIN" --no-daemon -C "$CG_PILOT_PROJECT" -a never -s workspace-write exec resume --json "$CG_ROOT_SESSION" - < "$CG_STAGE_C_PROMPT"
```

This command is only for proven resumed-process mode. In fallback mode the root
continues C in its existing exec after the author opens B's gate; the same worker
must complete handoff before that process is allowed to end.

```text
Controller: send this instruction to the exact existing held-claim worker, then receive its tracked final completion. Worker: park the normal task to the agent and verify its claim is absent. Report your final result with no pending tool, check or background process. Deliberately leave the separate recovery-control claim held and name it. Never release the controller's task. Controller: after receiving this specific final result, inspect the released task, start it normally and close it after review. Record the final result and inventory privately. If a stop hook blocks either actor, reply only "Pilot checkpoint; claims retained; awaiting the next inspected stage." and end with no further tool or claim mutation. Do not recover the control or close the controller task yet. Stage D alone authorizes recovery. In either transport mode the root may now end; no further worker input is required.
```

Gate: normal release/start/close succeeds, the worker's specific final result says
no pending work, and the control claim is still live. In fallback mode also check
the now-delivered held-claim root `Stop`, its actual ID against the pre-gate/root
exports, and its own/related message classification before D. Preserve the retry
that left the control/root claims unchanged. If the final result says work
continues or the root stop is missing/mismatched, stop; do not proceed to recovery.

- [ ] **Step 4: Run stage D's recovery inside its actual controller environment.**
Only after C's worker-specific tracked completion and the root-stop check pass,
launch the exact root with stage D. The author does **not** run the unforced close
before this command and does not impersonate the controller's native exports:

```sh
timeout --signal=INT --kill-after=10s 600 "$CG_CODEX_BIN" --no-daemon -C "$CG_PILOT_PROJECT" -a never -s workspace-write exec resume --json "$CG_ROOT_SESSION" - < "$CG_STAGE_D_PROMPT"
```

Stage-D prompt, executed by that controller:

```text
Inside your actual native environment, inspect the retained recovery-control claim and the specific worker final completion from C. Require that the assignment stopped with no pending tools/checks/background process. First attempt unforced done on the control task and require claimed, with unchanged record/store. If it unexpectedly succeeds, stop this stage and record failure. Only after that negative control passes, start the control with --force and the worker-specific completion reason, inspect/review it, then close under your acquired claim. Do not send further input to the worker. Keep your original controller task for E. A hook block is not new recovery authorization: reply only "Pilot checkpoint; claims retained; awaiting the next inspected stage." and end with no tool or claim mutation.
```

Within stage D, the controller's negative-control command is
`"$CG_TASKS_BIN" done "$CG_CONTROL_TASK" "Unforced recovery control"`. It requires
`claimed` and an unchanged record/store before the controller executes:

```sh
"$CG_TASKS_BIN" start "$CG_CONTROL_TASK" --force --reason "Pilot worker-specific final completion received; no pending tools, checks or background process; result and live claim inspected"
"$CG_TASKS_BIN" done "$CG_CONTROL_TASK" "Reviewed isolated recovery control"
```

These commands run inside the private controller's actual native environment, not the author's environment, and only target its disposable task. Verify the takeover note records the reason and that the claim is now acquired/closed by the controller. Do not revive the worker on that task afterward. Timeout, root state and shared PID are explicitly rejected controls in Task 1, not shortcuts here.

- [ ] **Step 5: Finish the root and repeat the configuration smoke for the other home.** Close the controller's own scratch task, require zero isolated live claims, and require an allowed actual root stop. Run a small fresh trusted delivery/identity smoke using the other candidate hook file's guard registrations; do not replay unrelated work-home hooks. Record both configuration-source verdicts. The full two-turn/recovery lifecycle needs one normally trusted worker execution; both candidate files also need the structural Task 3 check and genuine registration smoke.

```sh
timeout --signal=INT --kill-after=10s 600 "$CG_CODEX_BIN" --no-daemon -C "$CG_PILOT_PROJECT" -a never -s workspace-write exec resume --json "$CG_ROOT_SESSION" - < "$CG_STAGE_E_PROMPT"
```

Stage-E prompt: close only the controller's own remaining scratch task, verify all
three scratch tasks have no live claim, report the final inventory and stop. If a
hook blocks, use the same one-line controlled retry with no additional mutation,
and record it as an unexpected block. Require the actual **first** stop result to
be empty stdout; an allowed retry cannot turn an erroneous first block into PASS.
A failed close/residual claim blocks closeout and names the owner to reconcile.

Write the acceptance note as a table with stage, version/configuration, transport
mode, evidence, expected/observed result and verdict. Separate authentication,
writable-state, restoration/transport, prompt retention and guard/lifecycle
failures; synthetic fixtures are not genuine hooks. Include the known selection
limits. Cleanup on success or abort: stop/reap the tracked harness and any pending
author-gate helper, verify the scratch inventory, delete the private credential
copy first, then remove private config/state after retaining the sanitized
summary. If captures need review, retain only those with their path/cleanup owner
noted. On abort, unresolved scratch claims are reported as aborted fixture state,
not worker completion or authority to recover a real task. Run
`host-load --section session`; never commit credentials or raw private data.

### Task 6: Review, approve and activate the coordinated cutover

**Files:** all owner diffs, tasks acceptance note/delegated-identity brief and task records; hq active hook files and ops guard change only after approval.

**Interfaces:** Consume reviewed ops code, separate hq mirror/wiring commits, green owner gates, zero-skip real-reader checks and stage verdicts. Produce the live ownership/facts cutover, documented rollback and parent closeout. This task is the approval boundary for changes to the live global guard/settings.

- [ ] **Step 1: Run final verification and a fresh whole-branch review.** Ops runs `just test-one tests.test_claim_guard.RealFactsTests` with no skips, `just test-fast` and `just check`; hq runs `just test`; tasks runs `just test-fast` and `just check`. Review all relevant branches and their connected source, the accepted spec and the staged pilot verdicts. Record the review using the required `review: impl round ...` task note before acting on it. Fix or rule on Critical/Important findings in corrective rounds, up to five, each with RED→GREEN and scoped re-review. Record any Minor finding as `minor (deferred)` with a disposition; do not silently defer a failed delivery/reset gate.

- [ ] **Step 2: Prepare a concrete approval packet.** Show the three exact commits in activation order, both hook JSON diffs, the reader's resolved path, unchanged fact values and the passing pilot stages. Record current main SHAs for ops and hq, the former registrations in both homes, and whether any relevant main paths have unrelated edits. A conflicting draft or unavailable main requires resolution, not stashing without consent. Obtain approval for this packet before merging anything that changes the live guard or wiring; no push/PR is part of the default integration.

- [ ] **Step 3: Apply the approved order without changing facts.** First integrate only hq's mirror-removal commit from Task 2, leaving both live hook files at their old registrations. Next integrate the reviewed ops branch, so the existing live `Stop` command starts reading hq's validated view. Last integrate hq's worker-wiring commit, after the guard supports it and the lifecycle pilot passed. Review/trust both changed hook definitions through their ordinary native homes; hooks skipped pending trust are not activated. Hq's staged fact-validation gate remains present throughout.

Use owner-main branches, normal merges/cherry-picks of the recorded commits as appropriate, and normal hooks; no reset, force push, temporary host pointer or hook bypass. If capabilities drift during the interval, stop and reconcile; do not compensate with new constants.

- [ ] **Step 4: Check the activated state and retain a coordinated rollback.** Resolve `tasks` and the installed facts endpoint again. Verify both live files register the root and worker guard once and that native trust is established. Use the explicit-main guard on the retained isolated fixture controls, not actual private host task rows. Run each owner's gates where code/config changed; record that command checks do not replace genuine delivery evidence. Do not launch another desktop/live run without its required approval.

If activation fails, restore the recorded pre-cutover ops guard, both `Stop`-only native registration files, and both mirror halves together through reviewed reverts. Hq mirror code must never call an ops test that remains deleted. Do not remove unrelated hooks, modify facts in main or invent a partial rollback.

- [ ] **Step 5: Close records and clean only owned worktrees.** Update the delegated-identity brief to link accepted design/plan and the stage verdicts, naming outstanding limits. Close all plan children and owner pieces with their actual deliverables; close tasks-56b450 only after its required direct reader and mirror removal are landed and its delivery status is truthful. A delivery blocker is parked with the next concrete action rather than labeled verified. Any separately scoped follow-up goes to its owner and retains the evidence pointer.

Before removing a tree, run `tt-report`, inspect `git status --ignored --porcelain`, preserve needed ignored evidence, and carry every ledger's `Ruling:`/`minor (deferred)` disposition into the final report. Check home launchers, installed tools, systemd units and known includes for pointers into it. Unlock then remove only this task's trees, using the documented submodule checks if relevant. End with `host-load --section session`; report any intentionally retained process, pid and stop command, or say nothing remains running.

## Self-review and review gate

Coverage: ownership and spellings → Task 1; subprocess trust boundary and unknown semantics → Task 1; mirror deletion with retained staged validation → Task 2; both homes and unrelated registrations → Task 3; enumeration/root equality/trusted delivery → Task 4; retry lifetime/release/recovery → Task 5; migration order, external approval, rollback, documentation and closeout → Task 6. All five Review Focus cases have checks in those tasks.

The pilot's stages are sequential gates, each with a written verdict before the next starts. Writable-state and worker-transport checks have no claims; failed setup does not become a guard verdict. A failure preserves its evidence and stops dependent stages; Task 6 cannot turn a blocked pilot into a passing deployment. The spec remains approved; this plan remains unapproved until a separate review is recorded. Agreed execution: inline with a fresh final reviewer, followed by the live cutover approval packet.
