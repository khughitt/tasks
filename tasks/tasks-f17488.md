---
id: tasks-f17488
title: Doc-root rejections name spec_dirs and plan_dirs in tasks/.config.toml
status: doing
priority: 2
size: xs
complexity: low
process: direct
owner: main
created: 2026-09-11T12:17:29Z
updated: 2026-09-24T13:43:25Z
started: 2026-09-24T13:43:25Z
depends: []
tags: [feedback, friction, "from:atoms", cli]
---

A project that keeps design documents outside the default spec roots (reported: designs under docs/plans) sees `spec "…" must be under docs/specs/ or …/` from edit --spec and concludes the document cannot be attached. Listing the directory in `spec_dirs` in tasks/.config.toml attaches it (0f7d904; verified on a scratch project, check clean), but neither rejection names that key.

Done when both rejections end with a pointer to the config key for their kind (`spec_dirs` or `plan_dirs`, in tasks/.config.toml): the explicit-path error in `validate_doc_path` (src/format.rs) and the bare-name `no … matching` error in `Resolver::resolve_doc` (src/resolve.rs). An end-to-end test in tests/cli.rs asserts the key appears in each. The error kinds stay unchanged.

Original report: both a document stem and an explicit relative path passed to edit --spec were rejected when the repository convention stores designs under docs/plans; expected an explicit path to identify the document without moving historical files.

## Notes

- 2026-09-24T13:07:13Z (main): curate: stale; body records that spec_dirs (0f7d904) already attaches a design under docs/plans; proposal: drop citing 0f7d904, or retitle to "the must-be-under rejection names spec_dirs/plan_dirs in tasks/.config.toml" and keep as a small error-message fix
- 2026-09-24T13:37:46Z (main): Decided (user): keep as the smaller fix — rejections name spec_dirs/plan_dirs; scoped todo, xs, low, direct.
- 2026-09-24T13:43:25Z (main): started
  provenance: {"harness_session":"claude-code:c3b66540-0acc-4861-917d-8c10dfaea3bd","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
