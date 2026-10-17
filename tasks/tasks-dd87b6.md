---
id: tasks-dd87b6
title: tasks where fails with io ENOENT on a git repository whose path is not UTF-8
status: doing
priority: 1
size: s
complexity: low
process: direct
owner: fix/where-non-utf8-path
created: 2026-10-17T11:01:59Z
updated: 2026-10-17T11:18:14Z
started: 2026-10-17T11:18:04Z
depends: []
tags: [bug]
source: ops-e23130
agent: claude-code/claude-opus-5-5
---

Repro: mkdir a directory named b'caf\xe9', git init it, run `tasks where <that path> --json` -> exit 1, {"error":{"detail":"io: No such file or directory (os error 2)","kind":"io"}}. The same directory without .git answers project:null. Likely cause: git output (paths) is decoded with String::from_utf8_lossy (src/link.rs:91, :131 and similar), which replaces non-UTF-8 bytes with U+FFFD, so the path that comes back does not exist. Paths read from git should stay bytes (OsString via OsStringExt::from_vec on unix). Impact: ops's commit identity guard asks `tasks where` for every commit, so it refuses (exit 2) any commit in such a repository, and ops's git-index-health bytes test fails on every host with the global hook. Accept: a regression test with a non-UTF-8 repository path through where (and any other git-path decode the fix touches).

## Notes

- 2026-10-17T11:18:04Z (main): started
  provenance: {"harness_session":"claude-code:1c29b303-8058-4f2c-a42b-119ee67e812a","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
- 2026-10-17T11:18:14Z (fix/where-non-utf8-path): resumed
  provenance: {"harness_session":"claude-code:1c29b303-8058-4f2c-a42b-119ee67e812a","harness_session_source":"CLAUDE_CODE_SESSION_ID"}
