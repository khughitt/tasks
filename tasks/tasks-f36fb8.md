---
id: tasks-f36fb8
title: Amend the records-home design obs row for historical authorship exclusions
status: todo
priority: 2
size: xs
complexity: low
process: direct
created: 2026-10-23T14:00:10Z
updated: 2026-10-23T14:00:10Z
depends: [obs-7348a9]
tags: [docs]
source: obs-7348a9
agent: codex
---

After obs-7348a9 lands, amend the obs authorship row in records-home section 4 and any matching summary in section 9 to describe the implemented historical attribution contract, rather than saying detached and linked projects exclude nothing. Proposed spec r2 excludes separate records writes, excludes tasks/, docs/specs/, docs/plans/, docs/notes/, docs/superpowers/ and docs/designs/ plus own documents in all code trees, and treats unproven location classes as unresolved. Verify the approved obs spec and landed code before editing; r2 is still awaiting human review. Keep live flow fingerprint semantics distinct. Carry the prerequisite that obs lands before the first non-pilot project detaches; trial projects hq and obs detach after 2027-02-16. This is the docs follow-through from spec review round 2 finding 7, analogous to tasks-ea4cc5 for hq.

## References

- records-home (input, required, kind=spec): docs/specs/2026-10-22-records-home-design.md
