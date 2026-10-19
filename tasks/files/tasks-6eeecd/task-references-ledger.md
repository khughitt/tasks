# SDD ledger — plan: docs/plans/2026-10-19-task-references.md

Implementation goal: tasks-6eeecd. Design task tasks-3a445e closed after approved spec/plan.
Workspace: .worktrees/task-references; branch task-references. No setup recipe; root guide declares no dependency setup. Existing tree baseline just test-fast passed (log /tmp/task-references-baseline.log).

## Preflight: shared files and interfaces
| Tasks | Producer / consumer | Finding |
|---|---|---|
| 1, 5 | tools/cli.toml; ops CLI table → parser adoption | Sequential handoff; interface signatures agree. |
| 2, 3 | src/refs.rs; Ref entries → duplicate/citation audit | Sequential handoff; interface signatures agree. |
| 2, 4 | src/refs.rs; reference validation → location/presence | Sequential handoff; interface signatures agree. |
| 2, 5 | src/main.rs, src/refs.rs; parse/render and Task.refs → writers/query/doc | Sequential handoff; interface signatures agree. |
| 2, 6 | src/main.rs; Ref/Relation → inventory | Sequential handoff; interface signatures agree. |
| 2, 7 | src/refs.rs; Task.refs → gates | Sequential handoff; interface signatures agree. |
| 3, 4 | src/commands/check.rs, src/refs.rs, tests/refs.rs | Sequential handoff; interface signatures agree. |
| 3, 5 | src/refs.rs, tests/refs.rs; audit → write refusal | Sequential handoff; interface signatures agree. |
| 3, 6 | tests/refs.rs | Sequential handoff; interface signatures agree. |
| 3, 7 | src/refs.rs, tests/refs.rs | Sequential handoff; interface signatures agree. |
| 4, 5 | src/error.rs, src/refs.rs, src/resolve.rs, tests/refs.rs; presence/resolver → target checks and retrieval | Sequential handoff; interface signatures agree. |
| 4, 6 | tests/refs.rs; presence/locate → inventory status | Sequential handoff; interface signatures agree. |
| 4, 7 | src/error.rs, src/refs.rs, tests/refs.rs; presence → output/move gates | Sequential handoff; interface signatures agree. |
| 5, 6 | src/main.rs, tests/refs.rs; CLI writes/doc → inventory integration fixtures | Sequential handoff; interface signatures agree. |
| 5, 7 | src/commands/mod.rs, src/error.rs, src/refs.rs, tests/refs.rs; edit/transition → completion gates | Sequential handoff; interface signatures agree. |
| 6, 7 | tests/refs.rs | Sequential handoff; interface signatures agree. |
| 6, 8 | inventory behavior → README | Sequential handoff; interface signatures agree. |
| 7, 8 | gates → README | Sequential handoff; interface signatures agree. |
| 8, 9 | documented behavior → agent guidance | Sequential handoff; interface signatures agree. |

## Preflight: each task
| Task | Internal consistency check | Result |
|---|---|---|
| 1 | CLI rows vs later parser; ops setup/merge | Matches Task 5; worker must preserve unrelated ops edits and follow ops guide. |
| 2 | Grammar tests vs parse/render; storage tests vs section split; literals vs new field | Agrees. Rebase main before dispatch. |
| 3 | Code exclusions vs CommonMark; audit order vs check; record examples | Agrees; fixture writes isolated from real records. |
| 4 | locate remains file-only; entries accept directories; special/unsafe mapping | Agrees; all caller mappings explicit. |
| 5 | Tests vs flags/writers/editor and doc; clear semantics | Spec §6.1 requires no-refs conflicts; implement this clap constraint as well as clear/remove logic. |
| 6 | Inventory tests vs ordering/merge/presence and show readability | Agrees; warning and fallback paths use actual located target. |
| 7 | All done paths vs transition; recurring cleanup; move preflight atomic refusal | Agrees. |
| 8 | README behaviors vs implemented interfaces and status | Agrees; code remains separate from activation. |
| 9 | Skill guidance vs rollout; extraction includes trailing Rollout section | Task 9 implements skills only; rollout stays operator work after review. |

