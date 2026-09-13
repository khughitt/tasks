# Creation provenance: record the agent that filed a task

Status: implemented (2026-09-13). CLI on tasks main; hook ops-a405cc and wiring ai-d5a56c landed; the live /model transition passed (opus-5 → sonnet-5, both stamps followed).
Task: tasks-dc599b

## Problem

A task written by an agent carries that agent's assumptions: what it read, what it
took for granted, how it phrased the ask. Knowing which harness and model filed it is
context for reading it later — tasks-61cc5c was captured by Crush running Kimi-K3 and
tasks-dc599b by Codex running GPT-6, and neither record says so except, in one case, as
a trailing sentence in the body. Prose cannot be queried and is not written consistently.

The completion half of this already exists: `model`
(docs/specs/2026-09-10-model-provenance-design.md) records the model behind a task's
latest completion, supplied by the harness through `TASKS_MODEL`. Two findings shape
this design:

- **The completion stamp is unwired.** No harness configuration on this machine exports
  `TASKS_MODEL` — not the ops session-start hook, the ai Claude settings, nor the codex
  config — and no task record in any registered project carries `model:`. A creation
  field built on "the harness exports a variable" is inert until the harness side is
  done, so that wiring is in scope here, as pieces in the projects that own it.
- **Claude Code can supply the value, but not with one export.** Its `SessionStart`
  hook input may carry `model` (not guaranteed) and only SessionStart, Setup, CwdChanged,
  and FileChanged hooks can write `export …` lines to `$CLAUDE_ENV_FILE`, the script
  Claude Code runs as a preamble before every Bash command. The model can change
  mid-session (`/model`, automatic fallback, `opusplan`, resume), and the hook that
  follows those changes, `PostModelSwitch`, receives `to_model` but cannot write the env
  file. A value exported once at session start would misattribute everything after a
  switch. Both hooks receive `scratchpad_dir`, so the export can point at a state file
  the switch hook rewrites. ops already owns the session-start hook.
- **Codex knows its harness, not its effective model.** Codex 0.154.0 has hooks
  (`session_start` among them, already configured), but no documented model field or
  environment persistence, and `codex --model` overrides the configured default, so a
  static export of the configured model can be wrong. Crush and others are unknown.

## Decision

One optional field, `agent`, holding the harness and model that created the task.
Like `source` and `model` it is an opaque single-line string: stored, printed,
returned in JSON, never interpreted, resolved, or validated against a registry.

**Value convention** (instruction, not validation): `<harness>/<model>`, both as the
ids the harness itself reports — `claude-code/claude-opus-5`, `codex/gpt-6`,
`crush/kimi-k3`. When only the harness is known, the harness alone: `claude-code`.
An unknown identity records nothing; it never blocks capture and is never guessed.

**The field asserts creation attribution, nothing more.** It names the agent that ran
the `add` (or `feedback`) that wrote the record. Later editing, scoping, and completion
by other agents leave it alone; `model` remains the completion side, and the two are
independent — a task can carry either, both, or neither.

Alternatives rejected:

- **Two fields, `harness` and `created_model`, two variables.** More directly
  queryable, but doubles the surface (two flags, two clear flags, two env variables,
  two frontmatter keys) for a value that is almost always read as a pair. `jq` handles
  `split("/")` when a query needs one half.
- **Reuse `TASKS_MODEL` at creation into a `created_model` field.** Loses the harness,
  which is the half the motivating incident actually lacked (three harnesses, and the
  same model can run under several).
- **Body or note convention only.** What exists today; it is not queryable and is
  written by one harness in three.

## Rules

- **Value.** Non-empty, single line, validated with the same helper and error shape as
  `source` and `model` (`validate_line("agent", …)`); a multi-line stored value is a
  parse error when read.
- **Supply.** At creation the value comes from, in order: `--agent <value>` on `add`;
  otherwise `TASKS_AGENT` from the environment; otherwise nothing. The explicit value
  is resolved first and the environment is read only when no flag was given, so an
  invalid variable cannot fail an `add` that names a valid agent. Unset or empty
  `TASKS_AGENT` records nothing. A non-Unicode value is a validation error naming the
  variable, never a silent skip — the `completion_model()` shape, factored so both
  variables share one reader.
