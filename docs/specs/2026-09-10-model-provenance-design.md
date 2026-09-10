# Model provenance: record the model behind a task's completion

Status: approved in chat 2026-09-10; not yet implemented
Task: tasks-222dab

## Problem

A task record knows who owns it (the session identity) and who wrote each note, but
nothing says which model produced the implementation. Across a corpus of completed tasks
that is the missing half of observability: "which tasks were completed under model X, and
how did they fare" is unanswerable, so model choice cannot be evaluated against outcomes
or rework. Writing it into bodies or notes is prose: it cannot be queried and it drifts.

## Decision

One optional field, `model`, holding the model id the harness reported for the session
that ran the task's latest completion. Like `source`
(docs/specs/2026-09-06-task-source-design.md), it is an opaque string: tasks stores it,
prints it, and returns it in JSON, and never interprets, resolves, or validates it against
any model registry. `claude-fable-5-1` and an OpenRouter slug are equally acceptable.

**The field asserts latest-completion attribution, nothing more.** It names the model the
harness reported for the session in which the completion transition ran. The bulk of the
implementation may have happened in earlier sessions under other models; the field does
not claim otherwise, and no per-occurrence or per-session history is kept. Last
completion wins.

The harness supplies the value through an environment variable, `TASKS_MODEL`, following
the `TASKS_SESSION` precedent: the harness knows the exact model id more reliably than
the agent does, and setting it once per session puts zero friction on any individual
command. There is no flag on `done`; explicit correction after the fact goes through
`edit` (below).

## Rules

- **Value.** A non-empty, single-line string. Multi-line values are a validation error at
  write time and a parse error when read from a file, with the same rule and error text
  shape as `source`. Never interpreted.
- **Supply.** `TASKS_MODEL` is read from the environment at completion time. Unset or
  empty means "no model to record". A non-Unicode value is an explicit validation error
  naming the variable; it is never silently treated as unset.
- **Write path.** The stamp lives in `transition()` and fires exactly when `completing`
  is true: a fresh transition into `done`. That covers every completion entry point with
  one rule — `tasks done`, `tasks edit --status done`, and flipping the status in the
  editor all funnel through `transition()` (src/commands/edit.rs:192), and each stamps.
  The retry path does not stamp: repeating `done` on an already-done recurring task
  *while a claim or park still lingers for this worktree* — the residue of an
  interrupted completion's failed cleanup — takes the claim-release branch
  (`ctx.recovered`, src/commands/mod.rs:567), where `completing` is false, so a recovery
  under a different model never overwrites the original attribution. A bare repeated
  `done` after a clean completion finds no claim or park and errors ("`tasks start`
  first") before any write, so it cannot restamp either. Reopening is not a completion
  and never touches the field.
- **Recompletion clears.** A fresh completion with `TASKS_MODEL` unset or empty sets
  `model` to absent. Preserving the previous stamp would attribute the latest occurrence
  to a model that did not complete it; "unknown" is the accurate value. Each occurrence
  of a recurring task therefore carries the model of *its* completion, or nothing.
- **Correction.** `tasks edit --model <id>` replaces the stamp; `tasks edit --no-model`
  clears it; the two conflict. The flags live on `EditArgs`, not the shared `FieldArgs`:
  `add` never completes a task, so it has nothing to attribute. Because the binary is the
  only writer, this edit path is what makes a wrong stamp fixable. When one invocation
  both completes and corrects — `TASKS_MODEL=A tasks edit <id> --status done --model B`,
  or an editor session that flips the status and edits the model line together — the
  fresh completion stamps last and the correction is overwritten, because field
  application runs before `transition()`. Corrections are a subsequent edit, after the
  completion; that is what "after the fact" means.
- **Frontmatter.** `model:` is written after `source` and before `spec`, omitted when
  absent, quoted on the same rules as `source` so the value round-trips byte-for-byte.
  Parsing and serialization live in `format.rs` beside `source`; the generic
  `frontmatter.rs` does not change.
- **JSON.** Every task object gains `model`, `null` when absent: the full `Task` in
  `show` and `next`, the `TaskSummary` rows in `list`, `ready`, `prime`, and `tree`, and
  `ParkedRow` beside it. This is an additive contract change; no existing key changes.
  Summary rows carry it so `tasks list --status done | jq` is the query path — plain
  `list` shows open tasks, and the corpus this feature answers questions about is the
  completed one.
- **Pretty output.** `show --pretty` prints the frontmatter line. Tables do not gain a
  column; the JSON carries the field.
- **Docs.** `skills/tasks/SKILL.md` gains one line (harnesses export `TASKS_MODEL`;
  completion records it), and the README mentions the variable where it documents
  `TASKS_SESSION`.

## Deferred

- A `list --model` filter. jq over the summary rows covers the query; add the flag when a
  real consumer needs it.
- Key parameters beyond the model id (effort, temperature, harness version). Scoped down
  to model id only; notes remain the place for anything richer.
- Stamping `start` or notes. Completion is the moment that matters; earlier sessions are
  deliberately unattributed.
- Harness auto-detection. tasks never sniffs the environment for a model; the harness
  exports the variable or nothing is recorded.

## Testing

The regression cases that matter, in addition to the usual round-trip and validation
units mirroring `source`:

- Complete with `TASKS_MODEL=A` → reopen → complete with the variable unset: the field
  clears rather than preserving A.
- Complete with `TASKS_MODEL=A` on a recurring task → `start` the next occurrence →
  `done` under `TASKS_MODEL=B`: the stamp becomes B. Then arrange a completion whose
  claim cleanup failed by planting a claim entry for the done task in the claims store —
  park cannot help here, it rejects closed tasks — and retry `done` under
  `TASKS_MODEL=C`: the claim-release branch runs and the stamp stays B. Retrying under B
  could not detect accidental restamping; under C it can. A bare repeated `done` with no
  lingering entry errors and writes nothing.
- A completing edit combined with a correction stamps the completion: both
  `TASKS_MODEL=A tasks edit <id> --status done --model B` and the same with `--no-model`
  end stamped A, as does an editor session that flips the status and the model line
  together. The correction holds when applied in a following, non-completing edit.
- `edit --status done` and an editor-driven status flip both stamp identically to `done`.
- A non-Unicode `TASKS_MODEL` fails the completion with a validation error naming the
  variable.
- `edit --model` / `--no-model` replace and clear; the two flags conflict.
- JSON carries `model` (null when absent) in `show`, `list`, and `prime`'s parked rows;
  pretty `show` prints the line and tables stay unchanged.
- `tests/common/mod.rs` scrubs `TASKS_MODEL` in both command builders (`cmd` and `raw`)
  alongside the other `TASKS_*` removals, so the harness's own variable cannot leak into
  a test run; provenance tests set it explicitly.
