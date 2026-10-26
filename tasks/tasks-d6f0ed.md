---
id: tasks-d6f0ed
title: Clarify that automatic record commits exclude configuration writes
status: todo
priority: 3
size: xs
complexity: low
process: direct
created: 2026-10-10T17:51:21Z
updated: 2026-10-26T21:21:35Z
depends: []
tags: [docs]
source: "tasks-6939f6:final-review:M2"
agent: codex
---

Deferred final-review Minor M2 from tasks-6939f6: README and the tasks skill include configuration in the automatic-write commit list, but the journal records task records and attachment changes, and project administration has no automatic commit verb. Remove configuration from the automatic list or explicitly assign it to the tasks commit backlog sweep. Verify both prose surfaces against the engine.

## Notes

- 2026-10-26T21:21:35Z (main): curate-docs: tasks-d5ddb4 at c1bbcae confirms: criterion 9 finds README.md Record commits incorrectly includes configuration in automatic record commits. src/cli.rs Command::commit_verb excludes init, rename, retire, unretire and unregister; src/written.rs and the write call sites journal task records and attachments. This task owns the README correction and corresponding skill wording.
