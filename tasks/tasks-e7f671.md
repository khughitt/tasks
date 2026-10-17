---
id: tasks-e7f671
title: Record whether pi passes its session id to tool subprocesses
status: todo
priority: 3
size: xs
complexity: mid
process: direct
created: 2026-10-17T02:37:12Z
updated: 2026-10-17T02:37:12Z
depends: []
tags: [provenance]
source: docs/notes/2026-10-16-runner-provenance-brief.md
agent: claude-code
---

Question: Does pi (1.1.0) pass its session id to the subprocesses its tools run, under which environment variable, and does `pi --session-id <id>` make that value the id the caller chose?
Where to start: docs/notes/2026-10-16-runner-provenance-brief.md (alternative 1); `pi --help` (`--session-id`, `--session-dir`, `--no-session`); pi's bash/tool spawn code in its installed checkout (resolve `readlink -f "$(command -v pi)"`); src/provenance.rs for the shape a native source takes.
Bound: Read pi's tool-spawn code for the environment it passes. If the code is not conclusive, run one `pi --session-id <uuid>` turn whose tool call prints `env`, which needs a model call, so ask the user first. No tasks code change.
Expected result: Record the variable name (or its absence) and whether `--session-id` controls it on this task and in the brief's Current behaviour section, with a recommendation between the brief's alternatives 1 and 2.
Ideas it wakes: On completion, run `tasks note tasks-b13c2c --no-reviewed "<finding>"` before this result's branch merges.
