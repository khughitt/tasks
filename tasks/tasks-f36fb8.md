---
id: tasks-f36fb8
title: Amend the records-home design obs row for historical authorship exclusions
status: todo
priority: 2
size: xs
complexity: low
process: direct
created: 2026-10-23T14:00:10Z
updated: 2026-10-23T15:07:40Z
depends: [obs-7348a9]
tags: [docs]
source: obs-7348a9
agent: codex
---

After obs-7348a9 lands, amend the obs authorship row in records-home section 4 and any matching summary to describe the implemented historical attribution contract, rather than saying detached and linked projects exclude nothing. Proposed spec r4 retains the path-only design: exclude tasks/, docs/specs/, docs/plans/, docs/notes/, docs/superpowers/ and docs/designs/ plus the task own spec/plan uniformly across projects. No location classification, unknown location class, whole-overlay exclusion, or project-layout lookup is added to derivation. Overlay guides and tools outside those prefixes remain eligible. v4 also rejects stale authorship/claims versions. Its measured document-author loss and claim fallback to several are recorded in the obs spec and E0 note for the 2026-10-23 14:00 UTC derivation; declared-output exceptions remain outside v4. Verify the approved obs spec and landed code before editing; r4 still requires human review. Keep historical attribution distinct from the live flow fingerprint. Carry the prerequisite that obs lands before the first non-pilot project detaches; trial projects hq and obs detach after 2027-02-16. This is the source-design follow-through from round 2 finding 7; dependency on obs-7348a9 remains.

## References

- records-home (input, required, kind=spec): docs/specs/2026-10-22-records-home-design.md