- **Write path.** One resolver, `creation_agent(explicit: Option<&str>)` in
  `commands/mod.rs`, validates the flag when present and otherwise reads the variable.
  `add::blank()` takes the resolved value, so the two creators share it: `tasks add`
  (and therefore quick-add) passes its flag; `tasks feedback` passes `None` and is
  harness-driven only. `feedback` gains no flag: its reports must stay free of project
  detail, which a hand-typed value could carry. `apply_fields` also sets `agent` when
  the flag is present, which on `add` rewrites the same value.
- **Correction.** `agent` joins `FieldArgs`, so `edit --agent <value>` replaces the
  stamp; `edit --no-agent` clears it; the two conflict. Editor saves validate through
  the record parser. An `edit` never reads `TASKS_AGENT`: only creation stamps from the
  environment, so a session that later edits a task cannot claim to have filed it.
- **Frontmatter.** `agent:` is written after `model` and before `spec`, omitted when
  absent, quoted on `source`'s rules so it round-trips byte for byte. `format.rs` beside
  `model`; `frontmatter.rs` unchanged.
- **JSON.** Every task object gains `agent`, `null` when absent: `Task` in `show` and
  `next`, `TaskSummary` in `list`, `ready`, `prime`, `tree`, and `ParkedRow`. Additive;
  no existing key changes.
- **Pretty output.** `show --pretty` prints the frontmatter line. Tables gain no column.
- **Completion.** `TASKS_AGENT` is not read at completion; `model` keeps its own
  variable and rules. The harness wiring below exports both from one source so the
  existing feature starts working without a second change.
- **Checks and pickers.** No `check` warning, no readiness or ordering effect. An absent
  `agent` is the accurate value for a task filed by a person or an unwired harness.

## Harness wiring

The CLI stores what it is given; each harness owns exporting it. These are pieces in
the projects that own them, blocking this goal (`tasks dep tasks-dc599b --on <piece>`):

- **Claude Code (ops).** The model lives in a state file, `<scratchpad_dir>/tasks-model`,
  and the exports resolve it at command time, so a switch takes effect on the next
  `tasks` invocation without touching the env file again:
  - One ops hook script, `hooks/claude-provenance`, registered for both events and
    dispatching on `hook_event_name`, keeps the existing notice hook single-purpose.
    On `SessionStart` it reads its stdin JSON. When `$CLAUDE_ENV_FILE` and
    `scratchpad_dir` are both present it writes `model` to the state file (or removes
    the file when the field is absent) and appends two lines to the env file:
    `export TASKS_MODEL="$(cat '<state>' 2>/dev/null)"` and
    `export TASKS_AGENT="claude-code${TASKS_MODEL:+/$TASKS_MODEL}"`. The preamble runs
    before each Bash command, so the substitution re-reads the file every time. When
    `scratchpad_dir` is absent it appends `export TASKS_AGENT=claude-code` and
    `export TASKS_MODEL=` — the empty export clears any model inherited from the
    launching shell, which the CLI reads as "no model". When `$CLAUDE_ENV_FILE` is
    absent it writes nothing.
  - On `PostModelSwitch` the same script writes `to_model` to the same state file. A payload with `scratchpad_dir` but no `to_model` removes the
    file, so the next command records the harness alone rather than the previous
    model. A missing `scratchpad_dir` writes nothing.
  - Both hooks keep the session-start contract: advisory, exit 0, never fail the
    session. Paths are shell-quoted.
  - **What `<model>` means under Claude Code:** the enclosing session's model, as
    SessionStart and PostModelSwitch report it. A subagent runs in the same session
    and its `tasks` commands carry the session's stamp even when the subagent is
    configured for another model. The field's contract is narrowed to say so for this
    harness; per-subagent attribution needs a signal the preamble can see and is
    deferred.
  - Acceptance is a live transition, not only the unit tests, and it must distinguish
    per-command evaluation from one-time initialisation: under model A, `tasks add`
    and confirm the stamp is A; `/model` to B; `tasks add` and confirm B; `tasks done`
    and confirm `model` is B. If the preamble proves not to be re-evaluated per
    command, fall back to `export TASKS_AGENT=claude-code` and `export TASKS_MODEL=`
    alone and record why in this spec.
