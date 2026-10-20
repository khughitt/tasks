# Coverage-drop audit, 2026-10-20

Question: when a scope pass proposed dropping a feedback report as covered by other task(s), did the covering task(s) hold every distinct ask the report was meant to carry?

## Method

- Population: every feedback-tagged task across the registered projects whose scope drop note cites coverage or duplication (72; all in personal projects; filed 2026-09-20..2026-10-20).
- Each report as filed (its creating commit) and each covering task as it stood at the drop (last commit within five minutes after the drop note; otherwise the current record with only pre-drop notes, flagged).
- Origin: the `tasks feedback` call that filed it, located in the session stores (71 of 72 on one host, 0 elsewhere). Anchors: title text near the creation time (62), or the CLI's `created` output when the filing command's title differed from the stored one (9). Codex subagent origins (6) store their dispatch brief encrypted, so their packets add the parent thread's turns. One report (ops-42e107) has no recoverable origin: its creating commit is a catch-all commit of other sessions' records.
- Judging: blind (the drop note's reasoning withheld), one model judge per report, from the packet alone. Pilot of four, including one known wrong drop and one known sound drop, before the full run; both controls were judged as expected. Spot checks of three partial verdicts against the packets confirmed the quoted evidence; one lost ask (a test recipe splitting quoted arguments) was confirmed still live in the current code.

## Results (71 judged)

- 46 sound: every ask held.
- 25 partial; none unsound or insufficient. By what happened to the unheld ask:
  - lost (11): no task carries it.
  - declined (9): the covering task rejected or narrowed it on purpose (2 on a recorded user decision), yet the drop called it covered.
  - pending (4): held only by research, an upstream proposal or a decision task that may end in no change.
  - moot (1): the covering work removed the condition.
- The filed report alone stated every ask in 68 of 71. In 10 of the 11 lost cases the ask was in the filed report, so the misses come from judging coverage by topical overlap, not from missing context.
- Origins: 65 agent-observed, 3 user-relayed, 3 mixed. Both user-involved misses (one user-relayed, one mixed) lost a user's own ask.
- Confidence: verdicts are mostly "mid"; treat the categories as approximate.

## Partial verdicts

| id | category | unheld ask (judge's reading, condensed) |
|---|---|---|
| flows-79e5a0 | lost | task-done records pass result when a passing test prints trailing app output: hq-af4ca5 is scoped to Vitest-no-tests, shelved, recheck narrower |
| hq-872750 | lost | Controller can tell a capacity-errored worker stopped and recover its claim: only an open design input in relay-6825f7; both relay tasks done, outcome unknown |
| lore-881f7a | lost | Hand runbook phases between harnesses as general guidance (held only for one runbook by hq-c6c847; lore-4c1787 has only a constraint clause) |
| lore-c21298 | lost | Record a preserved ledger copy's location on the task later work cites: lore-da5aae keeps the ledger rule unchanged |
| ops-075ea7 | lost | Repair stale keys.toml copies breaking the default-branch gate: only a placement question in ops-7567db |
| ops-11a4a6 | lost | ns test recipe passes a quoted multiword filter intact: ops-f17995 excludes ns; ns-80a711 does not commit to quoting |
| ops-30e334 | lost | Guard recognises a live background subagent: ops-31ea9b assumed the earlier race without rechecking timing; keeps the block |
| ops-3b4a6e | lost | ops-check treats optional machine-local paths as optional, or names a setup route: covering record is a dropped duplicate; only a consumer edit and a wording change |
| ops-a1e8b1 | lost | Setup recipes for the other adopters (material and others): ops-9c7dab files pieces only for forge, beliefs, nodes; this report was the awaited bite but no piece was filed |
| ops-fec58a | lost | User's renewed ask: accept feedback for every registered project by default (covering task, scoped earlier, rejected it); Fallback keeps feedback tag/category/provenance: explicitly not done |
| tasks-2c7979 | lost | Explicit unpark: research forbids a new command and leans to the re-done workaround the report rejected; Re-completion should not restamp model when TASKS_MODEL is set: not named |
| hq-315879 | declined | Global worktree rule names tasks start first: delivered in the tasks skill instead (commit 323b1ed); drop ruled against the target surface |
| ops-64c0ed | declined | Worktree can record verification: refused on a user decision (record only from registered checkout) |
| ops-8cf94f | declined | Guard does not block a turn end a pending child notice will wake: ops-31ea9b keeps the one-shot block, adds no detection |
| ops-b39f6e | declined | test-one latency limit and raw gate failure-rate budget: narrowed on purpose by a recorded user decision |
| tasks-3704b5 | declined | Pending proposal survives an incidental note: covering tasks settled the opposite rule (latest note answers it) and fixed only the wording |
| tasks-562c53 | declined | Unblock cross-project dependent after worktree close, or supported record-only publication: covering tasks keep the block and add only diagnostics/guidance (tasks-d0d520, tasks-70f0bc) |
| tasks-57b66e | declined | Record merge command or superset acceptance: deliberately declined in favour of documented Git recovery |
| tasks-ae03ed | declined | Cross-project note on worktree-only record succeeds: tasks-16a48c keeps the error and names the remedy |
| tasks-f27fe3 | declined | Authorized worker closes while controller holds claim: tasks-f8c9cc is a policy decision framed to keep controller-closes |
| lore-6ea76d | pending | Vendored Finish step still prescribes rm -rf: lore-48dfa8 delivered only an upstream proposal |
| ops-29e951 | pending | Permit owned mktemp cleanup or name the permitted form: covering is investigation (ops-278533) that may end in no change, plus an idea (ops-75b29a) |
| ops-7933db | pending | Same as ops-29e951: covering commits only to diagnosis and a decision |
| tasks-673b21 | pending | Workflow sends closure back to the controller: tasks-f8c9cc only identifies a follow-up |
| ops-9cc7f4 | moot | Refusal names --force as the way out meanwhile: covered only indirectly; both covering tasks now done, likely moot |

## Sound verdicts

flows-868da3, hq-1d228b, hq-381218, hq-52e1d9, hq-982164, hq-d38086, lore-03f7e9, lore-3e5387, lore-8ce941, lore-969323, lore-bdd559, lore-d84cdf, lore-f22f79, ops-007e2e, ops-033d30, ops-2538d7, ops-2dfb52, ops-35a0a1, ops-38f668, ops-39b8e9, ops-4e97bf, ops-552425, ops-5bb3e5, ops-61a6c1, ops-89d93b, ops-b93d2e, ops-c3ccb4, ops-c89678, ops-c96bb4, ops-eb3fb3, ops-eda646, ops-f91c67, ops-f99904, ops-fbc19d, ops-fc82dc, relay-ebd6f5, tasks-06afbe, tasks-0f8043, tasks-16de1c, tasks-2337b3, tasks-2b2b49, tasks-84d2a9, tasks-c62c8d, tasks-de3e19, tasks-ebdc96, tasks-ef69c0

## Unavailable

ops-42e107: origin unrecoverable; not judged.
