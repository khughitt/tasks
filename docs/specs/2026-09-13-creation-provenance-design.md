# Creation provenance: record the agent that filed a task

Status: draft (2026-09-13)
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
- **Claude Code can supply the value.** Its `SessionStart` hook input may carry `model`
  (not guaranteed), and a hook can write `export …` lines to `$CLAUDE_ENV_FILE`, which
  runs as a preamble before every Bash command. ops already owns a session-start hook.
  Codex has no session hook; its config can set static environment variables. Crush and
  other harnesses are unknown.

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
  otherwise `TASKS_AGENT` from the environment; otherwise nothing. Unset or empty
  `TASKS_AGENT` records nothing. A non-Unicode value is a validation error naming the
  variable, never a silent skip — the `completion_model()` shape, factored so both
  variables share one reader.
- **Write path.** The stamp lives in `add::blank()`, the one constructor every created
  record passes through: `tasks add` (and therefore quick-add), and `tasks feedback`.
  `feedback` gains no flag; it is harness-driven by design and its reports must stay
  free of project detail, which a hand-typed value could carry. `blank()` reads the
  environment; `add` applies the flag afterwards through `FieldArgs` so the flag wins.
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

- **Claude Code (ops).** `hooks/claude-sessionstart` reads its stdin JSON and, when
  `$CLAUDE_ENV_FILE` is set, appends `export TASKS_AGENT=claude-code/<model>` and
  `export TASKS_MODEL=<model>` when `model` is present, and
  `export TASKS_AGENT=claude-code` alone when it is not. The hook keeps its contract:
  advisory, never fails session start, exit 0. Values are shell-quoted.
- **Codex (ai).** `codex/config.toml` sets `TASKS_AGENT = "codex/<model>"` through the
  shell environment policy, kept in step by hand with the `model` key beside it; the
  same block sets `TASKS_MODEL`. Codex has no documented per-session hook to derive it from.
- **Other harnesses (skill instruction, this repo).** `skills/tasks/SKILL.md` adds one
  rule: when `TASKS_AGENT` is unset and the session knows its harness and model ids,
  pass `--agent <harness>/<model>` on `add`; when it knows only the harness, pass that;
  when unsure, pass nothing. The README documents the variable beside `TASKS_MODEL`.
  quick-add inherits this through `tasks add` and needs no change of its own.

## Deferred

- `list --agent` filter; `jq` over summary rows covers it, as for `model`.
- Harness auto-detection by the CLI (`CLAUDE_CODE_SESSION_ID` is already sniffed for
  claim identity, so a `claude-code` harness stamp without the model is possible). Kept
  out so the field means what the harness said, not what the CLI inferred; revisit if
  the hook cannot be made reliable.
- Per-note or per-edit attribution; notes already carry the owner identity.
- Retroactive stamping of existing records.

## Testing

Mirroring the `model` cases in `tests/cli.rs` and the format units:

- `TASKS_AGENT=A tasks add` stamps A; `--agent B` with the variable set stamps B; neither
  present leaves the field absent; an empty variable is absent; a non-Unicode variable
  fails the `add` with an error naming it and writes nothing.
- `TASKS_AGENT=A tasks feedback …` stamps A on the filed report.
- `TASKS_AGENT=A tasks edit <id> --title x` on an unstamped task leaves it unstamped.
- `edit --agent` replaces, `--no-agent` clears, both together conflict; an editor save
  with a multi-line `agent:` is rejected.
- Round-trip through `parse_task`/`serialize_task`, key order after `model`.
- JSON carries `agent` (`null` when absent) in `show`, `list`, `ready`, and `prime`'s
  parked rows; pretty `show` prints the line and tables are unchanged.
- `tests/common/mod.rs` scrubs `TASKS_AGENT` in both command builders alongside
  `TASKS_MODEL`.
- Ops hook: a unit test feeding stdin JSON with and without `model`, asserting the lines
  written to a temporary `CLAUDE_ENV_FILE`, and that no file is touched when the
  variable is unset.
