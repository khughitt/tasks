# Vendored relay fixtures

`ancestry.json` is relay's cross-language contract for the session-process rule, copied
byte for byte from relay `test/fixtures/ancestry.json` at revision `45f4c47`. Its own
`schema` field is the fixture format version, not the registry schema.

`src/relay/ancestry.rs` runs every chain through tasks' walk. Refresh it only by copying
the file from a newer relay revision and updating the revision above in the same commit.
A new chain whose `session` is null fails that test until its tasks-side expectation is
added to `NULL_SESSION_EXPECTED` there.

`handles.json` is relay's process-handle contract, copied byte for byte from relay
`test/fixtures/handles.json` at revision `bfed7d7`. `src/relay/snapshot.rs` runs its
`starts` and `handles` cases through `snapshot::parse`; its `darwin` cases cover relay's
`ps` row parsing, which tasks never performs, so they are not run. Refresh it the same
way: copy the file from a newer relay revision and update the revision above in the same
commit. Its `schema` field is the fixture format version and the test fails when it changes.

`registry-workers.json` is relay's shared worker-row snapshot: a Codex root row, a worker
and a nested worker (whose `root` names the root, not its parent) on one handle, and an
unrelated Claude Code row. Relay commits it as `test/fixtures/registry-workers.json` in
relay-635fdc Task 1, after tasks has landed, so it was copied byte for byte from that
task's step in relay's plan `docs/plans/2026-10-15-codex-worker-rows.md` at revision
`74ce0d8`. When relay's commit lands, compare the bytes, then replace this source with the
relay revision of `test/fixtures/registry-workers.json`. A difference is a contract change
to settle with relay, not a local edit. `src/relay/snapshot.rs` parses it, and
`src/relay/resolve.rs` resolves each of its Codex threads against it.