- **Codex (ai).** Harness-only, because the effective model is not knowable from
  config: `codex/config.toml` sets `TASKS_AGENT = "codex"` through the shell
  environment policy and exports no `TASKS_MODEL`. Promote to `codex/<model>` when a
  Codex hook exposes the effective model; the session_start hook already configured is
  where that would go.
- **Other harnesses (skill instruction, this repo).** `skills/tasks/SKILL.md` adds one
  rule: when `TASKS_AGENT` is unset and the session knows its harness and model ids,
  pass `--agent <harness>/<model>` on `add`; when it knows only the harness, pass that;
  when unsure, pass nothing. For `feedback`, which has no flag, supply the variable on
  that invocation: `TASKS_AGENT=<harness>/<model> tasks feedback …`. The README documents
  the variable beside `TASKS_MODEL`. quick-add inherits this through `tasks add` and
  needs no change of its own.

## Deferred

- `list --agent` filter; `jq` over summary rows covers it, as for `model`.
- Harness auto-detection by the CLI (`CLAUDE_CODE_SESSION_ID` is already sniffed for
  claim identity, so a `claude-code` harness stamp without the model is possible). Kept
  out so the field means what the harness said, not what the CLI inferred; revisit if
  the hook cannot be made reliable.
- Per-note or per-edit attribution; notes already carry the owner identity.
- Retroactive stamping of existing records.
- Per-subagent attribution under Claude Code. The meaning is narrowed to the session's
  model above; `agent_id`/`agent_type` reach hooks, but the Bash preamble would need
  its own signal to record the harness alone for a subagent's commands.

## Testing

Mirroring the `model` cases in `tests/cli.rs` and the format units:

- `TASKS_AGENT=A tasks add` stamps A; `--agent B` with the variable set stamps B; neither
  present leaves the field absent; an empty variable is absent; a non-Unicode variable
  fails the `add` with an error naming it and writes nothing — and the same invalid
  variable with `--agent B` succeeds and stamps B, as does a multi-line variable.
- `TASKS_AGENT=A tasks feedback …` stamps A on the filed report.
- `TASKS_AGENT=A tasks edit <id> --title x` on an unstamped task leaves it unstamped.
- `edit --agent` replaces, `--no-agent` clears, both together conflict; an editor save
  with a multi-line `agent:` is rejected.
- Round-trip through `parse_task`/`serialize_task`, key order after `model`.
- JSON carries `agent` (`null` when absent) in `show`, `list`, `ready`, and `prime`'s
  parked rows; pretty `show` prints the line and tables are unchanged.
- `tests/common/mod.rs` scrubs `TASKS_AGENT` in both command builders alongside
  `TASKS_MODEL`.
- Ops hooks: unit tests feed session-start JSON with and without `model` and with and
  without `scratchpad_dir`, asserting the state file and the lines appended to a
  temporary `CLAUDE_ENV_FILE`, and that nothing is written when the variable is unset;
  then feed a PostModelSwitch payload and assert the state file changes. Every
  unknown-model path starts from an existing stamp: a state file already holding a
  model, and `TASKS_MODEL` already set in the hook's environment — session start
  without `model`, session start without `scratchpad_dir`, and a switch payload
  without `to_model` must each leave the next command with no model. A shell-level
  test sources the env file (with `TASKS_MODEL` pre-set) before and after the switch
  payload and asserts `TASKS_AGENT` and `TASKS_MODEL` follow it. The live transition
  above is the acceptance check for the piece; a shell test cannot prove the harness
  re-runs the preamble.
