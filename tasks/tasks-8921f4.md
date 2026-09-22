---
id: tasks-8921f4
title: Design opt-in relay ancestry identity
status: doing
priority: 2
size: l
complexity: high
process: planned
owner: design/relay-ancestry
created: 2026-09-17T21:08:29Z
updated: 2026-09-22T15:18:19Z
started: 2026-09-22T13:09:21Z
depends: [relay-06b1da]
parent: tasks-c9199a
tags: []
source: tasks-c9199a
agent: codex
spec: docs/specs/2026-09-22-relay-ancestry-identity-design.md
plan: docs/plans/2026-09-22-relay-ancestry-identity.md
---

Approved source: ops docs/specs/2026-09-16-relay-design.md (reviewed after d7e2c33); execution brief: ops docs/plans/2026-09-16-relay-bootstrap.md (approved after bc53c51 with review corrections).

Next: write the separate tasks identity design for review, then its implementation plan.

Read tasks `src/claims.rs`, `tests/cli.rs`, and
`docs/specs/2026-09-05-work-claims-design.md`. Design opt-in configuration and
identity continuity before implementation; preserve all spec §6 tasks constraints.
Outside recognized harness ancestry, native identity wins without opening the
registry, even with host-wide relay enabled. Unknown ancestry is not a shell.
Inside it, match the nearest harness using same-host PID/start/Linux boot evidence;
do not skip an unmatched inner harness. Missing, ambiguous or unavailable proof is
an acquisition error with explicit-identity recovery. Claude hints must agree with
process proof; shared OpenCode processes do not distinguish independent subagents.

Rust reads schema 1 directly without Node. Copy relay-06b1da's versioned handle fixtures and
label the oversized Linux value as a format test. Include the real Codex case:
live harness ancestor, empty snapshot before its first turn's SessionStart hook.
Test explicit TASKS_SESSION/TASKS_SESSION_PID precedence, nested harnesses, plain
shells with corrupt/missing registry, and held-claim refresh/release after registry
loss. Keep existing Live/Stale, takeover/release retries, mutation locking and TTL.
Darwin epochs must not enter Linux pid_start fields; keep its existing native
unverifiable path. Incompatible schema changes coordinate the producer and reader.

Prerequisites: relay-06b1da. Release goal: relay-1231bf. Bootstrap ops-998bbb closure does not mean this deliverable has shipped.

## Notes

