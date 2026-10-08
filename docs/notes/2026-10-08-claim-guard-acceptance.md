# Claim-guard acceptance — 2026-10-08

Status: candidate implementation verified; private native trust, no-claim
pre-gates and held-claim Stage A passed. Lifecycle stages are in progress.
No ops/hq live activation has occurred; cutover remains separately gated.

## Candidate verification

| Candidate | Revision | Verification |
| --- | --- | --- |
| ops reader and policy | `91984f5` | 87 focused tests; 4 real hq-reader tests without skips; 1,276 fast tests with one existing skip; hygiene passes |
| hq mirror removal | `a3662f6`, metadata correction `c9c787b` | 226 agent and 551 hook/tool tests; staged validation preserved; independent flow review accepts |
| Both native hook registrations | `116b0d7` | 226 agent and 553 hook/tool tests; original Stop/groups/routes preserved; independent flow review accepts |

The wiring review deferred one minor: its regression permits a future matcher.
The actual personal and work worker registrations have no matcher. These are
isolated candidates, not the host's live guard or settings.

## Inventory prerequisite

The installed `tasks 0.2.0` CLI in an isolated disposable main/worktree pair showed:

| Record | Main doing list | Worktree doing list | Claims inventory | Policy selector |
| --- | --- | --- | --- | --- |
| Registered controller | Present | Present | Live | Controller own |
| Worktree-only plan step | Absent | Present | Live | Worker own |
| Dead main-only record | Present | Absent | Not live | Excluded |

Rows preserve id, prefix, owner and bare session. Explicit fixture sessions carry
no root grouping; this does not establish native root equality or related-worker
selection. Existing `tasks claims` plus explicit live/session selection is
sufficient. tasks-724c9e is complete and tasks-27d515 is dropped as covered.
All fixture claims were released, inventory emptied, locked worktree removed and
isolated project/config/state/registry deleted. Details are in the
[delegated identity brief](2026-09-30-delegated-identity-brief.md#claim-inventory-tasks-724c9e-2026-10-08).

## Pilot preparation

The private world is under `.superpowers/sdd/claim-guard-policy/pilot/` in the
coordination worktree. Only tasks config/state are under `project/.private/`;
`control/` is a real sibling outside the workspace and temporary roots. It owns
Codex home, prompts, recorder, author gates and evidence. The fresh fixture now holds three native claims after Stage A; its captured
first blocks and retries are retained outside the model workspace. No author
gate is needed in the proven resumed-process transport.

Codex is `0.161.0`. The current session's recorded routing is `gpt-6.1-sol` with
high effort. The private feature report enables hooks and multi-agent support.
Personal file authentication was accepted; its temporary exclusive 0600 copy was
removed. The source credential and all shared host pointers remain untouched.
The launcher bounds and reaps its foreground process group and removes the copy
on completion, failure, INT and TERM.

Recorder controls pass for capture-only start, exact guard reply, nonzero exit,
immediate timeout and malformed input, with failure evidence retained. Author
helper controls accept the matching verdict and reject a mismatch. Launcher
controls verify credential cleanup on failure, isolated environment and bounded
process cleanup. These are preparation controls, not native delivery evidence.

After user approval, the ordinary native `/hooks` browser trusted the exact
three private definitions and showed all active. Only private-home trust was
changed; the browser was exited and reaped before the model probes.
The ordinary [native hook trust flow](https://learn.chatgpt.com/docs/hooks#review-and-trust-hooks)
is required by the approved plan. Exact source/command/file fingerprints and
its credential-owning review command are prepared in private
`control/hook-trust-review.md`. No bypass or handcrafted trust is used.

An optional standalone sandbox preflight could not execute its command because
this CLI requires a named permission profile. It supplies no permission verdict;
no sentinel survives. The required actual-model no-claim write and denial smoke
remains the next check after trust.

| Stage | Observed verdict |
| --- | --- |
| Recorder/auth/layout controls | PASS, preparation only |
| Private native trust | PASS, three exact definitions trusted through ordinary `/hooks` |
| Actual-model write/deny/recorder smoke | PASS, isolated writes and add/note/drop; controls denied `EROFS`; empty claims; genuine first Stop allows |
| Same-worker resume / fallback smoke | PASS across exec resume, same captured agent ID and distinct turn IDs; fallback unused |
| A: native IDs, held claims and delivered worker event | PASS, native own/related identities; worker and root first blocks then true retries allow; three live claims retained |
| B: second separate worker turn and retry reset | Not run |
| C: normal release and specific worker final result | Not run |
| D: unforced refusal then completion-proven recovery | Not run |
| E: own closure, empty inventory, first-stop allow | Not run |
| Other candidate-home registration smoke | Not run |

Root equality, worker delivery and same-turn retry are verified. The second
held-claim turn, handoff, recovery and closeout remain unverified. Transport is
`resumed_processes`, selected only after its no-claim pre-gate. The fallback has its own
1,800-second foreground budget and 180-second author helpers fully awaited with
a tool timeout of at least 200 seconds or the same yielded process handle.

## Isolation finding and corrected attempt

Stage A attempt 1 stopped before dependent stages: all three native claims were
correct, but the actual worker/root first stops allowed. Replaying those same
captured payloads under the prepared environment blocked correctly. Zsh login
startup replaced inherited XDG config/state roots with personal paths, so the
native hook read a different inventory from model commands that explicitly
restored the scratch paths.

The private launcher now sets child-only `ZDOTDIR` to an empty private startup
directory. The environment control failed before this change and passed after
it, preserving both isolated XDG roots and normal HOME for the installed facts
endpoint. Native hook definitions, recorder and production guard were unchanged.
The failed harness exited; its three aborted fixture claims were snapshotted,
then only its isolated config/state was removed. Raw captures and aborted task
records remain private under `control/attempt-1/`. This is fixture cleanup, not
worker completion or foreign-claim recovery. Tool feedback: hq-c19b34.

A fresh fixture, controller and worker then repeated all no-claim permission,
native delivery and restoration gates before taking claims. Corrected Stage A
passes. It resumes that proven no-claim root with a fresh held-claim worker, so
the preceding actual Stop belongs to the controller whose equality is checked;
a different fresh root would not establish that relationship.

## Cleanup and continuation

Each completed foreground slice has exited and its process group is reaped.
The launcher deletes its private auth copy at every slice exit. Captures and
claim snapshots remain in author-controlled storage for the staged checks and
fresh final review; the author owns their cleanup. Stage B continues on the exact
worker in the proven resumed transport. No live host pointer was changed.
