# Claim-guard acceptance — 2026-10-08

Status: candidate implementation verified; live pilot stopped before model runs
and claim acquisition because its three private native hooks need normal trust
review. No ops/hq live activation has occurred. The author owns continuation after
that permission, then the separately reviewed cutover.

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
Codex home, prompts, recorder, author gates and evidence. Three scratch tasks are
unclaimed; the private inventory and native evidence/gate directories are empty.

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

The native private `/hooks` browser shows three definitions awaiting review,
zero active. The browser was inspected and exited without trusting hooks or
submitting a model request; project trust was saved only in the private home.
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
| Private native trust | BLOCKED, three untrusted definitions |
| Actual-model write/deny/recorder smoke | Not run |
| Same-worker resume / fallback smoke | Not run |
| A: native IDs, held claims and delivered worker event | Not run |
| B: second separate worker turn and retry reset | Not run |
| C: normal release and specific worker final result | Not run |
| D: unforced refusal then completion-proven recovery | Not run |
| E: own closure, empty inventory, first-stop allow | Not run |
| Other candidate-home registration smoke | Not run |

No root equality, worker delivery, retry reset, handoff or recovery pass is claimed.
Neither transport is selected before its pre-gate. The fallback has its own
1,800-second foreground budget and 180-second author helpers fully awaited with
a tool timeout of at least 200 seconds or the same yielded process handle.

## Cleanup and continuation

The private CLI exited and its process group was reaped. No private credential,
scratch claim, native capture or author gate remains. The prepared private home,
scratch task records, prompts and review files are retained for the author to
resume; they are ignored and never staged. `host-load --section session` reported
nothing left running. The next action is approval of the exact three private hook
definitions, normal `/hooks` trust, then the required no-claim smoke. Live cutover
remains a later, separately prepared approval.
