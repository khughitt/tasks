---
id: tasks-79a907
title: Cover inherited Git selectors and cleanup in record-commit fixture tests
status: todo
priority: 3
size: s
complexity: low
process: direct
created: 2026-10-22T23:48:29Z
updated: 2026-10-22T23:48:29Z
depends: []
tags: [testing]
source: tasks-f23cd5
agent: codex
---

Deferred Minor from implementation review of a92324edb974b190fb4e61ec97e139362b35d1e5: the current runner routes discovery, staging and cleanup correctly, but the hostile-global-hook regression exercises only a successful commit and no inherited repository selectors. Extend the child test with GIT_DIR, GIT_WORK_TREE and GIT_INDEX_FILE pointing exclusively at owned temporary fixtures; exercise refused-commit cleanup and assert the unrelated fixture HEAD and index are unchanged. Keep child environments explicit and avoid process-wide mutation. Done when a targeted regression catches switching discovery or cleanup to the production runner, and just test-fast plus just check pass. This is a pre-close review follow-up, not a concerns record.
