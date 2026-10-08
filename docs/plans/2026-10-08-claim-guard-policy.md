# Claim-guard Policy Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Read authoritative wake facts and remind Codex controllers and workers about the correct live claims, without granting foreign closure authority.

**Architecture:** Keep the executable and its tests in ops. Hq supplies validated facts and both native hook registrations. Tasks records the policy, coordination and acceptance; the guard stays read-only and refuses only the first stop.

**Tech Stack:** Existing Python standard library, unittest in ops, pytest through hq's Just gate, the installed tasks CLI, and normally trusted Codex command hooks. No new dependency or public CLI.

**Spec:** [Approved design](../specs/2026-10-08-claim-guard-policy-design.md), accepted at `3d627b3` by claude-code/opus-5.5. This plan is proposed and requires separate review.

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

The reviewed parent spec provides the design for every child (`process: direct`). Children remain blocked while this plan is in review. On acceptance, record the plan review, unblock them, and preserve dependencies. Inline execution is recommended because the tasks share the same guard interfaces and a strict cross-project rollout order; a fresh whole-branch reviewer still runs before activation.

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

- [ ] **Step 2: Prepare one private pilot world and its recorder.** Use a temporary git project, isolated tasks config/state, a private Codex configuration home and a known authenticated route. Store paths in a private `pilot.json` under the ignored workspace, not in committed text. Use task-specific path names such as `CG_PILOT_HOME`; a child-process environment may select that directory as Codex's documented configuration home, but do not repurpose the parent shell's home/config variables or change shared links. Keep normal HOME for the installed facts endpoint.

The pilot home's user config enables hooks and multi-agent support, selects the session's existing model/effort without guessing a new one, and has no inherited plugin/project hook source. The normal hook trust flow is required. Both candidate hook files are parsed, their registrations are checked with Task 3's test, and the pilot installs only the candidate guard's root/worker registrations plus a capture-only `SubagentStart` hook. Never execute unrelated work-home integrations in a personal scratch project. Record which candidate file's registration is being tested; repeat the smoke for the second file after the lifecycle gates pass.

The feature block is `[features]` with `hooks = true` and `multi_agent = true`; check `codex features list` in the isolated home before launch. Use only the actual current session's established model/effort values. If that routing or authentication cannot be established without guessing, park the preparation with the missing input; do not silently select another model or copy work-account state.

Initialize the disposable project through the explicit tasks binary under the isolated config/state. Add three unclaimed `direct` tasks: controller, normal worker and recovery control; record their returned ids in `pilot.json`. The actual controller acquires the first, and the same actual worker acquires the other two. The author creates records but never impersonates native owners or starts these live-capture claims under synthetic exports. Prepare separate stage-A/B/C/D/E prompt files containing their actual scratch ids and explicit paths.

The recorder captures genuine input and the candidate guard's exact result into private files, but only the guard's original stdout reaches Codex. Create this private `capture.py`; its two absolute arguments come from `pilot.json`:

```python
#!/usr/bin/env python3
import json, os, subprocess, sys, time
from pathlib import Path

guard, evidence = Path(sys.argv[1]), Path(sys.argv[2])
evidence.mkdir(parents=True, exist_ok=True)
payload = sys.stdin.read()
event = json.loads(payload)
name = event["hook_event_name"]
record = {"event": event}
if name == "SubagentStart":
    record.update(returncode=0, stdout="", stderr="")
else:
    run = subprocess.run([str(guard), "--harness", "codex"], input=payload,
                         text=True, capture_output=True, timeout=25)
    record.update(returncode=run.returncode, stdout=run.stdout, stderr=run.stderr)
    if run.returncode:
        raise RuntimeError(f"candidate guard failed: {run.stderr}")
target = evidence / f"{time.time_ns()}-{os.getpid()}.json"
target.write_text(json.dumps(record))
sys.stdout.write(record["stdout"])
```

The wrapper never invents an event or edits `session_id`, `agent_id` or retry flags. Assert its capture-only start mode prints nothing with one injected self-check before running a real harness; this check is not delivery evidence. Use the same wrapper definition throughout both worker turns so a definition change cannot reset trust or state mid-pilot.

