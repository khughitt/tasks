# Scoped re-review of corrective round 1 (5b7549f)

Reviewer: fresh general-purpose subagent, claude-code/claude-opus-5-5. Round verdict: accept.

Product code unchanged: 5b7549f touches only the `#[cfg(test)]` module of src/blockers.rs (starts line 433) and tests/blockers.rs; src/blockers_text.rs and src/commands/blockers.rs untouched; 5b7549f..HEAD adds only task records. Ran `GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_NOSYSTEM=1 just test-one blockers_` in a throwaway worktree with its own CARGO_TARGET_DIR: 10 passed (since removed).

## Finding 1 (pretty continuation labels): Closed
Labels now render on ranked rows (each labelled task is a blocker with its own consumer): user quiet park with --needs headless plus task need paid-calls; agent park; reviewed idea --awaiting-user; hand-seeded cycle (acceptable, CLI refuses cycles); 30d deferral. Expected strings match src/blockers_text.rs:150-203 and spec lines 203-213/284-308/383-385; host and needs on the same row pin distinct labels. Catches the regression class (host under needs label drops `host:`; needs under host drops `needs:  paid-calls`; merging loses one line). Independent probe: changing the reviewed-idea wording failed at tests/blockers.rs:740. Isolation holds (TestEnv + CLI + config/record file edits, no git).

## Finding 2 (alias/move dedup): Closed
Fixture at src/blockers.rs:762-819: own (1,1) for aa-000030 naming the open target twice (move + direct) and a missing id twice (alias + canonical); exactly one diagnostic (aa-000030, old-0000ee, Some(aa-0000ee)) retaining stored spelling plus canonical (spec 123, 158-159); target bb-000021 counts [2,3,0,0,1,0,0] correct (relief 1 because aa-000030's unresolved ref disqualifies it, spec 149-155; transitive includes aa-000031); aa-000022 counted once in examples [aa-000022, aa-000030] (canonical order, spec 270) and in maps (direct idea1/todo1, transitive idea2/todo1). Independent probe (inflate unmet when unresolved present) failed at src/blockers.rs:786 left (2,1) right (1,1).

## New issues
Critical: none. Important: none. Minor:
1. tests/blockers.rs:730-741 label assertions search the whole output, not tied to the row; a label on the wrong row would pass. Could split output by row id.
2. src/blockers.rs:800-803 single-diagnostic expectation pins first-stored-spelling behaviour (deferred Minor 2); must change if Minor 2 is addressed.
3. tests/blockers.rs:729 the 120-column line-width check is not repeated on the fuller fixture; low risk.

## Declined to judge
- Exact fidelity of the ledger's recorded RED mutations; relied on own probes instead.

Nothing pushed; no processes left; scratch worktree and build dir deleted; reviewed worktree unchanged (HEAD 7f5129d, clean).
