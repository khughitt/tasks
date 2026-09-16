---
id: tasks-9a9ef3
title: Trim the tasks skill to what tasks --help cannot supply
status: idea
priority: 2
created: 2026-09-16T14:27:13Z
updated: 2026-09-16T14:27:13Z
depends: []
tags: [skills]
source: ai-45934e
agent: "claude-code/claude-opus-5[1m]"
---

The ai provenance audit (ai doc/instruction-provenance.md, rows T3, T7, T13, T17, T19, T28) finds ~700 of the skill's 3271 words describe fixed CLI behaviour that the warnings and --help already state: prefix routing, claim mechanics, stamp variables, the edit flag inventory, the picker-hiding inventory, prefix-rename recovery. Keep the imperatives (never pick an idea, never edit tasks/*.md, never guess --agent, the parked-idea exception, the escalation rules); replace each mechanism paragraph with one sentence and a pointer. Done when the skill loses those words and a fresh session still completes the protocol on a scratch project.
