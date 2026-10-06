# Delegated identity acceptance — 2026-10-06

Implementation: `feat/delegated-identity`; Tasks 1–3 committed, Task 4 open.
Contract: [approved design](../specs/2026-10-06-delegated-identity-design.md).
Integration, installation and symptom-report closeout remain gated on real relay
acceptance and the final independent review. Synthetic rows are test fixtures,
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
| Live relay root-only registry | Required trusted isolated Codex SessionStart hook and actual root row | Registered root refuses worker acquisition | Current SessionStart hooks do not invoke relay; no current registry or relay launcher on PATH. Local Node source exists, but no trusted private hook event was delivered for this run | **unverified** |

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

To resume the live relay gate, arrange a clean scratch Codex controller/worker
session with trusted SessionStart delivery to the local relay Node entry, an
absolute private `--subscribers` file, and private `RELAY_STATE_DIR`. Keep native
exports as delivered, enable relay identity only in scratch tasks configuration,
and verify the resulting real root row refuses worker acquisition without
mutating task or claim state. Manufactured hook payloads and shim registries are
not substitutes. No global hooks, configuration or launcher repointing is authorized.

Validation: focused checks named in the plan and `just test-fast` pass for Tasks
1–3. The Task 4 handoff regression passes and fails on the identity assertion
with a temporarily restored root-first selector; the thread-first selector was
restored and rechecked. Final verification and independent-review outcomes are
recorded on tasks-a5a502 before any integration decision.
