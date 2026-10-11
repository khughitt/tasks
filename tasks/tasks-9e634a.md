---
id: tasks-9e634a
title: "The task CLI launches an interactive editor for a flagless edit even when the automation caller uses pipes, so an accidental invocation can stall silently instead of refusing with an actionable message."
status: idea
priority: 2
created: 2026-10-11T12:31:33Z
updated: 2026-10-11T12:31:33Z
depends: []
tags: [feedback, friction, "from:ops"]
agent: codex
---

The caller mistakenly omitted edit flags; the CLI then launched the configured editor from a piped subprocess and waited indefinitely without output. Explicit edit flags are the recovery. Consider refusing interactive editor mode when the caller has no usable terminal, with a diagnostic naming editor mode and the required explicit flags.