Ruling: Task 5 must make --no-refs conflict with --ref and --rm-ref, as spec §6.1 requires — the plan omits this clap detail but its clear/remove sequence does not override the spec — if wrong, a caller combining those flags must split its edit.

## Execution
- [x] Task 1
- [x] Task 2
- [x] Task 3
- [x] Task 4
- [x] Task 5
- [x] Task 6
- [x] Task 7
- [x] Task 8
- [x] Task 9

Pre-Task 2 rebase: git rebase main succeeded; main added task-record changes only, no code changes since verified baseline.
Task 1 implementer: /root/task_1, codex/gpt-6.1-sol medium; ops commit 17bcf0a, review dispatched /root/review_1.

Task 1: minor (deferred): existing ops shelved-dependency warning in check; unrelated to table change.
Task 1: controller verified local ff merge 5525543..17bcf0a; tt-report run; ignored __pycache__ only; clean status, no submodules, no bin or user systemd pointers into worktree; removed ops worktree and branch.
Task 1: complete (commits 5525543..17bcf0a, review clean).

Task 2: started /root/task_2, codex/gpt-6.1-sol high; BASE 96f896155df78547ac800a7fe4fd0ead7bcace22.

Baseline-count correction: observed result rows sum to 1,551 passed, 4 ignored (previous dispatch/commentary said 1,651 in arithmetic error); Task 2 fast run has 1,562 passed, +11, same test targets.

Task 2: implementation complete d242e98; handoff a7bcb1b; report task-2-report.md; review pending /root/review_2 codex/gpt-6.1-sol high over 96f8961..a7bcb1b; do not redispatch implementation.

Task 2: complete (commits 96f8961..a7bcb1b, review clean); implementation d242e98; full review task-2-review-report.md.

Task 3: implementer /root/task_3 codex/gpt-6.1-sol high; BASE 9bb3ee0ab06a54432d66838de88efe817dcd118d. Task 2 review accepted and step done before dispatch.

Task 3: implementation cffb7ae; handoff 672a365; report task-3-report.md; review pending /root/review_3 codex/gpt-6.1-sol high over 9bb3ee0..672a365. Local binary reinstalled; no real References sections introduced.

Task 3: complete (commits 9bb3ee0..672a365, review clean).

Task 4: implementer /root/task_4 codex/gpt-6.1-sol high; BASE 404ac88466b80b0226eae566d3106c92da1e80d3. Task 3 reviewed and closed before dispatch.

Ruling: Task 4 keeps file-only locate/find precedence for governing links, then guards the actual read target; only a missing file lookup uses locate_entry to detect main-only special files — spec §5.2/§9 and Task 6 require governing rows and doc to select the same target — if wrong, a special object here shadowed by a regular governing document in main is not a governing-link special_file finding (an explicit entry still reports it).

Task 4: implementation 7e8f4f0; handoff 6ce20c2; report task-4-report.md; review pending /root/review_4 codex/gpt-6.1-sol high over 404ac88..6ce20c2.

Task 4: complete (commits 404ac88..6ce20c2, review clean); controller resolution ruling accepted by reviewer.

Task 5: implementer /root/task_5 codex/gpt-6.1-sol high; BASE 9ab5c07a7d5dc1086728f8106ce11df8c3047697; ops main table ready at 17bcf0a.

Task 5: implementation 3d35e51; handoff 7bb6017; report task-5-report.md; review pending /root/review_5 codex/gpt-6.1-sol high over 9ab5c07..7bb6017. 1586 fast tests pass, 4 ignored; check, parser surface, reinstall pass.

Task 5: controller resolved adoption ⚠️: current tools/cli.toml byte-identical to ops 17bcf0a:cli.toml (both SHA256 ab5a149b99cc174c6cb91e9812e6f218b06296e14263e6cbeec678da7372bdfb); source last table commit is17bcf0a, worker reports vendored adopt.
Task 5: complete (commits 9ab5c07..7bb6017, review clean).