- [ ] **Step 3: Review/trust the exact pilot definitions and check startup.** Verify explicit executable resolution and record `codex --version` and `tasks --version`. Codex 0.161.0 was installed when planning; acceptance records whatever version actually executes. The [official hook trust flow](https://learn.chatgpt.com/docs/hooks#review-and-trust-hooks) uses `/hooks` and trusts exact definition hashes. Prepare the source/command/hash list for the user when trust is needed; pause only at that concrete host-use gate. Do not use `--dangerously-bypass-hook-trust`, fabricate managed trust or copy live trust tables wholesale. If the CLI's headless route cannot establish ordinary trust, park with the exact private-home review command and artifact rather than treating skipped hooks as a passing pilot.

Run an initial foreground, bounded session in the private git project. The launcher supplies isolated tasks config/state, a clean parent native-ID environment and the private Codex home in its child environment. Use a tracked exec/PTY, `--no-daemon` and a ten-minute timeout; no detached server, tmux or `nohup`. Do not use `--ephemeral`: retaining the root/worker state is needed for the second worker turn.

```sh
timeout --signal=INT --kill-after=10s 600 codex --no-daemon -C "$CG_PILOT_PROJECT" -a on-request -s workspace-write exec --json - < "$CG_STAGE_A_PROMPT"
```

The child environment and prompt paths are prepared in the private launcher; the command is run by explicit path if `PATH` resolves to a different binary than the recorded one. Never set fabricated native IDs for this live capture.

Stage-A controller prompt:

```text
This is an isolated claim-guard acceptance probe, not repository implementation. Use the explicit tasks binary and isolated project/state named in pilot.json. Acquire the controller test task under the actual exported native IDs and retain it until later stages. Record only the two native IDs and your tasks claims row in private evidence. Spawn exactly one worker for the normal-worker and recovery-control tasks; it must use its actual exported IDs, acquire both tasks and return a checkpoint under this test's deliberately retained claims. Record the worker's native IDs and emitted claims rows. Do not take over any claim, change exports, change hooks, spawn more workers or start the recovery phase. Use the harness's tracked wait for that worker's result. Report the concrete worker id and checkpoint, then stop this stage.
```

- [ ] **Step 4: Inspect stage A before continuing.** The capture must contain actual controller `Stop` and worker `SubagentStart`/`SubagentStop` records, not only JSONL notifications. Compare root session, root thread, hook `session_id`, controller bare claim session and qualified root. Require equality. Compare worker `agent_id` with its actual thread export and bare claim session; require a distinct worker with the same qualified root. The guard's root own/related messages must classify each correctly.

Write a stage-A PASS or BLOCKED row before any retry. Missing hooks, skipped trust, missing worker `agent_id`, mismatched IDs or root equality failure stops here. Preserve completed-stage evidence and park a concrete next action; do not run the remainder hoping it will repair delivery. A `SubagentStart` record is association evidence, not a permanent ownership or termination signal.

### Task 5: Verify worker turns, release and completion-proven recovery

**Files:** tasks `docs/notes/2026-10-08-claim-guard-acceptance.md`, private pilot prompts and captures.

**Interfaces:** Consume stage-A's actual root and worker thread ids and the still-held normal/control claims. Produce separate PASS/BLOCKED verdicts for first retry, second-turn reset, normal release and unreleased-claim recovery. Keep all raw identifiers/captures private; publish a sanitized result table. Stages A and B are the two held-claim turns that test retry lifetime; C is a separate cleanup/handoff turn, D the controller recovery turn, and E root closeout. No stage uses `--last` or replaces the worker.

- [ ] **Step 1: Check the first turn's refusal and retry.** Inspect stage A's delivered `SubagentStop` sequence. With live worker claims, require a first event whose flag is not true and a block naming only that worker's tasks; require its subsequent flag-true event to allow. Neither message may instruct the worker to close the controller's claim. Match by actual `agent_id` and `turn_id`, not file order alone. If the continuation never arrives or the retry is not allowed, stop before the second-turn test. Do not treat synthetic `_is_retry` tests as proof.

- [ ] **Step 2: Ask the same worker for a second separate turn and verify reset.** Resume the exact root session, not `--last`, using the same private home/config and unchanged wrapper:

```sh
timeout --signal=INT --kill-after=10s 600 codex --no-daemon -a on-request -s workspace-write exec resume --json "$CG_ROOT_SESSION" - < "$CG_STAGE_B_PROMPT"
```

Stage-B controller prompt:

```text
Continue only the isolated pilot from the prior turn. Send a follow-up to the exact existing worker id recorded in pilot.json; do not spawn a replacement. Ask it for a second checkpoint while retaining its existing worker task claim and with no pending tool/check/background work at its report. Wait for its specific result using the harness's tracked wait. Keep the controller claim. Record the actual worker id and second-turn result privately. Do not release, take over or run the recovery stage yet.
```

Require the same worker `agent_id`, a distinct `turn_id`, a reset first-stop flag and a new block, followed by an allowed flag-true retry. A sticky flag or a new worker thread is a failed gate; do not activate wiring or add a retry ledger. Record the successful/failed stage before moving on.

- [ ] **Step 3: Verify ordinary release independently.** Follow up with the same worker to park its normal task with `--waiting-on agent`, verify its claim disappears from the isolated inventory, and return its worker-specific final result with no pending work. The controller then starts the released task, inspects results and closes its own acquired claim. Never force this normal path. Keep a separate intentionally unreleased control task for Step 4, identified in the worker's final result.

```sh
timeout --signal=INT --kill-after=10s 600 codex --no-daemon -a on-request -s workspace-write exec resume --json "$CG_ROOT_SESSION" - < "$CG_STAGE_C_PROMPT"
```

```text
Controller: send this instruction to the exact existing worker, then receive its tracked final completion. Worker: release the normal worker test task using its standard park-to-agent handoff, verify its claim is absent, and report your final result with no pending tool, check or background process. The separate recovery-control task is intentionally left claimed for the next test; name it explicitly. Do not release the controller's task. Controller: after receiving this specific final result, check the released task, start it normally and close it after review. Record the final result and inventory privately. No takeover is permitted yet.
```

Gate: normal release/start/close succeeds, foreign controller/worker closures remain refused, and the control claim is still live. If the final result says work continues, wait/reconcile; do not proceed to recovery.

- [ ] **Step 4: Verify unreleased-claim recovery after actual completion.** Only after Step 3's tracked result proves that specific worker's assignment stopped, run the controller's unforced `done` on the recovery-control task and require `claimed` with an unchanged record/store. Observe the root guard's related-claim paragraph and its explicit recovery conditions. Then run:

```sh
timeout --signal=INT --kill-after=10s 600 codex --no-daemon -a on-request -s workspace-write exec resume --json "$CG_ROOT_SESSION" - < "$CG_STAGE_D_PROMPT"
```

Stage-D prompt authorizes only the disposable controller to run the negative control, inspect its unchanged record/store, and then recover after confirming the exact worker completion evidence from stage C. It forbids another follow-up to that worker. The controller executes:

```sh
"$CG_TASKS_BIN" start "$CG_CONTROL_TASK" --force --reason "Pilot worker-specific final completion received; no pending tools, checks or background process; result and live claim inspected"
"$CG_TASKS_BIN" done "$CG_CONTROL_TASK" "Reviewed isolated recovery control"
```

These commands run inside the private controller's actual native environment, not the author's environment, and only target its disposable task. Verify the takeover note records the reason and that the claim is now acquired/closed by the controller. Do not revive the worker on that task afterward. Timeout, root state and shared PID are explicitly rejected controls in Task 1, not shortcuts here.

- [ ] **Step 5: Finish the root and repeat the configuration smoke for the other home.** Close the controller's own scratch task, require zero isolated live claims, and require an allowed actual root stop. Run a small fresh trusted delivery/identity smoke using the other candidate hook file's guard registrations; do not replay unrelated work-home hooks. Record both configuration-source verdicts. The full two-turn/recovery lifecycle needs one normally trusted worker execution; both candidate files also need the structural Task 3 check and genuine registration smoke.

```sh
timeout --signal=INT --kill-after=10s 600 codex --no-daemon -a on-request -s workspace-write exec resume --json "$CG_ROOT_SESSION" - < "$CG_STAGE_E_PROMPT"
```

Stage-E prompt: close only the controller's own remaining scratch task, verify all three scratch tasks have no live claim, report the final inventory and stop. Require that actual stop's recorder result to be empty stdout. A failed close or residual claim blocks closeout and names the owner to reconcile; it never silently deletes live host state.

Write the acceptance note as a table with stage, version/configuration, evidence, expected result, observed result and verdict. Clearly separate emitted synthetic fixture rows, injected controls and genuine hooks. Include missing-root/intermediate-worker/explicit-identity limitations. Cleanup: stop the tracked harness, verify zero claims, remove private auth/config/state after retaining the sanitized summary, and run `host-load --section session`. Private captures that need review are kept with their location and cleanup owner in the task note; never commit credentials or raw private project data.

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

The pilot's stages are sequential gates, each with a written verdict before the next starts. A failure preserves its evidence and stops its dependent stages; Task 6 cannot turn a blocked pilot into a passing deployment. The spec remains approved; this plan remains unapproved until a separate review is recorded. Recommended execution: inline with a fresh final reviewer, followed by the live cutover approval packet.
