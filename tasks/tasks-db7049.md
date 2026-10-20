---
id: tasks-db7049
title: "tasks --version cannot tell a host's binary from main's: it stays 0.4.0 across record-format changes"
status: idea
priority: 2
reviewed: true
reviewed_at: 2026-10-20T01:58:16Z
created: 2026-10-19T18:53:30Z
updated: 2026-10-20T01:58:16Z
depends: [tasks-e3fdce]
parent: tasks-83e0e1
tags: []
source: tasks-f0c643
agent: claude-code/claude-opus-5-5
---

The task-references rollout had to confirm every host ran the new binary before any record gained a References section. Europa's pre-merge build and titan's post-merge build both print 'tasks 0.4.0', so the check fell back to probing a feature (show JSON carries inventory). The README's own upgrade advice (run tasks --version for each tasks on PATH) cannot detect this either. The wait-aware rollout (tasks-06be6f) has the same 'every host runs X' gate. Options: bump the version on every record-format or JSON-shape change, embed the build commit in --version, or record each host's binary build in shared state so prime or check warns when another host lags main.

## References

- brief (input, required, kind=brief): docs/notes/2026-10-19-binary-compatibility-brief.md — version-boundary evidence, completed rollout outcomes and alternatives

## Notes

- 2026-10-19T18:53:37Z (main): concerns: tasks-f0c643 extension — host rollouts need a way to confirm each host's binary build
- 2026-10-20T01:58:16Z (main): scope: briefed; installed CLI and Cargo.toml print 0.4.0, unchanged across wait-aware 19974fe and references 5472812 after 4a436e5; tasks-f0c643 and tasks-06be6f record completed upgrades using inventory feature checks; prefer explicit release versions over shared host tracking, with policy and diagnostic contract unresolved; preserved source and capture; wait on design tasks-e3fdce under tasks-83e0e1; brief: docs/notes/2026-10-19-binary-compatibility-brief.md
