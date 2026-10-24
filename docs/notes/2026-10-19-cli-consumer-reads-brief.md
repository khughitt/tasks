# CLI reads for agents and dashboards

## Problem

Agents need a session summary that fits a tool response; dashboards need timely
cross-project task lists and canonical project names and purposes. Ideas
tasks-4ca7bd, tasks-bc5efc and tasks-6845e6 concern the same JSON read surface,
but need different evidence before implementation.

## Current behaviour and evidence

Checked against `89c58a7` on 2026-10-19; installed CLI reports `tasks 0.4.0`.

- `src/commands/list.rs::prime` serializes all ready rows and a roadmap;
  `PrimeOut` and `TaskSummary` in `src/output.rs` define their contracts. One
  local read produced 101,001 bytes: ready 31,045 bytes (63 rows), roadmap 60,474,
  lanes 9,136, measured by compactly serializing each section. Its elapsed time
  was 0.131 s, an observation rather than a benchmark. The session-start response
  was truncated. The report's roughly 250 KB in another project was not reproduced
  here. Limiting ready alone would leave most of this local payload.
- `src/commands/list.rs::list` calls `ReadCtx::scan_with_claims`, builds review
  dependencies, then filters tasks. `Project::scan` in `src/repo.rs` reads complete
  records; each selected row goes through `TaskSummary::of`. The reported 3.1 s
  across roughly 30 projects is unverified here; no bottleneck is established.
- `src/commands/projects.rs::run` scans reachable projects for counts/activity and
  emits `ProjectRow`, which has neither `name` nor `purpose`. Its pretty table
  shows the prefix. tasks-6845e6 records the user's 2026-10-17 decision to expose
  optional name/purpose from root `identity.toml`, with absence for missing or
  unreachable roots. Its original source, consumer tui-0fa4d0 and schema owner
  ops-03f3e3 are retained. Their external documents were not inspected under this
  pass's fixed-checkout boundary; this does not establish missing evidence.

No matching local brief, attached draft spec, or overlapping open research was
found. Related tasks-282666 owns the broader migration of direct-file consumers
to JSON and is deferred to 2027-02-17 under a flow-trial measurement gate.

## Constraints

JSON is a public contract. Preserve existing selection, ordering, claim/copy
precedence, warnings, and malformed-record errors. A bounded response must disclose
omissions and offer a complete read. Missing optional metadata differs from an
invalid file; lean toward typed failure for malformed/unreadable identity data.
Check schema ownership before defining another parser contract. Keep aliases and
pretty-table changes outside the minimal name/purpose addition unless evidence
requires them. New CLI options must land in ops's CLI table before adoption here.
Any change overlapping tasks-282666's measurement path must evaluate its existing
trial gate before rollout; this brief does not waive it. No shared needs are declared.

## Alternatives

1. **Keep the full contract, add an explicit bounded summary, and optimize measured
   costs.** Current lean: make omitted work discoverable, then assess a default
   migration separately. Add identity fields only after verifying their schema.
2. **Cap or reshape default `prime` output.** Directly addresses the report, but
   requires a migration decision and bounds for roadmap and lanes as well as ready.
3. **Cache cross-project reads.** Consider only if measurements justify the
   invalidation and freshness contract; reduced serialization alone may not fix
   scan cost, and a cache is not yet warranted.

## Unanswered questions

- Which sections and byte/row budget make session startup useful, how is completeness
  recovered, and should defaults change? Resolved by accepted spec `76501bd` and
  implementation plan `3a264b9` on 2026-10-24: opt-in JSON `prime --summary`,
  12,288 stdout bytes including LF, exact omission metadata and full-data argv,
  whole-row eviction and complete warnings/halts. Ordinary prime stays full.
  Implementation goal tasks-65b733 owns binary delivery, separate approved shared
  reinstall and gated guide adoption. The real consumer token-budget pilot and
  trial measurement-scope check remain required before adoption; no no-effect
  verdict has been issued. The plan inventories thirteen active project guides
  plus the skill/README, with other-owner records blocked under the same gate.
- Where does listing time go, and what is the cheapest useful improvement? The
  listing research follow-up measures scan, dependency/claim work, row construction
  and serialization, with host/dataset conditions and repeat timings.
- What identity schema and linked-root rule actually apply, and how should optional,
  malformed and unreadable metadata behave? The identity research follow-up checks
  owner and consumer evidence and recommends concrete fixtures. Pretty names and
  aliases remain explicit scope decisions, with omission the current lean.

## Proposed decomposition

All three ideas remain **briefed** with one dependency apiece beneath goal
tasks-96c00c, an ordinary child of the existing Shared vocabulary and presentation
lane (tasks-1f05a7). The lane's guidance and broader deferred migration task are
unchanged.

- tasks-e7424b — summary design: P2/m/high/planned; wakes tasks-4ca7bd.
- tasks-e4ccb6 — listing measurement and recommendation: P2/s/mid/direct;
  wakes tasks-bc5efc.
- tasks-48216e — identity contract verification and recommendation:
  P2/s/mid/direct; wakes tasks-6845e6.

Each follow-up updates this brief and records its finding with `--no-reviewed` on
its waiting idea before the result's branch merges. Implementation is scoped after
those findings; no cache or new default is authorized by this handoff.
