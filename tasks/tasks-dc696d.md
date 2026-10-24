---
id: tasks-dc696d
title: Adopt bounded reads in agent guidance after the trial gate
status: blocked
priority: 2
size: m
complexity: mid
process: direct
created: 2026-10-24T16:09:37Z
updated: 2026-10-24T16:47:22Z
depends: [tasks-7c5a4b, tasks-72093a, fam-70cf4f, forge-8b1006, forge2-0473c4, material-292de8, mind6-40ee7d, mm-b90315, nrp-8fcbd5, prism-9f2087, relay-e6c38f, sun-5a4acf, tui-cb7a3d, wali-143e8f]
parent: tasks-65b733
tags: []
source: "tasks-e7424b:plan-task-3"
agent: codex
spec: docs/specs/2026-10-24-bounded-session-summary-design.md
plan: docs/plans/2026-10-24-bounded-session-summary.md
step: "Task 3: Adopt bounded reads in agent guidance after the trial gate"
---

After binary delivery and verified reinstall, coordinate all inventoried project startup guides and the skill under the flow-trial-1 gate. No source edits before readout or specific owner no-effect verdict. The skill remains gated even if individual unenrolled guides are released earlier.

## Notes

- 2026-10-24T16:10:41Z (design/tasks-e7424b): Await flow-trial-1 readout (2027-02-16) or the user's no-effect verdict for this precise startup-guidance change. Skill remains gated for enrolled sessions.
  provenance: {"harness_session":"codex:01a16e34-fd27-7e51-b10c-c1250b33416d","harness_session_source":"CODEX_THREAD_ID"}
- 2026-10-24T16:47:22Z (design/tasks-e7424b): Measurement-scope check: flows 84f3f9ea87add384c54ec19771914d49489716f5 bin/trial-verdict joins census members only, and its outside-census regression confirms this. Obs 4331d7a19b62036578ab774625626047f93acd53 outcome_report.py defaults project=None and computes comparisons over all selected projects; the trial case requests that unfiltered report. This does not establish a no-effect verdict for outside-project guide adoption. Keep the existing gate; the trial owner must judge broader measurement use before any early release.
