---
id: tasks-146aee
title: show --pretty panics with a Rust broken-pipe backtrace when stdout is closed early (piped to head)
status: idea
priority: 2
created: 2026-09-06T17:44:05Z
updated: 2026-09-06T17:44:05Z
depends: []
tags: [feedback, friction, "from:dots"]
---

tasks show <id> --pretty | head -3 -> 'failed printing to stdout: Broken pipe (os error 32)' panic. Expected a silent exit like most CLIs; output itself was fine.
