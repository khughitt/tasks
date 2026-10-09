---
id: tasks-fa0276
title: Name the obstructing Git marker when linked discovery fails
status: doing
priority: 2
size: s
complexity: low
process: direct
owner: main
created: 2026-10-07T16:57:53Z
updated: 2026-10-09T00:38:48Z
started: 2026-10-09T00:38:48Z
depends: []
parent: tasks-a7460c
tags: [feedback, friction, "from:ops"]
agent: claude-code/claude-opus-5-5
---

Why: Failed linked discovery scans ancestor .git entries after Git says not a repository. Any marker causes GitTree::of to return io, but the diagnostic names only the starting directory, hiding the obstructing marker. An isolated current-CLI where probe reproduced success without a marker, io with an empty ancestor .git, and success after its removal. A healthy unborn repository succeeded; corrupt HEAD also produced io and the same Git not-a-repository text.

Chosen scope: deliver the original report's explicit-diagnostic alternative. Keep the conservative error behavior; include the exact marker path found by src/link.rs::GitTree::of and actionable guidance to inspect/repair it, distinguishing a confirmed empty directory from unreadable or otherwise broken metadata if the message claims emptiness. Never tell the user to delete unknown Git metadata. Do not silently classify every failed discovery or empty-looking repository as no project.

Done: regression checks assert kind io, the marker path rather than only the starting child, and clear inspection/recovery guidance for an empty ancestor .git; no-marker and healthy-unborn cases retain no-project behavior; malformed HEAD/gitfile and filesystem errors remain errors and failed init does not write config or registry entries. Exercise the shared helper through where plus the existing linked-init protection tests. Use just test-one with the relevant filter, then just test-fast and just check.

Where to look: src/link.rs::GitTree::of (marker branch introduced in 7040de3), src/repo.rs::locate, src/commands/where_.rs, src/commands/init.rs, src/commands/parked.rs and linked-discovery tests in tests/cli.rs. tasks-084a9f separately owns discovery ceilings; coordinate shared-file changes without widening this task. docs/notes/2026-10-08-project-discovery-movement-brief.md records the evidence and alternative.

Limits: the scratch probe exercised where only, not init, feedback, or the entire suite. The reported host process recreating the marker was not investigated.

Original report (preserved):
git reports 'not a git repository' for a directory under TMPDIR when TMPDIR itself holds an empty .git directory (something on the host recreates one there every minute or two); the link lookup's marker scan sees the .git entry and turns the miss into an io error, so init, feedback and the whole test suite fail under TMPDIR. Expected: a .git entry that git itself does not recognise as a repository (an empty directory) is no project, or the error names the stray path and how to clear it.

## Notes

- 2026-10-08T10:41:44Z (main): scope: scoped; reproduced empty-marker failure with no-marker, removed-marker, healthy-unborn and corrupt-HEAD controls; chose the requested actionable diagnostic rather than weakening corruption refusal; P2/s/low/direct; parent tasks-a7460c; ceiling semantics stay with tasks-084a9f; brief: docs/notes/2026-10-08-project-discovery-movement-brief.md
- 2026-10-09T00:38:48Z (main): started
  provenance: {"harness_session":"claude-code:4b1d7eef-4783-40e0-b224-bba892d8a7e3","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
