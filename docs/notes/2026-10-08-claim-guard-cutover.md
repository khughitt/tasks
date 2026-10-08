# Claim-guard coordinated cutover — approval packet

Status: ready for approval; fresh whole-branch review and corrective re-review accepted. Live guard and native homes
are unchanged. This packet requests the final approval required by the accepted
[plan](../plans/2026-10-08-claim-guard-policy.md#task-6-review-approve-and-activate-the-coordinated-cutover).

## Proposed activation

| Order | Owner / exact candidate revision | Result |
| --- | --- | --- |
| 1 | hq `a3662f6` | Remove the temporary consumer mirror and recipe; retain staged fact validation |
| 1a | hq `c9c787b` | Include the corrected, verified owner-flow metadata; runtime tree unchanged |
| 2 | ops `bd51421557a88797c8a79906c5bed67b2de76c69` (includes runtime `91984f5` plus reviewed test correction) | Existing live Stop command reads validated hq facts and uses the reviewed claim policy |
| 3 | hq `116b0d7abaeb2ef01e904ed1caa1aa0acbac681b` | Add the worker guard once in both native homes; preserve existing Stop and unrelated hooks |

Use normal local merges/cherry-picks and owner hooks. Hq integrates through its
mirror phase before the ops branch, then through its separate wiring phase.
Ops main has unrelated subsequent commits; a normal merge preserves them.
Native trust is then established only for the new guard definitions through each
home's ordinary `/hooks` browser. A skipped/untrusted worker hook is not activated.
The operation includes final owner gates, explicit-main fixture controls and task
closeout. No host pointer is redirected to a worktree.

The identical addition in **both** `codex/hooks.json` and
`codex/hooks.work.json` is:

```diff
+    "SubagentStop": [
+      { "hooks": [{ "type": "command", "command": "~/d/ops/hooks/claim-guard --harness codex" }] }
+    ],
```

Neither addition has a matcher. Each existing Stop array and all unrelated parsed
event groups remain intact. `links.toml` continues to install the two sources into
`~/.codex/hooks.json` and `~/.codex-work/hooks.json` respectively.

## Observed state and safeguards

- Ops main: `350951edc109e7bcec57b580314332486b81654b`; clean working tree.
  Its changes since the candidate base do not touch the guard, guard tests,
  README or split-design note. `git merge-tree` reports a clean candidate merge
  with the corrected candidate, tree `dccebbe35deafab4c728e35ffff14fa26841dddb`.
- Hq main: `c00e1c0402d2ecff59947259afe37e2e72b715a2`; no relevant draft.
  Untracked feedback `hq-c19b34` stays with its maintainer.
- `~/d/ops/hooks/claim-guard` resolves to ops main. Both installed hook files
  resolve to their hq main sources. `~/.agents/bin/harness-facts` resolves to
  hq main `tools/harness-facts`, whose default file is `facts/capabilities.toml`.
  The installed `tasks` is `0.2.0` at `~/.cargo/bin/tasks`.
- Current facts bytes equal the candidate baseline. SHA-256:
  `1eea51c090a5eafbb472b4ff96a26404a1ece0c482d5f7732b2a5389b8270621`.
  No capability values change in this operation. Fact edits stay in hq worktrees.
- Read-only feature inspection reports hooks and multi-agent enabled in both
  actual native homes. Their new worker definitions still need ordinary trust
  after the approved wiring change.
- Pre-cutover main files, hashes and SHAs are retained in this plan's private
  `cutover-before/` snapshots. Recheck heads, relevant drafts, endpoint resolution
  and fact hash immediately before activation; reconcile drift before continuing.

## Verification

See the [acceptance record](2026-10-08-claim-guard-acceptance.md) for observed
stages and limits. Normally trusted Codex 0.161.0 private execution passed:

- Actual model state writes, denied control mutation, empty no-claim inventory and
  genuine root Stop; same-worker restoration across exec resume.
- A/B: same worker in separate held turns, reset first-stop flag, block followed by
  allowed retry, with all claim owners and heartbeats retained.
- C: normal worker park/release followed by controller start/review/done; specific
  result plus tracked completion; separate control claim retained.
- D: controller-native unforced refusal with unchanged record/store hashes, then
  explicit stopped-worker recovery with reason and closure under its own claim.
- E: controller closes its own remaining task, zero claims, actual first Stop allows.
- Fresh no-claim native registration/delivery/identity smokes for both candidate
  sources. Their selected definitions are identical; unrelated work integrations
  were excluded.

Fresh owner gates passed: ops real reader 5/no skips, fast 1,277 with one existing
skip and hygiene; hq 226 agent plus 553 hook/tool tests and full fact validation;
tasks fast 1,159 with one ignored and hygiene. One earlier coordination run failed
68 temporary init fixtures; focused and unchanged full reruns passed, and feedback
tasks-eec7d5 records it. The initial pilot environment failure is separately
recorded as blocked, with a fresh successful attempt after private shell isolation
was corrected. Neither failure is silently reclassified as passing evidence.

Fresh full review required one test-only source-coupling fix, now accepted by a
fresh scoped reviewer. The runtime guard bytes and native definitions did not
change. Deferred minors: the guard detached-warning fragment and the wiring
regression permitting a future matcher; the actual two stanzas have none. Stored wake facts were not re-probed. Native ancestry
coverage and rootless/intermediate-worker limits remain those of the accepted
spec. Single-process fallback was prepared but unused because restoration passed.

## Coordinated rollback

On activation failure, stop dependent work and retain evidence. Restore through
reviewed reverts as a coordinated set:

1. Restore both previous Stop-only native files, preserving every unrelated hook.
2. Restore ops's pre-cutover guard, embedded constants and mirror tests together.
3. Restore hq's former mirror call and recipe only after the referenced ops test
   exists again; retain its staged validation and unchanged capability file.

Use the recorded pre-cutover SHAs and snapshots to verify these exact surfaces.
Never restore hq's mirror call while its ops test remains deleted. Existing
unrelated ops main commits remain. Review the resulting native definitions through
normal trust as needed. No reset, force push or host-pointer workaround is part of
rollback.

## Cleanup ownership

Native pilot slices exited and were reaped; the private credential is absent and
fixture config/state removed. Raw private captures, aborted-attempt evidence,
verdicts and this plan's ledger remain for review under the coordination worktree;
the author owns their cleanup after disposition. Candidate worktrees remain until
integration and checks finish. Their removal requires ignored-file review,
`tt-report`, host-pointer checks, and ledger rulings/minors in the final report.
