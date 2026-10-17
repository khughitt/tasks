---
id: tasks-820d8e
title: "The scope skill says to keep every evidence read in the fixed checkout, but a feedback idea's evidence (the incident's plan, ledger, or the tool it names) usually lives in the reporting or owning project's registered checkout; the skill does not say whether reading those is allowed or only forbids worktree copies"
status: todo
priority: 2
size: xs
complexity: low
process: direct
created: 2026-10-15T22:29:07Z
updated: 2026-10-17T02:28:24Z
depends: []
tags: [feedback, friction, "from:lore"]
agent: claude-code
---

Why: skills/scope/SKILL.md says to use the fixed root "for every evidence read" (§1, line ~40) and "Keep every read in the fixed checkout" (§2, line ~61). The rule exists so a pass never mixes main and worktree copies of this project's own records (§1: a registered root resolved from a worktree points at the main checkout). But a feedback idea's evidence (the incident's plan, ledger, or the tool it names) lives in the reporting or owning project's registered checkout, and feedback titles deliberately omit project content. As written, the skill reads as forbidding those reads, so passes either skip the evidence or break the rule. Original report (from:lore): the skill does not say whether reading those is allowed or only forbids worktree copies.

Done: §1 and §2 say that task commands, this project's records and code, and every document write stay in the fixed root, and that read-only evidence from another registered project (its records via `tasks --project <prefix>`/`tasks root`, its plans, ledgers, or the tool a feedback idea names) may be read from that project's registered checkout, never from a worktree of either project. Nothing is ever written there. The §2 sentence on failed lookups stays. The wording is consistent with §6 ("documents through explicit paths under the fixed root").

Where: skills/scope/SKILL.md §1 (the root paragraph) and §2 (the gather paragraph). Check: reread both sections for one consistent rule. Keep the shipped copy in step (AGENTS.md: skills ship to other projects).
