# Vendored relay fixtures

Everything here except `whoami.mjs`, `sources/package.json` and this file is copied byte
for byte from relay at revision `f62924e0d45241b4215caca31833645b50425015`. Refresh all of it
together from one newer revision and update the revision above in the same commit. A
difference is a contract change to settle with relay, not a local edit.

- `whoami.json` (relay `test/fixtures/whoami.json`) is relay's conformance corpus for
  `relay whoami`. `src/whoami.rs` parses every expected result, and `src/claims.rs` checks
  every case whose `tasks` object gives a proof-only verdict.
- `ancestry.json` (relay `test/fixtures/ancestry.json`) is relay's session-process rule.
  `src/hooks/ancestry.rs`, the Stop guard's walk, runs every chain. Its `schema` field is
  the fixture format version, not the registry schema.
- `sources/src/` holds the modules `src/identity/whoami.js` imports, at their relay paths.
  `whoami.mjs` serves `relay whoami --json` from them with relay's stdout and exit contract.
  `TestEnv::frozen_relay` puts it on a test's PATH, so the relay-on tests resolve real
  processes against a real registry file. It needs Node 22 or newer, as `tests/hooks.rs` does.
