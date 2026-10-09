# Project discovery and individual task movement

Scoping handoff, 2026-10-08. Lane: tasks-a7460c. Not an approved design.

## Problem

Project discovery should explain why a directory cannot be used (tasks-fa0276).
Moving one task to another project should preserve the history and references the
user needs (tasks-7ba557). Both use project identity and lookup, but the diagnostic
can proceed independently of migration design.

## Current behaviour and evidence

- `src/link.rs::GitTree::of` asks Git for repository facts. After a
  not-a-repository response it scans ancestor `.git` markers; any marker retains
  an `io` error, whose detail names the starting directory rather than the marker.
  This corruption safeguard landed in `7040de3` under tasks-9b0a2e.
- An isolated installed-CLI probe ran `where` on a child directory with scratch
  config and state. No marker: exit 0, project null. Empty ancestor `.git`: exit 1,
  kind io. Removing that marker restored exit 0. A healthy unborn repository also
  succeeded; corrupting its HEAD produced io with the same Git error text. Scratch
  files were cleaned up. This did not reproduce init/feedback failures, the full
  suite, or the reported host process recreating the marker.
- tasks-084a9f already owns the separate problem of the marker scan crossing
  `GIT_CEILING_DIRECTORIES`; reuse its boundaries when changing this shared helper.
- Project-resolution and linked-checkout work (tasks-7580d2, tasks-9b0a2e) is done.
  `Registry::canonical_id` in `src/registry.rs` changes a whole prefix and retains
  the suffix. `src/resolve.rs::Resolver::resolve_task` then selects the project.
  It has no individual-task forwarding table. The implemented
  `docs/specs/2026-10-06-project-resolution-design.md` describes this rule and does
  not promise that a resolved task exists.
- `src/commands/add.rs` reuses matching source-and-title records, but a newly added
  task gets a new identity and does not inherit notes or attachments automatically.
  `src/hierarchy.rs::validate_parent` requires a same-project parent. A project
  rename therefore cannot implement moving one task out of a still-live project.

## Constraints

Preserve corrupt-metadata errors and failed-init protection. Do not remove or repair
host Git metadata during scoping. Name the precise marker and provide inspection
guidance without telling the user to delete unknown repository data. Keep normal
no-project and healthy-unborn behavior unchanged.

Movement must account for incoming dependencies, parent/child ownership, original
notes/provenance, attachments and their ledger, relative spec/plan links, claims,
parks, destination collisions, interrupted writes and reruns. Existing prefix aliases
and location history are host-local; they are not a portable task-migration journal.
No live move, redirect schema or new JSON shape is approved here.

## Alternatives

For discovery, retain conservative refusal with a precise diagnostic, or define a
new rule treating selected markers as absent. Choose the diagnostic option now:
the original report permits it, and Git's error text alone cannot distinguish the
empty-directory case from damaged metadata. Treating markers as absent needs
separate evidence and a reviewed boundary if still wanted.

For movement, compare:

1. Keep source-and-title add/drop with explicit manual transfer and reference repair.
2. Provide an explicit migration to a new destination id, preserving a source record
   and defining exactly what is transferred or refused.
3. Add per-task forwarding so old references resolve transparently, with a reviewed
   authority, persistence and cross-host contract.

Lean toward evaluating option 2 against option 1 before introducing forwarding.
Transparent old-id resolution is an unresolved requirement, not something the current
prefix-alias mechanism supplies.

## Unanswered questions

- Must old task ids transparently resolve after a move, or is a retained source record
  sufficient? The migration design author proposes concrete examples for user review.
- Which records can move together, how are unavailable incoming-reference owners
  handled, and what constitutes a recoverable interrupted move? The design compares
  explicit refusal with supported transfer cases before choosing a write protocol.
- Does a precise discovery diagnostic resolve the reported friction? The direct task
  verifies the requested diagnostic alternative; changing discovery classification
  remains outside it. The creator of the host's stray marker is still unknown.

## Proposed decomposition

- tasks-fa0276: **scoped**, P2/s/low/direct, under tasks-a7460c. Name the obstructing
  marker with actionable inspection guidance through the shared discovery helper;
  verify empty-marker diagnostics, ordinary discovery, corrupt metadata and failed
  init preserving state. tasks-084a9f retains discovery-ceiling semantics.
- tasks-7ba557: **briefed**, stays an idea with its original report preserved.
- tasks-bf6b9b: P2/m/high/planned, design individual task migration. Produce a
  user-reviewed spec and then plan; no implementation or live migration. On completion,
  update this brief and add the finding to tasks-7ba557 in the same commit. Reuse the
  existing lane and identity code; do not create another project-resolution effort.
