---
id: tasks-905fc2
title: "Amend the records-home design §7.3: the retained set seeds from every tracked file outside the moved paths"
status: todo
priority: 3
size: xs
complexity: low
process: direct
created: 2026-10-23T12:35:54Z
updated: 2026-10-23T12:35:54Z
depends: []
tags: [docs]
source: ops-5ec535
agent: claude-code/claude-opus-5-5
---

records-migrate (ops-5ec535) seeds the retained-set closure from every tracked file outside the moved paths, docs/ files outside the moved directories included (docs/runbooks/, docs/reports/). §7.3's 'one outside docs/ and outside every moved path' would skip those, so a remaining docs file's citation of a moved spec would break and the post-commit scan would stop only after the removal commit. Reviewers in all four rounds endorsed the broader reading. Rewrite that sentence to match, and note that a docs/README.md index retains everything it lists.
