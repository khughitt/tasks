---
id: tasks-9a9ef3
title: Trim the tasks skill to what tasks --help cannot supply
status: todo
priority: 2
size: s
complexity: mid
process: direct
created: 2026-09-16T14:27:13Z
updated: 2026-09-30T10:10:19Z
depends: []
tags: [skills]
source: ai-45934e
agent: "claude-code/claude-opus-5[1m]"
---

The ai provenance audit (ai doc/instruction-provenance.md, rows T3, T7, T13, T17, T19, T28) found ~700 of the skill's 3271 words (2026-09-16) describe fixed CLI behaviour that the warnings and --help already state: prefix routing, claim mechanics, stamp variables, the edit flag inventory, the picker-hiding inventory, prefix-rename recovery. Keep the imperatives (never pick an idea, never edit tasks/*.md, never guess --agent, the parked-idea exception, the escalation rules); replace each mechanism paragraph with one sentence and a pointer. Done when the skill loses those words and a fresh session still completes the protocol on a scratch project.

The skill is 3904 words as of 2026-09-24. Of the sections added since the audit, the same pass trims Lifecycle provenance (209 words of mechanism; the README holds the contract) to one sentence and a pointer, and keeps the quiet-run `run:` note format (tools parse it) and the host-pointer rule (no CLI states it) as they are. The pass then adds rows for those three sections to the ai audit so it stays complete; it does not wait on a fresh audit.

## Scope and acceptance

Current evidence: skills/tasks/SKILL.md is 4317 words on 2026-09-30. The six mechanism areas and Lifecycle provenance remain; tasks edit --help already lists the editable flags, and README.md documents lifecycle provenance and rename recovery. The external audit and ai-45934e were not independently read in this checkout; its findings above remain captured source evidence, not fresh verification.

Done: trim the seven areas already selected in the 2026-09-24 decision, replacing mechanism text only where current command help or a precise README/spec link actually supplies it. Keep the protocol imperatives, claim-override cautions, escalation behavior, quiet-run note format, and host-pointer rule. Record before/after word counts rather than treating the old 700-word estimate as a quota. Preserve the audit back-fill requested above; verify its current location before editing it.

Check: in a fresh agent session using the revised skill, run one disposable project's prime, add, ready, start, note, park, resume, done, and check sequence, and confirm an idea is not offered as executable work. Isolate both XDG_CONFIG_HOME and XDG_STATE_HOME. Record the session result and verify every new pointer resolves. Where to look: skills/tasks/SKILL.md, README.md, current command help, and the source audit. Coordinate README anchors with tasks-03bfc3. Process direct follows the recorded decision; no new behavior or workflow design is needed.

## Notes

- 2026-09-24T13:07:13Z (main): curate: refined; dated the 3271-word baseline, recorded current 3904 words and the unaudited sections, set size s and complexity mid (bounded editorial choices, clear done check)
- 2026-09-24T13:35:22Z (main): Decided: trim the six audited rows plus the Lifecycle provenance section in one pass; keep the quiet-run note format and the host-pointer rule; back-fill the ai audit rows afterwards rather than blocking on them.
- 2026-09-30T10:10:19Z (main): scope: scoped; retained the recorded seven-area trim decision and source, refreshed the baseline to 4317 words, and specified an isolated fresh-session check; external audit remains to verify during execution; p2/s/mid, direct