- 2026-09-22T13:09:21Z (main): started
  provenance: {"harness_session":"claude-code:3b265943-33e2-4466-947a-581d11a9cbb0","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-09-22T13:14:59Z (main): Design decision 1: the relay-identity opt-in is a new host config file ~/.config/tasks/config.toml, not the registry, an env var, or the committed per-project tasks/.config.toml (which syncs across hosts). Decision 2: in scope, the matched relay Agent.id becomes the claim session for every recognized harness including Claude and Codex, per ops relay-design 6.2; native session vars are a hint that must agree with process proof. Boundary: identity() is reached only from claim_guard/refuse_foreign_live_claim, so ancestry walk and registry read stay on claim-mutation paths.
- 2026-09-22T13:15:42Z (main): Relay schema 1 read: Agent.id is <harness>:<sessionId> over {claude-code, codex, opencode}; process handle is {platform, host, bootId, pid, start} or null; opencode scope is 'process', others 'session'; snapshot is {schema:1, generation, revision, agents}. Registry lives at RELAY_STATE_DIR, else XDG_STATE_HOME/relay, else ~/.local/state/relay, dir 0700 and files 0600, and relay refuses a non-private path. Consequence: the matched Agent.id for Claude and Codex is the same qualified key the lifecycle notes already carry, so for those two harnesses the registry read verifies rather than discovers; only OpenCode needs discovery.
- 2026-09-22T13:20:02Z (design/relay-ancestry): Design spec written at docs/specs/2026-09-22-relay-ancestry-identity-design.md and committed on design/relay-ancestry. Three user decisions recorded: host config file ~/.config/tasks/config.toml for the opt-in; matched relay Agent.id becomes the claim session for every in-scope harness; approach A, verify-then-adopt, with native session variables as hints that must agree with process proof. Awaiting user review before the implementation plan.
- 2026-09-22T13:20:03Z (design/relay-ancestry): parked (waiting on user, review): User review of .worktrees/relay-ancestry/docs/specs/2026-09-22-relay-ancestry-identity-design.md; on approval write the implementation plan with writing-plans, then decompose into step children.
  provenance: {"harness_session":"claude-code:3b265943-33e2-4466-947a-581d11a9cbb0","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-09-22T13:44:40Z (design/relay-ancestry): Spec revised on review. Continuity now has a mechanism: acquisition and continuation are separated, and a caller proves ownership of a claim registry-free by re-deriving the claim's own host/boot/pid/pid_start as an ancestor of itself, so registry loss cannot strand park, done or edit; identity-resolution errors are held rather than raised on an already-claimed task. Match gains harness agreement (claude->claude-code, codex->codex, opencode->opencode) and requires platform linux, so a Darwin handle is parse-only and never yields a claim. Native hints are compared only for the nearest harness, fixing nested mixed-harness sessions. Platform refusal sits below the explicit override. Continuity is step 6 of the decomposition with owner/foreign-caller tests.
- 2026-09-22T13:44:42Z (design/relay-ancestry): parked (waiting on user, review): Re-review of .worktrees/relay-ancestry/docs/specs/2026-09-22-relay-ancestry-identity-design.md, revised against the four review items; on approval write the implementation plan with writing-plans.
  provenance: {"harness_session":"claude-code:3b265943-33e2-4466-947a-581d11a9cbb0","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-09-22T13:53:17Z (design/relay-ancestry): Spec revised on second review. Ownership proof is bound to the caller's nearest harness ancestor, not any ancestor, so a Codex session launched under a Claude owner is refused; proof also requires the scoped hint not to contradict the claim session, normalized over the known raw/claude:/claude-code: and raw/codex: forms only. Takeover is acquisition and always requires resolved identity: --force displaces an owner but never substitutes for one, for live claims, stale claims and start --force alike. note is corrected as status.rs:89 inside status::note and handled separately in 6.6: it guards nothing, keeps foreign notes unrefused, decides the heartbeat by proof, and never loses a note to a relay-level resolution failure; relay off stays byte-for-byte. Section 7 now states the stage order explicit override, configuration, platform support, ancestry, limiting the out-of-scope exemption to hosts where ancestry can be established.
- 2026-09-22T13:53:18Z (design/relay-ancestry): parked (waiting on user, review): Re-review of .worktrees/relay-ancestry/docs/specs/2026-09-22-relay-ancestry-identity-design.md, revised against the second round; on approval write the implementation plan with writing-plans.
  provenance: {"harness_session":"claude-code:3b265943-33e2-4466-947a-581d11a9cbb0","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-09-22T14:07:37Z (design/relay-ancestry): resumed
  provenance: {"harness_session":"claude-code:3b265943-33e2-4466-947a-581d11a9cbb0","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-09-22T14:16:16Z (design/relay-ancestry): Implementation plan written at docs/plans/2026-09-22-relay-ancestry-identity.md and decomposed into eight step children, ordered by dependency: config, snapshot reader, ancestry walk, relay level, ladder and adoption, guard continuity, note continuity, acceptance tests. Self-review corrected two plan defects: Task 6 described claim_guard and park instead of showing them, and its interfaces named an Ownership type the code does not use; the guards now share Ctx::resolve_for_guard, which raises a native resolution failure where identity raised it before and carries only a relay-level one, so relay-off behaviour stays byte-for-byte. Plan fixtures use a neutral hostname after the pre-commit hook refused the real one.
- 2026-09-22T14:16:18Z (design/relay-ancestry): parked (waiting on user, review): User review of .worktrees/relay-ancestry/docs/plans/2026-09-22-relay-ancestry-identity.md; on approval start tasks-8103e8 and work the eight steps in dependency order.
  provenance: {"harness_session":"claude-code:3b265943-33e2-4466-947a-581d11a9cbb0","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-09-22T15:00:46Z (design/relay-ancestry): Plan revised on review; all nine findings verified against the code first. Ownership proof is now gated on relay being enabled and TASKS_SESSION being unset, so an explicit mismatch stays foreign and the opt-in is respected. claims::continuation_identity separates an owner's refresh from acquisition: a repeated start or park by the owner records the claim's own session and never needs fresh resolution, so registry loss cannot strand it and enabling relay cannot rewrite a native key. park.rs gets concrete code because it needs me.tagged at two sites and me.session at a third. refuse_foreign_live_claim resolves first again, and every non-owner path requires identity. The snapshot reader now implements relay's full validateSnapshot/validateAgent/validateHandle plus the checkPrivate ownership, type, symlink and mode checks. Match predicates apply before cardinality so a stale-boot row cannot manufacture ambiguity. stage_platform asks whether /proc is readable rather than what the target triple says, and unknown ancestry is refused before the registry is opened. note warns when ownership evidence is unavailable. The acceptance shim reads /proc/$$/stat, not /proc/self/stat inside a substitution, which was sampling a different process. Every commit builds green under -D warnings via file-level allows removed in step 5, and test filters now name real test functions.
- 2026-09-22T15:00:51Z (design/relay-ancestry): parked (waiting on user, review): Re-review of .worktrees/relay-ancestry/docs/plans/2026-09-22-relay-ancestry-identity.md; on approval start tasks-8103e8 and work the eight steps in dependency order.
  provenance: {"harness_session":"claude-code:3b265943-33e2-4466-947a-581d11a9cbb0","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-09-22T15:18:19Z (design/relay-ancestry): Plan revised on third review; all eight findings verified first. Ownership is now a three-way verdict (ByIdentity, ByProof, Foreign): an identity match keeps using the freshly resolved identity exactly as today, so native Codex parks still record codex:<id> — tests/cli.rs:79 is named in the plan as that regression — and a repeated start still replaces a stale pid; only proof-only ownership substitutes the claim's own identity. The identity decision moved above the liveness refusal so a held resolution error is raised as itself rather than masked by Claimed, and claims::name_the_claim appends the held session per spec 5 and 6.5. Store::insert_park removes the claim with its proof, so the registry-loss test now parks and closes two separately held claims and a new test asserts a parked task cannot be resumed without identity; the README and skill wording lost the resume promise. Task 5 now updates the twelve three-argument identity_from test calls and keeps the relay dead_code allowance until Task 6, where same_session gains its consumer. Test JSON reads go through show's task object. The resolver keeps full-predicate filtering before cardinality but diagnoses a zero-candidate refusal against rows naming the same process, restoring the darwin and harness messages. revision and updatedAt are capped at the producer's safe-integer bound while start stays full-range u64 text. The explicit-mismatch test moved under one shim with relay enabled and TASKS_SESSION_PID set, where removing the guard would actually fail it, and the native-to-relay mode change and force-without-identity cases are now acceptance tests.
