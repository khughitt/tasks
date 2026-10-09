---
id: tasks-d18278
title: Correct whole-branch task move review findings
status: todo
priority: 2
size: m
complexity: high
process: direct
created: 2026-10-09T07:25:24Z
updated: 2026-10-09T07:25:24Z
depends: []
parent: tasks-7ba557
tags: []
agent: codex
spec: docs/specs/2026-10-08-task-move-design.md
---

Fix final review I1–I6 in .superpowers/sdd/2026-10-09-task-move/final-review.md: monotonic move stamps; valid historical deferral; pending-parent and resumed-child guards; preserve changed source attachments/stores; coherent journal contents and park/escalation pairs; pointer-aware local halt remedies. Test each RED→GREEN; full fast/check, reinstall, close in code commit. Do not fold deferred Minor M1–M5 into corrective rounds.
