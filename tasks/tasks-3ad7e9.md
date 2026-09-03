---
id: tasks-3ad7e9
title: Fix clippy lints so clippy -D warnings can be a gate
status: todo
priority: 2
size: xs
created: 2026-09-03T02:39:07Z
updated: 2026-09-03T02:39:07Z
depends: []
tags: [hygiene]
---

cargo clippy --all-targets -- -D warnings fails on three pre-existing lints (two collapsible if statements, one large enum variant size difference). Fix them, then remove the pending note from the Gates section of AGENTS.md.
