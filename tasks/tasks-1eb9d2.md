---
id: tasks-1eb9d2
title: "Default plan roots omit docs/superpowers/plans, unlike the default spec roots"
status: todo
priority: 2
size: xs
created: 2026-09-04T14:50:00Z
updated: 2026-09-04T21:25:29Z
depends: []
tags: [feedback, gap, "from:beliefs", cli]
---

`tasks edit <id> --plan <topic>` fails with doc_not_found ("no plan matching ... under docs/plans/") when the plan lives under docs/superpowers/plans/, where the writing-plans skill puts plans by default. spec_dirs already defaults to docs/specs plus docs/superpowers/specs and docs/superpowers/designs; plan_dirs defaults to docs/plans only, so --step cannot be used either. Expected the plan defaults to mirror the spec defaults. Workaround: set plan_dirs in tasks/.config.toml.

## Notes

- 2026-09-04T21:25:16Z (main): triage: redacted the reporter's ids and paths; promoted to todo xs: add docs/superpowers/plans to DEFAULT_PLAN_DIRS
