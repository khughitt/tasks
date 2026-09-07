# Task source: an opaque origin reference

Status: implemented (2026-09-06); the two deferrals below landed 2026-09-07
Task: tasks-13a0b6; follow-ups tasks-a22dd9, tasks-cc41e7

## Problem

Tasks are filed from somewhere: a pasted list of notes, an email, an issue in another
tracker, a message. Once filed, nothing on the record says where it came from, so "what did
I file from this note" is unanswerable and re-filing the same note twice is undetectable.
Writing the origin into the body is prose: it cannot be queried and drifts.

## Decision

One optional field, `source`, holding an opaque reference to the task's origin. tasks stores
it, prints it, and returns it in JSON. It never interprets the value, resolves it, or
checks that it points at anything. Reachability of an external reference is not the
tracker's business.

The documented convention is a scheme prefix so readers can tell origins apart:
`https://example.org/issues/7`, `mail:<42@example.org>`, `note:<id>`. Convention only; no
validation beyond the rule below.

## Rules

- **Value.** A non-empty, single-line string. Empty and multi-line values are validation
  errors at every write path (`add --source`, `edit --source`, and the editor path), and a
  parse error when read from a file. Same rule and same error text shape as `title` and
  `step`.
- **Frontmatter.** `source:` is written after `tags` and before `spec`, omitted when
  absent. The writer quotes it whenever the frontmatter subset requires (a `:` in the
  value, which most references have), and the reader strips the quotes, so the value
  round-trips byte-for-byte.
- **JSON.** Every task object gains `source`, `null` when absent: the full `Task` in
  `show` and `next`, and the summary rows in `list`, `ready`, `prime`, and `tree`. This
  is an additive contract change; no existing key changes.
- **CLI.** `tasks add --source <ref>` sets it. `tasks edit --source <ref>` replaces it;
  `tasks edit --no-source` clears it; the two conflict. Flag completion is derived from
  clap, so `--source` completes with no completion code.
- **Pretty output.** `show --pretty` prints the frontmatter line. Tables do not gain a
  column; the JSON carries the field.
- **Filter.** `tasks list --source <ref>` keeps only tasks whose source equals `<ref>`
  byte for byte. Exact, like the field itself: no prefix, substring, or case-folded
  matching, and no interpretation. It combines with the other `list` filters and with
  both read scopes. `ready` does not gain it; readiness is about what can be worked on,
  not where it came from.
- **Duplicate check.** `tasks add --source <ref>` is idempotent. If the target project
  already holds a task with that exact source *and* that exact title, in any status, the
  add returns that id with `action: "reused"` and a warning, and writes nothing. Both
  halves of the key are what a caller can reproduce byte for byte; nothing fuzzy, and no
  merging of the ignored flags. This makes rerunning a batch filed from one origin safe
  for every caller, which is why it lives in the tracker rather than in each of them.

  Deferred (2026-09-06) and landed 2026-09-07 once the first real batches showed exact
  title equality was the right key.