Task 6: implementer /root/task_6 codex/gpt-6.1-sol high; BASE e5f98a154e3a56a406bb374ef5d8c67f76a51cb2.

Task 6: implementation 6817c21; handoff 53c5a89; report task-6-report.md; review pending /root/review_6 codex/gpt-6.1-sol high over e5f98a1..53c5a89; fast1599/check/install pass.

Task 6: complete (commits e5f98a1..53c5a89, review clean).

Task 7: implementer /root/task_7 codex/gpt-6.1-sol high; BASE 7c9debff99f39167967815438c9d21b39d271e2c.

Task 7: implementation d81cfd6; handoff 613ea78; report task-7-report.md; review pending /root/review_7 codex/gpt-6.1-sol high over 7c9debf..613ea78.

Task 7: complete (commits 7c9debf..613ea78, review clean).

Task 8: implementer /root/task_8 codex/gpt-6.1-sol medium; BASE a0ff6a37bc8f8dab77d62061799929436770c0e6.

Task 8: complete (commits a0ff6a3..b607fae, review clean). Code-only boundary after controller closure:74b8e85ff5ae887c522af4bcbfa01029499b9647.
Rollout task tasks-f0c643 added under goal (depends Task9); goal must stay open until rollout complete.

Task 9 reference-skill baseline: /root/skills_baseline codex/gpt-6.1-sol medium; three source-only retrieval/application scenarios show missing guidance; full returned report skills-baseline-report.md. This is a reference-knowledge test, not a discipline/wording pressure experiment.

Task 9: implementer /root/task_9 codex/gpt-6.1-sol medium; BASE74b8e85ff5ae887c522af4bcbfa01029499b9647; code-only boundary before activation recorded. No activation until all hosts confirmed.

Task 9: implementation d11d9da; handoff 11a7ec1; report task-9-report.md; review plus source-only post-edit retrieval/application test pending /root/review_9 codex/gpt-6.1-sol high over74b8e85..11a7ec1. Skills remain unactivated.

Task 9: complete (commits74b8e85..11a7ec1, review clean); skills application GREEN after documented baseline gaps. Whole branch rebase main: up to date.

Final whole-branch review: /root/final_review codex/gpt-6-astra high; accepted with0new findings; full returned text final-review-report.md. Review note on goal impl round1 recorded before acting.
Final-review declined-to-judge dispositions: (1) cross-project/pinning/locators/relocation remain approved spec§11 non-goals; (2) other-host upgrades and activation are owned by rollout task tasks-f0c643, still gated; (3) automatic prose migration stays excluded, only explicit rollout citation-escaping sweep remains.
Plan implementation complete; rollout task active. Main has unrelated untracked tasks/tasks-de3e19.md; preserve without staging/editing.

Rollout: code-only merge5472812 on main through74b8e85; merged fast1605passed0failed4ignored13targets0warnings, justcheck pass, cargo install from main pass. Installed tasks show design task done with3inventory rows and no warnings. All-project open body citation search returned0matches/no warnings, so sweep needs no edits. Source/Cargo/README/CLI-table diff from reviewed code boundary empty.
Rollout followups filed early while host confirmation pending, each depends tasks-f0c643 and carries exact concerns note: flows-aa2328, ops-eba300, lore-8bf796.
Activation question pending: keep skills withheld until host upgrades confirmed (recommendation) or affirm all hosts upgraded and approve activation. No real References task sections created. Preserve branch/worktree/ledger until gated skills integration; do not delete.

User follow-up query: deduplicated open/closed task records; captured two deferred extensions as unreviewed ideas in main: tasks-b841bc cross-project references and tasks-d4cf54 pinned evidence revisions. Each has concerns: tasks-3a445e extension note. Existing doc-root/branch-resolution work remains separate; pickup packets, code locations and bulk migration remain unfiled pending concrete need. No code/skills/rollout changes; activation gate unchanged.
