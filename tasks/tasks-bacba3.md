---
id: tasks-bacba3
title: Adopt the ops act design's front door
status: todo
priority: 2
size: s
complexity: low
process: direct
created: 2026-09-28T10:28:33Z
updated: 2026-09-28T10:28:33Z
depends: []
tags: [testing]
source: ops-a5a7ef
agent: claude-code/claude-opus-5-5
---

ops docs/specs/2026-09-28-test-ci-act-design.md §7.3. Copy templates/justfile's test-one, the docs-only pre-commit path and the CI-aware pre-push gate (ci_suite_refs, ci_remote, push_fast_cmd, hook-pre-push-fast) with both templates/githooks; set ci_suite_refs from what CI runs the full suite for and note it; AGENTS.md Gates line per templates/AGENTS.md. Move `test-fast [<name>]`'s filter to `test-one`; `test-fast` takes no arguments afterwards. No alias.
