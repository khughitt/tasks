---
id: tasks-e3fdce
title: Design binary compatibility from the brief
status: todo
priority: 2
size: s
complexity: high
process: planned
created: 2026-10-20T01:57:45Z
updated: 2026-10-20T01:57:45Z
depends: []
parent: tasks-83e0e1
tags: []
source: "docs/notes/2026-10-19-binary-compatibility-brief.md:design"
agent: codex
---

Why: [ref:brief] establishes that different record-reading capabilities still report 0.4.0 and explains how an unfamiliar timestamp can be reported as a quoting error. The original incident's exact binary and record remain unknown.

Done: a user-reviewed design at [ref:design] settles the minimum version/build information an operator needs, which compatibility changes require a version boundary and how that policy is checked, whether a format marker is justified, and diagnostic wording that preserves genuine corruption errors. Define checks for old-style unquoted timestamps, unknown keys, malformed known fields and version output; distinguish capability compatibility from exact build identity and state the coordinated rollout order. Account for binaries already deployed without the new diagnostic. Produce no implementation or host-state service in this task; implementation planning follows the reviewed design.

Where to start: read [ref:brief], then src/cli.rs, Cargo.toml, src/format.rs::parse_task/quote_timestamps, src/frontmatter.rs, src/error.rs, tests/cli.rs::cli_vocabulary_help_version_usage and README.md's Upgrading guidance. Historical evidence: 4a436e5, 5bf4280, c2b7095, 19974fe and 5472812; rollout outcomes: tasks-f0c643 and tasks-06be6f. Reuse that trace rather than adding a research child for it. Start with explicit release versions and qualified upgrade guidance; justify any additional build metadata or format marker against that option.

Ideas it wakes: on completion, run tasks note tasks-476fd7 --no-reviewed "<finding and design pointer>" and tasks note tasks-db7049 --no-reviewed "<finding and design pointer>" before this result's branch merges. Both ideas must receive the outcome; their dependency edges are recorded in this scope pass.

## References

- brief (input, required, kind=brief): docs/notes/2026-10-19-binary-compatibility-brief.md — version history, diagnostic constraints and decisions
- design (output, kind=spec): docs/specs/2026-10-19-binary-compatibility-design.md — reviewed binary identification and record-diagnostic contract
