---
id: tasks-34424b
title: Establish the detached-record migration recovery contract
status: todo
priority: 2
size: m
complexity: mid
process: direct
created: 2026-10-24T11:15:26Z
updated: 2026-10-24T11:15:26Z
depends: []
parent: tasks-72e3d7
tags: []
source: "tasks-e9ce63+tasks-13d935+tasks-4e656c:recovery-contract"
agent: codex
---

Question: What is the smallest supported recovery contract for forced detached-checkout relocation, discovery during an unpaired host window, and init configuration writes that remain uncommitted?
Where to start: [ref:brief] preserves all three reports and the traced paths at eedf9a9. Read src/commands/init.rs, src/repo.rs::Project::init_detached, src/scope.rs::registry_warnings, src/commands/projects.rs, src/commands/where_.rs and src/cli.rs::Command::commit_verb, plus the records-home and record-commit designs. Coordinate the prose outcome with tasks-d6f0ed and avoid duplicating tasks-5aebd6 or tasks-7b1b22.
Bound: Trace those paths and use one disposable lifecycle fixture matrix: unchanged pairing/second host, healthy unpaired checkout (projects and explicit where on another path), moved checkout with old path absent, unrelated existing checkout, fresh and modified records config with an unrelated staged file. Compare calls from inside and outside the unpaired checkout. Isolate registry, state and Git config; build subprocess environments key by key and assert fixture Git roots before writes. No production changes, real migrations, host-pointer changes or cross-host rollout. Report an unreproduced where symptom separately; do not infer its cause from projects.
Expected result: Record exact invocations, observed errors, config/registry diffs and commit/index effects on this task and in [ref:brief]. Recommend a separately checkable disposition for both config and registry relocation, both discovery commands, and commit-or-explicit-warning behavior for new and modified config. Explain conflicts with the approved designs, negative controls and any unknowns; identify any later reviewed design needed. Do not call a documentation correction coverage for an unaddressed behavior request.
Ideas it wakes: On completion, before this result's branch merges, run tasks note tasks-e9ce63 --no-reviewed "<relocation finding>", tasks note tasks-13d935 --no-reviewed "<projects and where findings>", and tasks note tasks-4e656c --no-reviewed "<configuration commit or warning recommendation>".

## References

- brief (input, required, kind=brief): docs/notes/2026-10-24-detached-migration-recovery-brief.md — all reported asks, code evidence and investigation limits
