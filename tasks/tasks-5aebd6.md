---
id: tasks-5aebd6
title: "Detached records: deferred review minors"
status: todo
priority: 3
size: s
complexity: low
process: direct
created: 2026-10-23T01:04:35Z
updated: 2026-10-23T01:04:35Z
depends: []
tags: []
source: tasks-211e28
agent: claude-code/claude-fable-5-1
---

Minor findings the per-task and final reviews of docs/plans/2026-10-22-records-detached.md deferred (ledger attached to tasks-211e28 as sdd-ledger.md). Code: compare [records] (and [projects]) values with canonical paths through dir_identity in retire::authority, mod.rs effective_retirement/reject_retired, check.rs and link::refuse_moved; replace the two silent same-directory fallbacks in rename (snapshot.rs records_pair, classify.rs) and the four <parent>/<target> constructions with one typed helper; move the derived-directory and tasks/ creation in init --records after the last refusal (init.rs, repo.rs init_detached); the config error for an unpaired prefix whose config is detached names init --records although that command's strict scan refuses the same way (name tasks unregister first, or exempt the prefix being paired); a paired prefix whose records directory has no config still says run tasks init there; adopt.rs old-root-still-holds-a-project check cannot fire for a detached project; validate_records repeats validate_checkout's preamble and overlap loop; wording: scope.rs unreachable names the code root when the records dir is missing, mod.rs and move_.rs say registered checkout when naming a records directory, registry validates [records] before [aliases] so a retired alias key reads as unregistered. Tests: tasks commit run from the code checkout of a detached project; check under a hook environment (GIT_INDEX_FILE set) across the staged-deletion window; resolve id form carries records; resume-over-root and records-over-code precedence and the bare-name break-at-first-base; detail assertions for the rebase-or-merge remedy; init --records with a records root that is a file and the inside-checkout refusal detail. Docs: the plan says it implements §12 cases 1–6 but case 6 is flows' fingerprint; its Task 3 filter just test-one registry::records selects nothing. Not enforced and worth a ruling: a records root nested inside another registered repository, and the directory-name-equals-prefix contract on registry load.
