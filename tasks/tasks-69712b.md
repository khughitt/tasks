---
id: tasks-69712b
title: "sample --older-than 0 shortfall warning reads \"not updated in 0 days\""
status: todo
priority: 3
size: xs
created: 2026-09-09T10:32:53Z
updated: 2026-09-09T10:32:53Z
depends: []
tags: [cli]
---

The warning interpolates older_than verbatim; with --older-than 0 the age check was skipped yet the warning says 0 days. Branch the parenthetical on older_than == 0. Found by the curation branch final review.
