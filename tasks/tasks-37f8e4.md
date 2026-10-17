---
id: tasks-37f8e4
title: Decide whether re-reviewing an idea clears awaiting_user
status: todo
priority: 2
size: s
complexity: low
process: direct
created: 2026-10-17T17:45:11Z
updated: 2026-10-17T18:03:46Z
depends: []
tags: []
source: tasks-aeeef1
agent: claude-code/claude-opus-5-5
---

Why: `note --reviewed` (Review::Set) on an idea already awaiting the user keeps the marker. A re-scope that gives such an idea a new edge therefore leaves it hidden after that edge closes, the failure wait-aware spec §7.3 warns about, reached through the normal writer. Spec §3.1 lists what clears the marker and omits `--reviewed`, so the shipped code is compliant; this changes the rule. Raised by the final whole-branch review of tasks-aeeef1; split out of tasks-d2cdfe (whose "Needs a spec decision first" bullet this task covers) because it needs a spec decision.

Decision (scope pass 2026-10-17; the user may override before start): a review replaces the previous review's wait, so `Review::Set` clears `awaiting_user` unless `--awaiting-user` rides along (`Awaiting::Set`). The scope `question` verdict already writes `--reviewed --awaiting-user`, so a re-review that keeps the question keeps the marker. Rejected: keep the marker and have the scope skill pass `--no-awaiting-user` on every re-review, because that relies on an agent remembering a flag, the class of failure the wait-aware design replaced with derived state.

Established (checked 2026-10-17 at 8e1ff08):
- `src/review.rs` `apply()`: `Review::Set` sets `reviewed` and `reviewed_at` and leaves `awaiting_user` as is; only `Review::Clear` and `Awaiting::Clear` clear it. `note --reviewed` and `edit --reviewed` both reach it (`src/commands/mod.rs` ~2032, `src/commands/edit.rs` ~286). The editor sets the marker explicitly, so it is unaffected.
- `--awaiting-user` already conflicts only with `--no-reviewed` and `--no-awaiting-user` (`src/cli.rs` ~321), so `--reviewed --awaiting-user` stays valid.
- The registry has four `awaiting_user` ideas today (material-2592a2, material-2d0360, mind6-0e690d, mm-370e7f). None is mid re-review, so nothing needs migrating; mm-370e7f waits on both the user and mm-f1fde8, written in one `--reviewed --awaiting-user` note, which the new rule keeps.

Done:
- `apply()`: under `Review::Set` with `Awaiting::Keep`, clear `awaiting_user`; when that clears a marker that was set, warn `<id> no longer awaits the user; add --awaiting-user to keep it`.
- Tests (RED first) in `tests/review.rs`: a re-review with `--reviewed` clears the marker and warns; `--reviewed --awaiting-user` keeps it; an idea re-reviewed with a new edge comes back as `woken` once that edge closes.
- Spec `docs/specs/2026-10-17-wait-aware-review-design.md` §3.1 "Cleared" gains `--reviewed` without `--awaiting-user`, with a dated amendment line; README (~line 30) and `skills/tasks/SKILL.md` (~line 63) say a re-review clears the marker unless `--awaiting-user` is given again.
- No CLI surface or JSON shape change.

Where: `src/review.rs` `apply`, `tests/review.rs`, spec §3.1. Related wording item in tasks-d2cdfe (plain-note warning vs answering a proposal) touches the same warnings; keep the two consistent.

## Notes

- 2026-10-17T17:45:29Z (main): concerns: tasks-aeeef1 change — whether a re-review should clear awaiting_user, which the shipped rule keeps
- 2026-10-17T18:03:46Z (main): scope: scoped; premise confirmed at 8e1ff08 (review::apply keeps awaiting_user under Review::Set; registry has 4 awaiting ideas, none mid re-review); decided Review::Set clears the marker unless --awaiting-user rides along, rejecting a skill-side --no-awaiting-user rule; direct, s/low
