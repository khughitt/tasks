---
id: tasks-f36fb8
title: Amend the records-home design obs row for historical authorship exclusions
status: doing
priority: 2
size: xs
complexity: low
process: direct
owner: docs/records-home-obs-authorship
created: 2026-10-23T14:00:10Z
updated: 2026-10-23T16:51:30Z
started: 2026-10-23T16:46:43Z
depends: [obs-7348a9]
tags: [docs]
source: obs-7348a9
agent: codex
---

obs-7348a9 landed locally in obs main at 9eb4f982cd60c70d6494c3278caec0cc805108ec; independent review accepted with no findings and the merged gate passed 1223 tests. Amend the obs authorship row in records-home section 4 and any matching summary to describe the implemented historical attribution contract, rather than saying detached and linked projects exclude nothing. Approved spec r4 retains the path-only design: exclude tasks/, docs/specs/, docs/plans/, docs/notes/, docs/superpowers/ and docs/designs/ plus the task own spec/plan uniformly across projects. No location classification, unknown location class, whole-overlay exclusion, or project-layout lookup is added to derivation. Overlay guides and tools outside those prefixes remain eligible. v4 also rejects stale authorship/claims versions. Its measured document-author loss and claim fallback to several are recorded in the obs spec and E0 note for the 2026-10-23 14:00 UTC derivation; declared-output exceptions remain outside v4. Verify approved spec r4 at ba84f7b and the landed code before editing; both spec and implementation plan were approved, and obs-7348a9 is done. Keep historical attribution distinct from the live flow fingerprint. Carry the prerequisite that obs lands before the first non-pilot project detaches; trial projects hq and obs detach after 2027-02-16. This is the source-design follow-through from round 2 finding 7; dependency on obs-7348a9 remains.

## References

- records-home (input, required, kind=spec): docs/specs/2026-10-22-records-home-design.md

## Notes

- 2026-10-23T16:18:18Z (main): obs-7348a9 implementation landed locally in obs main at 9eb4f982cd60c70d6494c3278caec0cc805108ec after independent review acceptance (0 findings). The six-prefix path-only rule and paired attribution-version refusal are implemented; verify this landed code and approved spec r4 at ba84f7b before updating records-home section 4. Post-merge obs gate is running; parent completion will release the dependency. No migration or live index run was performed.
- 2026-10-23T16:46:43Z (main): started
  provenance: {"harness_session":"claude-code:049fb7ab-86eb-4014-8cfb-ff365280b5ff","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-23T16:48:28Z (docs/records-home-obs-authorship): resumed
  provenance: {"harness_session":"claude-code:049fb7ab-86eb-4014-8cfb-ff365280b5ff","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-23T16:51:30Z (docs/records-home-obs-authorship): baseline: just test in .worktrees/tasks-f36fb8 passed 1663 tests (0 failed). Verified obs spec r4 at ba84f7b (user approval 2026-10-23T15:21Z on obs-7348a9) and landed code at 9eb4f98 in obs main: authorship.EXCLUDED holds the six prefixes, VERSION authorship-v4, CLAIMS_VERSION claims-v4, outcome_report refuses missing or unequal versions with run obs index.
