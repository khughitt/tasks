---
id: tasks-476fd7
title: An older tasks binary reading records written by a newer one reports a malformed record (bare scalar contains a reserved character; quote it) instead of saying the binary is older than the record format
status: idea
priority: 2
reviewed: true
reviewed_at: 2026-10-20T01:58:03Z
created: 2026-10-19T09:19:07Z
updated: 2026-10-20T01:58:03Z
depends: [tasks-e3fdce]
parent: tasks-83e0e1
tags: [feedback, friction, "from:hq"]
agent: claude-code/claude-opus-5-5
---

A host with a stale binary ran tasks projects --paths through another tool; the parse error named a record line and told the reader to quote it, which reads as a corrupt record. Expected: an error naming a format newer than this binary and the upgrade route.

## References

- brief (input, required, kind=brief): docs/notes/2026-10-19-binary-compatibility-brief.md — timestamp-parser evidence and honest upgrade-diagnostic alternatives

## Notes

- 2026-10-20T01:58:03Z (main): scope: briefed; 4a436e5 lacks reviewed_at/closed timestamp quoting, added by 5bf4280 and shared by c2b7095, so reserved-character failure can precede unknown-key detection; original binary/key/record remain unknown, not reproduced; retained strict parsing and source text; wait on design tasks-e3fdce under tasks-83e0e1; brief: docs/notes/2026-10-19-binary-compatibility-brief.md
