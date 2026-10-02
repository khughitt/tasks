# Delegated session identity

Scoping handoff, 2026-09-30. Goal: tasks-2fa8c6. This is not an approved design.
Updated 2026-10-02: recurring and newer worker reports reuse this investigation;
the related opt-in note-stamping capability is scoped independently.

## Problem

Four reports describe Codex workers whose claims become stale as soon as a command
exits, sometimes alongside conflicting session-variable and provenance warnings.
Reliable ownership needs a stable identity that distinguishes the sessions actually
doing the work. The related Claude live-check automation idea is conditional upkeep.
tasks-0005a6 separately asks for explicit session provenance on ordinary gate notes,
so stage attribution need not infer it from nearby lifecycle markers.

## Current behaviour and evidence

- `src/claims.rs::identity_from` tries explicit `TASKS_SESSION`, opt-in relay, Claude
  native identity, Codex native identity, then Unix session identity. A single Codex ID
  or an agreeing pair yields a raw session ID with no PID and uses TTL liveness. This
  landed in `903f04a` under tasks-3190fb.
- Missing Codex IDs reach the Unix fallback when no higher level resolves. Conflicting
  IDs warn and skip the Codex level. The fallback uses the command's Unix session-leader
  PID; `liveness_with` marks a claim stale when its recorded process is gone, subject
  to its existing proof/TTL rules. These paths explain the reported symptoms but do not
  establish what the affected worker actually exported.
- `src/relay/resolve.rs` rejects conflicting hints and requires exactly one qualifying
  process match. Relay is not an automatic cure for multiple logical workers sharing a
  process. The relay design explicitly requires explicit identities for that case.
- `src/provenance.rs::resolve_from` independently omits provenance and warns on native
  conflicts. An explicit claim override does not supply native note provenance.
- tasks-616cd7 received another feedback note after its earlier scope audit, making it
  eligible again. tasks-cf8687 is a newer report of conflicting native variables,
  Unix fallback, and omitted provenance. Neither includes a controller/worker capture;
  recurrence supports reusing the investigation, not assuming a demonstrated cause.
- `src/commands/status.rs::note` always uses `append_note`, which leaves provenance
  absent. `append_lifecycle_note` already appends normalized text and attaches the
  existing optional pair through the native resolver. The reader round-trips that
  pair today. The lifecycle writer landed in `8f13112`, and its local plan documents
  the separate claim and consumer contracts. A plain note can opt into the same
  metadata without recognizing gate prefixes or changing task status.
- `just test-one codex` passed six tests on 2026-09-30, including native claim survival,
  conflicting-variable fallback, and relay hint disagreement. These are controlled
  fixtures, not a fresh reproduction in a delegated worker.
- tasks-fc61f4 records a successful outer/nested Claude live check. Its recipe remains
  in `docs/plans/2026-09-23-relay-session-process.md`, Task 5. Vendored ancestry and
  process-handle fixtures provide repeatable synthetic coverage; no automation script
  for that live recipe was found.

## Constraints

Preserve the four reports rather than label them duplicates without captures. The
original `ai-2f1271` source is outside this fixed checkout and was not inspected; the
missing-ID account in tasks-0c2c39 remains report evidence. The other reports do not
record the effective relay configuration or full parent/worker identity relationship.
For tasks-0005a6, the original mindful lookup returned no matching thought on
2026-10-02; its captured body and source reference remain the available evidence.

Keep session/PID pairs from one resolution level. Do not choose whichever conflicting
variable happens to work, or merge worker ownership into the controller by assumption.
Claim liveness, lifecycle provenance, and controller completion gating are separate
contracts. Investigations isolate config, state, registry, and processes; host settings
and shared launchers stay untouched. Existing ownership/precedence changes require
reviewed design and plan work after evidence establishes the needed change.
Opt-in note stamping preserves plain-note behavior, the existing pair schema and
warning-only provenance failures, and all existing note mutation/claim checks.
Generated text remains the lifecycle classifier; user text equal to a marker retains
the already documented ambiguity. Flow and stage-cost consumers were not inspected
in this checkout, so adoption there remains separate owner work.

## Alternatives

1. **Repair identity delivery at the harness boundary.** If workers receive stale,
   conflicting, or absent IDs, fix the adapter/configuration that supplies them. This is
   the first path to investigate because agreeing native IDs already pass the survival
   check.
2. **Change tasks' resolution contract.** If correct worker signals cannot be represented,
   design how to distinguish them while preserving explicit overrides and ownership.
   Do not weaken conflict checks without evidence for a trustworthy selector.
3. **Use an explicit worker identity as a bounded workaround.** It must remain stable
   across that worker's commands; any PID must belong to its lifetime. This does not
   by itself solve native provenance or controller aggregation.

Current lean: capture one controller/worker pair before assigning the fix to tasks,
relay, harness configuration, or controller hooks. One investigation covers the overlap.
For ordinary notes, choose explicit `note --stamp` over automatic prefix recognition:
the existing append helper supplies the capability while tasks stays ignorant of
flow's gate vocabulary. Stamping cannot repair conflicting worker identities.

## Unanswered questions

- Are worker IDs missing, inherited from the controller, or independently correct?
  Research tasks-1ece46 answers with presence/equality relationships and versions.
- Which resolution mode and process proof were active, and do controller and worker
  share a process? The same capture must answer this before proposing ownership rules.
- Does the controller need to aggregate child claims, or is claim identity alone wrong?
  Research identifies the boundary and owner; this pass does not change that policy.

## Proposed decomposition

- tasks-0c2c39, tasks-5745bb, tasks-616cd7, tasks-cf8687: briefed; remain ideas under
  tasks-2fa8c6. The 2026-10-02 pass reconsidered tasks-616cd7 and added tasks-cf8687;
  the first two were related context, not processed again.
- tasks-1ece46: priority 2, small/mid/direct investigation. Capture one controller/worker
  pair, distinguish actual observations from synthetic controls, recommend an owner and
  regression check, update this brief, and add finding notes that wake all four ideas.
  No speculative implementation or separate design task is filed yet.
- tasks-0005a6: scoped, priority 2, small/mid/direct; independent `note --stamp`
  capability using the existing resolver, pair codec, warnings, and note write path.
  Check stamped versus plain gate text, native inputs and conflicts, round-trip
  preservation, status, and claim behavior. It remains outside the claim-identity
  goal and is not blocked on the investigation. No new umbrella goal or handoff is
  needed for this independent scoped capability.
- tasks-481fd6: shelved outside the goal. Wake when a relay/tasks change to process
  selection or registry identity matching requires another outer/nested Claude live
  check; reuse the existing Task 5 recipe then.
