# Tag dictionary: named meanings, and the path from a tag to a field

Status: implemented (2026-09-11)
Task: tasks-603ef9

## 1. Problem

Tags are free strings. `tasks tags` shows which are in use and how often, which is how a
vocabulary gets *chosen*, but nothing records what a tag means, so two projects drift on
one word and a new metadata idea has nowhere cheaper to start than a field. Every field
so far (`source`, `model`, `every`, the park reason) went straight to a design, a schema
change, and a JSON contract change, which is the right cost for a field that has earned
it and the wrong cost for one being tried.

## 2. Decision

An optional `[tags]` table in `tasks/.config.toml`, tag → one-line meaning. It is the
project's dictionary: `tasks tags` prints the meaning beside each tag, and `tasks check`
reports an open task carrying a tag the dictionary does not define. A project without
the table is not held to anything.

The table has the same shape as the `[tags]` table in ops `skills/quick-add/routing.toml`
(the shared meanings quick-add routes by), so a project can seed its own from it.

**The promotion path is a convention, not machinery.** A metadata idea starts as a tag
plus a dictionary entry — cheap, reversible, visible in `tasks tags`. When it proves
useful enough to need a value, validation, or a JSON key, it becomes a field through the
usual design; its dictionary entry then says so ("promoted to `<field>` on <date>; do not
tag"), and `check`'s finding is what flushes the tag off the open tasks still carrying
it. `park --reason` is the precedent for the last step taken directly, when the
vocabulary was already known.

## 3. Rules

- **Config.** `[tags]` is optional. Each key must be a valid tag and each value a
  non-empty single line, checked when the project opens; a bad entry is a config error
  naming the entry (`tasks/.config.toml: [tags] "x": meaning must not be empty`), never a
  skipped one. `init` writes no table.
- **`tags`.** Every row gains `meaning`, `null` when no dictionary defines the tag. In
  local scope it is that project's entry; under `--all-projects` it is the entry of the
  first registered project that defines the tag, so a cross-project listing reads one
  meaning per tag rather than a map. Pretty output prints the meaning after the tag.
- **`check`.** When the project has a table, each *open* task's tags are looked up and an
  undefined one is a warning of kind `undefined_tag`, one per tag per task, naming the
  tag and the config file. Closed tasks are not checked: their tags are history. Without
  a table, `check` says nothing about tags.
- **JSON.** Additive: `meaning` on `TagRow`. Nothing else changes shape.

## 4. Deferred

- **Pattern tags.** The tasks project's own feedback tags are `from:<prefix>`, a pattern
  the exact-key dictionary cannot express; a project using them either lists each or
  keeps no dictionary. A glob or prefix entry waits until a second pattern appears.
- **Unused entries.** A dictionary entry no open task carries is not reported. It is
  either a tag being tried or a promoted one kept as a tombstone, and `check` cannot
  tell which.
- **A shared dictionary.** Each project owns its table. If the same meanings keep being
  copied between projects, ops can render them the way it renders `identity.toml`.

## 5. Testing

Unit: a config with a table parses into `Project::tags`; without one it is `None`; an
empty meaning is a config error naming the entry. End to end: `tags` rows carry the
meaning locally and take the first registered project's under `--all-projects`; pretty
output prints it; `check` warns `undefined_tag` for an open task and not for a closed
one, and reports nothing when there is no table; a malformed table fails `tags` with a
config error.
