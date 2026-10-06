# Delegated identity acceptance — 2026-10-06

Implementation: `feat/delegated-identity`; Tasks 1–3 committed, Task 4 open.
Contract: [approved design](../specs/2026-10-06-delegated-identity-design.md).
Native and real relay live acceptance pass. Final independent review accepted the
refreshed branch and independently corroborated the live relay evidence. The one
stale documentation sentence was corrected. Integration and installation follow
the approved local closeout procedure. Synthetic rows are test fixtures,
never evidence of delivered worker registration.

| Case | Evidence | Expected result | Observed result | Verdict |
| --- | --- | --- | --- | --- |
| Split native root/thread selection | Native unit and CLI regressions | Thread key, tagged thread, no PID or proof | Baseline produced Unix key; remedy selects worker | pass |
| Malformed selected native values | Native unit/CLI tables, Unicode and controls in either value | Name variable/recovery; no input-value leak, no fallback or mutation | Both values validated even with valid thread; writes refuse, read views succeed; held-claim errors direct recovery through show and omit both native values | pass |
| Explicit and Claude precedence | Native unit controls | Higher tiers remain authoritative | Explicit owner can recover despite malformed Codex input; Claude precedence retained | pass |
| Native note provenance | Provenance/format/CLI tables | Thread source including equal IDs, unchanged serialized pair | New stamps use concrete thread; malformed/mixed/qualified conflicts warn and omit | pass |
| Root persistence and recovery | Store/output and `delegated_root` checks | Optional qualified root; retain absent refresh, refuse conflict, replace takeover | Old records and opaque `codex:codex:x` load; conflict preserves files/store; release/reacquire succeeds | pass |
| Root projection/read holds | `show`, `prime`, `claims`, read-only hold regression | All claim views expose root; failed identity owns no holds | Projections agree; malformed-root reads succeed and hide work behind the foreign hold | pass |
| Relay worker selection | Synthetic shim and resolver tables | Match concrete actor; root-only row refuses without controller adoption | Worker row acquires qualified worker key/root; root-only error names relay-f56e30; unrelated mismatch stays generic and value-free | pass |
| Relay proof-only ownership | Unit and synthetic shared-process CLI controls | Matching valid thread plus process proof only | Matching thread continues with registry missing; sibling, session-only, absent and malformed hints fail across start/park/done/protected edits/heartbeat/holds | pass |
| Relay owner recovery | Synthetic exact-identity control | Exact held TASKS_SESSION recovers without fabricated PID/thread | Owner refresh and release succeed; native root is retained on owner refresh | pass |
| Handoff and takeover | `delegated_worker_handoff_and_takeover` | Foreign release refuses; park/start handoff and explicit takeover succeed | Bidirectional refusals preserve record/store; worker done releases; holds do not transfer by root | pass |
| Public agent-waiting park | Native CLI regression | No controller reservation | Third agent sees doing park in next/list --parked and starts it; ready retains its todo filter | pass |
| Native real controller/worker | Codex 0.160.1, actual exported IDs, explicit worktree binary, private scratch config/state | Distinct persistent worker claims, shared root and thread provenance; foreign close refused | Clean Codex pair, worker thread distinct/root shared, claim live after separate command exits, no PID, exact provenance, worker cannot close controller | pass |
| Native real reverse handoff | Same actual pair and tracked worker-specific completion | Controller cannot close live worker; normal release/start; forced takeover only after stopped worker | Controller done/park refused without mutation; worker parked normal task before final report; after tracked completion unforced control close still refused, recorded forced start/done succeeded | pass |
| Live relay root-only registry | Codex 0.160.1, normally trusted isolated SessionStart hook, actual root row and actual worker | Registered root refuses worker acquisition | Genuine current-root event matched native exports; root acquired a live qualified claim; distinct worker had no row and start/controller done/park refused without changing task, claim or registry; errors omitted native IDs | pass |

The native worker issued acquire and checkpoint commands separately, refreshed and
stamped both worker tasks, and reported a checkpoint while holding them. The
controller inspected live claims and checked its own premature done/park refusals
before authorizing release. The worker then parked the normal task and explicitly
identified one intentionally unreleased control. Its tracked final completion
reported no pending tools, checks or background processes. Only after receiving
that worker-specific result did the controller take over the control. A checkpoint,
timeout, shared PID or root session state was not used as termination evidence.
All native scratch claims were released and scratch registry/state removed.

The live root is the shared tree root. This one-level exchange does not establish
an immediate-parent contract for nested workers. No worker Stop/SessionEnd/Interrupt
delivery or relay worker registration has been repaired. Session-only workers
remain indistinguishable from their controller; uncaptured historical missing
exports have no new evidence. The six split-ID reports and tasks-0c2c39 remain open
until the approved closeout gates are met. relay-f56e30, tasks-f8c9cc and tasks-56b450
remain outside the remedy.

Real relay capture used a separate private Codex home, empty subscriber list,
private task config/state and `RELAY_STATE_DIR`, and the explicit worktree binary.
The user reviewed/trusted the exact hook through `/hooks`. A startup handler
forwarded the genuinely delivered payload to the existing relay Node entry with
an absolute private `--subscribers` file, recording an event receipt. Neither
registry rows nor hook payloads were manufactured. The controller verified its
own receipt and matching live relay root claim before dispatching one real worker.
The worker checked acquisition and controller release refusals with unchanged
record/store/registry snapshots. Tracked worker completion preceded controller
cleanup; the final verdict reports all scratch claims released, controller exit
and owned process-group cleanup. Global configuration and launchers were unchanged.

Two setup limits were observed and preserved before retry. Workspace-write command
isolation exposed the nearest Codex ancestor as PID 1 while relay published its
host PID, preventing controller acquisition. The successful capture used the
user-approved per-invocation `danger-full-access` mode to share host PID visibility;
namespace support is tracked separately as tasks-b9cbe3. The first host-PID run
verified controller acquisition but `--ephemeral` prevented loading a worker fork
context. The retry retained rollouts only in the private Codex home. No passing
worker verdict was inferred from either failed attempt or from controller exit.
After independent evidence inspection and a final zero-claims check, private
scratch task/relay state, native-ID receipts, the sign-in copy and Codex rollouts
were removed. Only sanitized verdict/completion summaries remain in the local
execution ledger.

Validation: focused checks named in the plan and `just test-fast` pass for Tasks
1–3. The Task 4 handoff regression passes and fails on the identity assertion
with a temporarily restored root-first selector; the thread-first selector was
restored and rechecked. After refreshing from current main, final `just test-fast` passes 1,121 tests
(457 unit, 36 attachment, 615 CLI, 2 front-door and 11 resolver); one unrelated exhaustive enumeration is ignored by the fast target.
Commit hooks passed hygiene, format, clippy and task checks. Independent review
round 1 reproduced a held-session diagnostic leak; the strengthened regression
failed before the shared recovery suffix was corrected to direct inspection
through `tasks show`. Round 2 reproduced the refusal without native values and
accepted the correction with no remaining findings. It also verified that process
controls use matching thread input and override guidance is conditional. Reviews
are recorded on tasks-a5a502. Final review round 3 accepted the refreshed branch,
independently corroborated actual relay delivery/worker checks and process cleanup,
and reported no Critical or Important findings; its one stale-documentation Minor
finding was corrected before closeout.
