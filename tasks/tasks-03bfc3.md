---
id: tasks-03bfc3
title: "Restructure README into quick start, concepts, and reference"
status: todo
priority: 2
size: s
complexity: mid
process: direct
created: 2026-09-19T13:13:54Z
updated: 2026-09-30T10:10:19Z
depends: []
tags: [docs]
agent: "claude-code/claude-opus-5[1m]"
---

After the 2026-09-19 staleness pass the README is still a linear accumulation of feature notes (provenance, process, rename recovery) between the agent snippet and the command list. A first-time reader wants: install, one-screen session loop, the concepts (statuses, goals, process, park reasons, claims), then a reference section per feature with its design link.

## Scope and acceptance

Why: README.md still places extensive provenance, park, and relay reference text before Install; the first-time reader must traverse implementation detail to reach the basic loop.

Done: organize the README in the requested order: quick start (install, initialize, add a task, and one short session loop), concepts (statuses, goals/dependencies, process, parks, claims), then feature reference with the existing design links. Keep the agent-adoption snippet easy to find. Preserve all existing contracts, examples, recovery cautions, and installation alternatives while consolidating repetition; do not change CLI behavior or introduce a docs site.

Check: account for every existing section in the new organization; verify local document links and heading anchors, including incoming README links found with rg. Walk the quick-start commands in a disposable project with isolated XDG_CONFIG_HOME and XDG_STATE_HOME and confirm tasks check succeeds. Read the JSON omission, provenance, rename/adoption, attachment, and process passages against their current versions to catch accidental semantic changes.

Where to look: README.md, skills/tasks/SKILL.md, the design links already in README.md, and command help. Coordinate links with tasks-9a9ef3 without making either task depend on the other. Process direct: the requested information order and preservation check settle the approach.

## Notes

- 2026-09-30T10:10:19Z (main): scope: scoped; verified README still puts reference detail before Install; specified quick-start/concepts/reference organization, preservation and link checks, and an isolated command walkthrough; p2/s/mid, direct
