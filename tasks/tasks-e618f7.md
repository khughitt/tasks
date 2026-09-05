---
id: tasks-e618f7
title: Reject unreadable claim stores and resolve final review findings
status: done
priority: 1
size: s
owner: feat/doing-claims
created: 2026-09-05T13:21:20Z
updated: 2026-09-05T13:24:17Z
depends: []
tags: []
spec: docs/specs/2026-09-05-work-claims-design.md
---

Final branch review reproduced inaccessible claim state appearing as no claims. Load directly and treat only NotFound as empty; add a real permission-error regression and correct the design exception count.

## Notes

- 2026-09-05T13:24:17Z (feat/doing-claims): Made claim-store loads treat only missing files as empty, added permission regression coverage, and corrected the documented exception count.
