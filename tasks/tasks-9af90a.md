---
id: tasks-9af90a
title: check prints nothing on a clean tree by default; drop --quiet
status: doing
priority: 2
size: s
complexity: low
process: direct
owner: main
created: 2026-09-19T12:52:22Z
updated: 2026-09-19T12:52:22Z
started: 2026-09-19T12:52:22Z
depends: []
tags: []
agent: "claude-code/claude-opus-5[1m]"
---

tasks-597472 added -q. Every caller wants it (each project's hook), no consumer parses check's stdout, and a flag would have to be rolled into 17 justfiles and would break older binaries on other hosts. Make JSON mode print nothing when both lists are empty and remove the flag. --pretty keeps printing `ok`: it exists for a person at a terminal who cannot see the exit status; JSON is the hook and agent contract, where the exit status carries the verdict. Update the tests that parse a clean check, README, and the skill.

## Notes

- 2026-09-19T12:52:22Z (main): started
  provenance: {"harness_session":"claude-code:3111d755-79c1-4c78-8c1a-8c2478835ee8","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
