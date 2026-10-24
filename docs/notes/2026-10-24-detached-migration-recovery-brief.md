# Detached-record migration recovery

Scoping handoff, 2026-10-24. Ideas: tasks-e9ce63, tasks-13d935,
tasks-4e656c. Within the project-location lane tasks-a7460c.
Not an approved design.

## Problem

Migration tooling needs to recover a detached project's pairing after its code
checkout moves, discover projects during a second host's unpaired window, and
know which configuration writes remain to be committed. The three reports name
different points in that lifecycle; none alone settles the recovery contract.

## Current behaviour and evidence

Inspected the main checkout at eedf9a9; no reproduction was run in this pass.
The reports have no source links or attachments; their bodies are the incident
evidence, and the caller environments remain unknown.

- tasks-e9ce63: `init --force --records` refuses after the checkout moves.
  `src/commands/init.rs::run` repoints the registry in memory, but
  `src/repo.rs::Project::init_detached` refuses an existing detached config whose
  checkout differs before the registry save. The report asks to update both.
  `tests/detached.rs::init_records_on_an_existing_records_config_adds_checkout_and_enables_commits`
  deliberately checks the refusal without force; force-specific relocation is
  not established by that test. Initial implementation: 32a21ef.
- tasks-13d935: `projects` fails from an unpaired checkout.
  `src/commands/projects.rs::run` calls `src/scope.rs::registry_warnings`, whose
  local `Project::locate` propagates `records_unpaired` before rows are built.
  Row reachability already handles missing configs. The report also says
  `where <other-path>` failed: `src/commands/where_.rs::run` resolves the supplied
  path, so that second observation still needs reproduction and must not be
  assumed to share the warning helper. Relevant commits: 519e9f3, ca56dd9.
- tasks-4e656c: init writes a new or changed records config without a commit or
  commit-skipped warning. `src/cli.rs::Command::commit_verb` excludes init;
  `tests/detached.rs::register` explicitly commits after init. This is consistent
  with the administration exclusion in
  `docs/specs/2026-10-10-record-commit-design.md`, but inconsistent with the broad
  configuration guarantee in the tasks skill. Existing tasks-d6f0ed addresses
  that prose discrepancy, not this report's commit-or-explicit-warning request.

## Constraints

Keep prefix mismatch, linked-checkout, retirement, overlap and missing-records
refusals. Do not let force replace an unrelated live checkout silently. Keep
registry changes, synced config and location history consistent, and preserve
config comments. Never commit callers' unrelated staged work.

The approved records-home design §3.2/§5.1 and README describe unpaired errors
and forced repointing; the exact exceptions for discovery and relocation need
reconciliation. `docs/plans/2026-10-22-records-detached.md` was approved and
implemented under closed tasks-211e28; this brief grants no implementation
approval. tasks-5aebd6 already owns detached-init cleanup and error-message
minors. tasks-7b1b22 investigates absent/corrupt checkouts, not this healthy but
unpaired case. Neither is a replacement for these three asks. No overlapping
open research or existing handoff/draft spec was found in this checkout.

## Alternatives

1. Narrow recovery changes: support validated forced detached relocation, keep
   portfolio discovery independent of a healthy unpaired cwd, and explicitly
   report config writes requiring a manual commit. Current lean; it preserves
   the administration exclusion while covering every reported outcome.
2. Also make detached init commit its own config through the existing commit
   engine. This reduces caller work but changes the administration contract and
   must account for staged work, failures and idempotent second-host pairing.
3. Keep present behavior and document caller-side repoint/commit/discovery
   procedures. Least code, but it retains duplicated migration logic; it does
   not satisfy the reports unless those tradeoffs are explicitly accepted.

## Unanswered questions

- What evidence distinguishes a moved checkout from an unrelated replacement,
  and which synchronized config changes are safe across hosts? Research owner.
- Which discovery calls can tolerate an unpaired cwd while explicit local or
  prefix reads still refuse? Can the reported `where <other-path>` failure be
  reproduced, including its full invocation? Research owner; incident context
  remains unknown if a minimal fixture cannot reproduce it.
- Should config-only init commit, or report the exact pending path and manual
  recovery? Research owner recommends a choice against both existing designs
  and tasks-d6f0ed; broader API/design changes require subsequent review.

## Proposed decomposition

Goal tasks-72e3d7 is a child of tasks-a7460c. Research tasks-34424b
(P2/m/mid/direct) traces the shared migration lifecycle and recommends a
disposition for each ask. tasks-e9ce63, tasks-13d935 and tasks-4e656c wait on it.
Its fixture matrix covers an unchanged pairing, an unpaired
host, a moved checkout, an unrelated existing checkout, and fresh/modified
configuration with an unrelated staged file. It records observations and
negative controls here and on the task; it implements no production change.
No additional design task is needed before those findings exist.
