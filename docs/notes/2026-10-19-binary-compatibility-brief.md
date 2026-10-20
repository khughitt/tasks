# Identify binary compatibility before synced-record upgrades

## Problem

An operator needs to establish whether an installed `tasks` can read the records
another host writes. `tasks-db7049` reports indistinguishable binary versions;
`tasks-476fd7` reports a stale reader suggesting a record repair instead of an
upgrade. These are related gaps in the coordinated-upgrade workflow.

## Current behaviour and evidence

- `src/cli.rs` uses Clap's package version. Installed `tasks --version` prints
  `tasks 0.4.0`; `Cargo.toml` is also 0.4.0. Commit `4a436e5` introduced that
  version on October 15. Both the wait-aware merge `19974fe` and reference-format
  merge `5472812` retain it despite changing records or their interpretation.
- The completed rollout `tasks-f0c643` verified `show`'s `inventory` field on
  each host. Its outcome and `tasks-06be6f`'s latest note establish that those
  upgrades finished; this pass does not reopen the rollout or verify hosts anew.
- `src/format.rs::parse_task` quotes known timestamp keys, runs
  `frontmatter::parse`, then rejects unknown keys. At `4a436e5`, timestamp quoting
  omitted `reviewed_at` and `closed`; `5bf4280` added those fields and `c2b7095`
  shared their timestamp list with rename. An unfamiliar unquoted timestamp's
  colons can therefore produce `src/frontmatter.rs`'s “quote it” error before
  unknown-key validation. This is a supported failure mechanism, not a reproduced
  identification of the report's exact binary, key or record, which were not saved.
- `README.md`'s Upgrading guidance and the wait-aware design §7.1 rely on
  version checks. `tests/cli.rs::cli_vocabulary_help_version_usage` checks that
  version output succeeds and starts with `tasks `, not that it identifies a
  compatibility boundary. No existing brief, draft spec or open follow-up found
  in the local corpus settles this contract. `tasks-96b215` studies consumption
  by nodes; it does not answer stale-reader identification.

## Constraints

Preserve strict parsing, records, history and actionable malformed-record errors.
An unknown field or reserved character alone cannot prove that a writer was newer.
Already-deployed older executables cannot gain better diagnostics without upgrading.
Build identity and readable-format compatibility are different claims. Keep
coordinated upgrades before new writes; design and implementation-plan reviews
precede implementation. CLI additions start in ops's vocabulary; JSON shape changes
need explicit scope. This is a bounded child effort in the record-authority lane
`tasks-0e432f`, with no new lane or host-state service implied.

## Alternatives

1. **Release versions at compatibility boundaries, with honest upgrade guidance.**
   Current lean: extend the existing versioned-upgrade practice to every reader or
   output-contract change that needs coordination, document a minimum version,
   and specify when a parse error may suggest checking the binary. Decide how
   review or checks keep the version policy from drifting again.
2. **Embed build identity and define compatibility separately.** A commit identifier
   distinguishes builds, but does not establish which records they read. Design
   would also need dirty-tree, source-archive and ordering semantics, and decide
   whether a durable format marker justifies another coordinated transition.
3. **Track installed builds across hosts.** Shared state could expose lagging hosts,
   but adds registration, freshness and authority questions. Defer unless local
   binary identification demonstrably cannot satisfy the rollout gate.

## Unanswered questions

- Which changes require a version boundary, and what enforces it? The design
  follow-up proposes a policy and regression check for user review.
- Is an explicit compatibility marker worth its rollout cost, or is qualified
  upgrade advice sufficient? The design compares unfamiliar timestamps and keys
  with genuinely malformed records, without claiming certainty it cannot establish.
- Can the original failing binary and record be recovered? The reporter may supply
  them; otherwise retain the unknown and use a labelled historical fixture.

## Proposed decomposition

- `tasks-83e0e1` groups this work under `tasks-0e432f`; it introduces no new lane.
- `tasks-e3fdce`, **Design binary compatibility from the brief** (P2, s, high,
  planned), settles binary identification, compatibility claims, diagnostics, a
  checkable release policy and rollout order. Its declared output is
  `docs/specs/2026-10-19-binary-compatibility-design.md`, not yet written or approved.
- Both `tasks-476fd7` and `tasks-db7049` remain ideas with recorded dependency
  edges to `tasks-e3fdce`. The design's completion must deliver a finding with
  `note --no-reviewed` to each before its branch merges.

No separate research task is needed to repeat the source/history trace already
recorded here. Implementation remains outside this scope pass.
