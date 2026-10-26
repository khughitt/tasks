---
id: tasks-d6f0ed
title: Clarify that automatic record commits exclude configuration writes
status: done
priority: 3
size: xs
complexity: low
process: direct
owner: docs/readme-docs
created: 2026-10-10T17:51:21Z
updated: 2026-10-26T23:29:54Z
started: 2026-10-26T23:26:15Z
completed: 2026-10-26T23:29:54Z
closed: 2026-10-26T23:29:54Z
depends: []
tags: [docs]
source: "tasks-6939f6:final-review:M2"
agent: codex
---

Deferred final-review Minor M2 from tasks-6939f6: README and the tasks skill include configuration in the automatic-write commit list, but the journal records task records and attachment changes, and project administration has no automatic commit verb. Remove configuration from the automatic list or explicitly assign it to the tasks commit backlog sweep. Verify both prose surfaces against the engine.

## Notes

- 2026-10-26T21:21:35Z (main): curate-docs: tasks-d5ddb4 at c1bbcae confirms: criterion 9 finds README.md Record commits incorrectly includes configuration in automatic record commits. src/cli.rs Command::commit_verb excludes init, rename, retire, unretire and unregister; src/written.rs and the write call sites journal task records and attachments. This task owns the README correction and corresponding skill wording.
- 2026-10-26T23:26:15Z (main): started
  provenance: {"harness_session":"claude-code:ff0bbf59-4077-4302-b69b-3dac4260dec2","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-26T23:26:19Z (docs/readme-docs): resumed
  provenance: {"harness_session":"claude-code:ff0bbf59-4077-4302-b69b-3dac4260dec2","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-26T23:29:54Z (docs/readme-docs): done
  provenance: {"harness_session":"claude-code:ff0bbf59-4077-4302-b69b-3dac4260dec2","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-26T23:29:54Z (docs/readme-docs): README Record commits and the tasks skill now say each write commits its records and attachment changes only; init, rename, retire and unretire leave tasks/.config.toml for the user or the tasks commit sweep (matches Command::commit_verb and the written.rs journal callers)
  provenance: {"harness_session":"claude-code:ff0bbf59-4077-4302-b69b-3dac4260dec2","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
