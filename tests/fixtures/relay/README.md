# Vendored relay fixtures

`ancestry.json` is relay's cross-language contract for the session-process rule, copied
byte for byte from relay `test/fixtures/ancestry.json` at revision `45f4c47`. Its own
`schema` field is the fixture format version, not the registry schema.

`src/relay/ancestry.rs` runs every chain through tasks' walk. Refresh it only by copying
the file from a newer relay revision and updating the revision above in the same commit.
A new chain whose `session` is null fails that test until its tasks-side expectation is
added to `NULL_SESSION_EXPECTED` there.
